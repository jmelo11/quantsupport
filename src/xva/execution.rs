//! Reusable native exposure and XVA execution with setup-time risk linearization.

use crate::{
    ad::{scalar::Scalar, CompileError, CompiledProgram, Recorded, Recording, RecordingScope},
    models::lgm::marketpathplan::MarketPathPlan,
    time::date::Date,
    utils::errors::{QSError, Result},
    xva::{
        aggregator::XvaAggregatorFactory,
        claimevaluationstrategy::ClaimEvaluationStrategy,
        contigentclaim::ContingentClaim,
        nettingset::NettingSet,
        visitors::exposureevaluator::{
            ExposureResult, NpvCube, VariationMarginSimulation, XvaModelSetup, XvaValue,
        },
    },
};
use rayon::prelude::*;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write,
    hash::BuildHasher,
};

/// Numerical execution owned and rebound by the existing `XvaEngine`.
///
/// Prepared state owns numeric coefficients, first-risk maps and native code.
/// The engine invalidates changed claim/CSA bindings and refreshes market
/// coefficients before reusing compatible native code.
pub struct XvaExecution {
    program: CompiledProgram,
    market: MarketPathPlan<f64>,
    parameters: Vec<f64>,
    jacobian: Vec<Vec<f64>>,
    labels: Vec<String>,
    dates: Vec<Date>,
    netting_sets: Vec<String>,
    measures: Vec<(usize, String)>,
    paths: usize,
}

struct Chunk {
    cubes: Vec<Vec<Vec<f64>>>,
    adjustments: Vec<f64>,
    gradient: Vec<f64>,
}

impl XvaExecution {
    /// Captures a supported market/claim/aggregation setup and compiles a reusable path program.
    ///
    /// # Errors
    /// Rejects unsupported models, scripted claims or custom aggregators without
    /// native plans, invalid numerical coefficients, and native compilation failures.
    #[allow(clippy::too_many_lines)] // Preparation keeps risk-coordinate and coefficient layouts together.
    pub fn prepare<S, H1, H2, H3>(
        dates: &[Date],
        trades: &HashMap<String, &[ContingentClaim], H1>,
        factories: &HashMap<String, Vec<Box<dyn XvaAggregatorFactory>>, H2>,
        margins: &HashMap<String, VariationMarginSimulation, H3>,
        setup: &S,
        previous: Option<&Self>,
    ) -> Result<Self>
    where
        S: XvaModelSetup,
        H1: BuildHasher,
        H2: BuildHasher,
        H3: BuildHasher,
    {
        if dates.is_empty() || setup.n_paths() == 0 {
            return Err(QSError::InvalidValueErr(
                "compiled XVA requires dates and positive path count".into(),
            ));
        }
        let _scope = RecordingScope::new();
        Recording::start();
        setup.with_model(dates, &mut |model, model_leaves| {
            let market = model.compiled_path_plan()?.ok_or_else(|| {
                QSError::NotImplementedErr("market model has no compiled path plan".into())
            })?;
            if model.uses_compact_dated_requests() {
                return Err(QSError::NotImplementedErr(
                    "compiled XVA requires global request slots".into(),
                ));
            }
            let market_count = market.parameters().len();
            let mut parameters = market.parameters().to_vec();
            let mut labels: Vec<String> = model_leaves
                .iter()
                .map(|(label, _)| label.clone())
                .collect();
            let mut leaves: Vec<Recorded> = model_leaves.iter().map(|(_, leaf)| *leaf).collect();
            let mut netting_sets: Vec<_> = trades.keys().cloned().collect();
            netting_sets.sort();
            let mut aggregations = Vec::new();
            let mut measures = Vec::new();
            for (ns, name) in netting_sets.iter().enumerate() {
                let mut plans = Vec::new();
                for factory in factories.get(name).into_iter().flatten() {
                    let bundle = factory.create_aggregator(dates[0], dates);
                    let plan = bundle.aggregator.compiled_plan(dates).ok_or_else(|| {
                        QSError::NotImplementedErr(format!(
                            "aggregator {} has no compiled path plan",
                            factory.name()
                        ))
                    })?;
                    let start = parameters.len();
                    parameters.extend(plan.coefficients());
                    plans.push((plan.numerical(), start..parameters.len()));
                    measures.push((ns, factory.name().to_owned()));
                    for (label, leaf) in bundle.leaves {
                        labels.push(format!("{name}.{label}"));
                        leaves.push(leaf);
                    }
                }
                aggregations.push(plans);
            }
            let jacobian = Recorded::jacobian(&leaves, &parameters)?;
            let values: Vec<f64> = parameters.iter().map(Scalar::value).collect();
            if values
                .iter()
                .chain(jacobian.iter().flatten())
                .any(|value| !value.is_finite())
            {
                return Err(QSError::EvaluationErr(
                    "nonfinite compiled XVA setup coefficient or risk Jacobian".into(),
                ));
            }
            let mut active = Vec::new();
            let positions: Vec<_> = jacobian
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    if row.iter().any(|derivative| *derivative != 0.0) {
                        let position = active.len();
                        active.push(index);
                        Some(position)
                    } else {
                        None
                    }
                })
                .collect();
            let numerical_market = market.map_parameters(|value| value.value());
            let mut samples: Vec<f64> = active.iter().map(|&index| values[index]).collect();
            let active_count = samples.len();
            let mut positions = positions;
            for (index, position) in positions.iter_mut().enumerate() {
                if position.is_none() {
                    *position = Some(samples.len());
                    samples.push(values[index]);
                }
            }
            let coefficient_count = samples.len();
            samples.resize(coefficient_count + market.draw_count(), 0.0);
            let program = CompiledProgram::trace_reusing(
                &samples,
                active_count,
                previous.map(|p| &p.program),
                |inputs| -> Result<_> {
                    let coefficients: Vec<_> = positions
                        .iter()
                        .zip(&values)
                        .map(|(position, &value)| {
                            position
                                .map_or_else(|| Recorded::from(value), |position| inputs[position])
                        })
                        .collect();
                    let scenario = numerical_market
                        .replay(&coefficients[..market_count], &inputs[coefficient_count..])?;
                    let mut observations = Vec::new();
                    let mut contributions = Vec::new();
                    let mut total = Recorded::from(0.0);
                    for (ns, name) in netting_sets.iter().enumerate() {
                        let mut npvs = vec![Recorded::from(0.0); dates.len()];
                        for (date, &evaluation_date) in dates.iter().enumerate() {
                            for claim in trades[name]
                                .iter()
                                .filter(|claim| claim.payment_date() > evaluation_date)
                            {
                                let index = claim.idx().ok_or_else(|| {
                                    QSError::EvaluationErr("unindexed compiled XVA claim".into())
                                })?;
                                let response = scenario
                                    .get(date)
                                    .and_then(|row| row.get(index))
                                    .ok_or_else(|| {
                                        QSError::EvaluationErr(
                                            "missing compiled claim response".into(),
                                        )
                                    })?;
                                npvs[date] += claim.evaluate(response)?;
                            }
                        }
                        observations.extend_from_slice(&npvs);
                        let exposures = margins
                            .get(name)
                            .map(|margin| margin.adjust_path(&npvs, &scenario))
                            .transpose()?
                            .unwrap_or(npvs);
                        for (plan, range) in &aggregations[ns] {
                            let contribution =
                                plan.evaluate_with(&exposures, &coefficients[range.clone()]);
                            contributions.push(contribution);
                            total += contribution;
                        }
                    }
                    observations.extend(contributions);
                    Ok((total, observations))
                },
            )
            .map_err(|error| QSError::EvaluationErr(format!("compiled XVA capture: {error}")))?;
            let parameters = samples[..coefficient_count].to_vec();
            let jacobian = active
                .into_iter()
                .map(|index| jacobian[index].clone())
                .collect();
            Ok(Self {
                program,
                market: numerical_market,
                parameters,
                jacobian,
                labels,
                dates: dates.to_vec(),
                netting_sets,
                measures,
                paths: setup.n_paths(),
            })
        })
    }

    /// Whether the executable code is shared with a preceding market binding.
    #[must_use]
    pub fn shares_code_with(&self, previous: &Self) -> bool {
        self.program.shares_code_with(&previous.program)
    }

    /// Executes the native path kernel in parallel and returns cubes, adjustments and first risk.
    ///
    /// Each worker owns fixed input/output buffers. Random draws match the capture
    /// model's Sobol dimensions and antithetic pairing. Path normalization is
    /// already part of the aggregation plan and is not applied a second time.
    ///
    /// # Errors
    /// Rejects invalid random layouts, buffer mismatches and nonfinite native outputs.
    pub fn evaluate(&self) -> Result<ExposureResult> {
        let threads = rayon::current_num_threads().max(1);
        let size = self.paths.div_ceil(threads);
        let chunks: Vec<_> = (0..self.paths)
            .step_by(size)
            .map(|start| (start, (start + size).min(self.paths)))
            .collect();
        let chunks: Vec<Chunk> = chunks
            .into_par_iter()
            .map(|(start, end)| self.run_chunk(start, end))
            .collect::<Result<_>>()?;
        let mut cubes: Vec<Vec<Vec<f64>>> = self
            .netting_sets
            .iter()
            .map(|_| Vec::with_capacity(self.paths))
            .collect();
        let mut adjustments = vec![0.0; self.measures.len()];
        let mut gradient = vec![0.0; self.program.gradient_count()];
        for chunk in chunks {
            for (target, source) in cubes.iter_mut().zip(chunk.cubes) {
                target.extend(source);
            }
            for (target, source) in adjustments.iter_mut().zip(chunk.adjustments) {
                *target += source;
            }
            for (target, source) in gradient.iter_mut().zip(chunk.gradient) {
                *target += source;
            }
        }
        let mut sensitivities = BTreeMap::new();
        for (column, label) in self.labels.iter().enumerate() {
            let derivative = gradient
                .iter()
                .zip(&self.jacobian)
                .map(|(gradient, row)| gradient * row[column])
                .sum::<f64>();
            *sensitivities.entry(label.clone()).or_insert(0.0) += derivative;
        }
        let values = self
            .measures
            .iter()
            .zip(adjustments)
            .map(|((ns, measure), value)| XvaValue {
                netting_set: self.netting_sets[*ns].clone(),
                measure: measure.clone(),
                value,
            })
            .collect();
        Ok(ExposureResult {
            cubes: self
                .netting_sets
                .iter()
                .cloned()
                .zip(cubes)
                .map(|(trade_id, npvs)| NpvCube {
                    trade_id,
                    dates: self.dates.clone(),
                    npvs,
                })
                .collect(),
            xva_values: Some(values),
            sensitivities: Some(sensitivities.into_iter().collect()),
        })
    }

    /// Evaluates a deterministic range of paths into private cube and gradient buffers.
    fn run_chunk(&self, start: usize, end: usize) -> Result<Chunk> {
        let coefficients = self.parameters.len();
        let active = self.program.gradient_count();
        let mut inputs = vec![0.0; self.program.input_count()];
        inputs[..coefficients].copy_from_slice(&self.parameters);
        let mut output = vec![0.0; self.program.output_count()];
        let mut gradient = vec![0.0; active];
        let mut scratch = vec![0.0; self.market.factor_count()];
        let mut chunk = Chunk {
            cubes: self
                .netting_sets
                .iter()
                .map(|_| Vec::with_capacity(end - start))
                .collect(),
            adjustments: vec![0.0; self.measures.len()],
            gradient: vec![0.0; active],
        };
        for path in start..end {
            self.market
                .fill_draws(path, &mut inputs[coefficients..], &mut scratch)?;
            self.program
                .evaluate(&inputs, &mut output, &mut gradient)
                .map_err(|error| match error {
                    CompileError::ComparisonChanged => QSError::InvalidValueErr(
                        "simulation input changed a recorded validation comparison".into(),
                    ),
                    other => QSError::EvaluationErr(format!("compiled XVA execution: {other}")),
                })?;
            if output
                .iter()
                .chain(&gradient)
                .any(|value| !value.is_finite())
            {
                return Err(QSError::EvaluationErr(format!(
                    "nonfinite compiled XVA path {path}"
                )));
            }
            let date_count = self.dates.len();
            for (ns, cube) in chunk.cubes.iter_mut().enumerate() {
                cube.push(output[ns * date_count..(ns + 1) * date_count].to_vec());
            }
            for (target, &value) in chunk
                .adjustments
                .iter_mut()
                .zip(&output[self.netting_sets.len() * date_count..])
            {
                *target += value;
            }
            for (target, &value) in chunk.gradient.iter_mut().zip(&gradient) {
                *target += value;
            }
        }
        Ok(chunk)
    }
}

/// Exact current claim/CSA binding identity for supported built-in strategies.
/// Unsupported strategies use the existing engine path instead of a stale cache.
#[must_use]
pub fn binding_key<S: BuildHasher>(sets: &HashMap<String, NettingSet, S>) -> Option<String> {
    let mut ordered = sets.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(id, _)| *id);
    let mut result = String::new();
    for (id, set) in ordered {
        let _ = write!(result, "{id:?}:{:?}:", set.csa_terms());
        for claim in set.claims() {
            result.push_str(&claim_key(claim)?);
        }
    }
    Some(result)
}

/// Encodes a supported claim's structure for prepared-program binding invalidation.
fn claim_key(claim: &ContingentClaim) -> Option<String> {
    let strategy = match claim.evaluation_strategy() {
        ClaimEvaluationStrategy::Deterministic { amount } => format!("det:{:x}", amount.to_bits()),
        ClaimEvaluationStrategy::LinearRate {
            spread,
            day_counter,
        } => format!("linear:{:x}:{day_counter:?}", spread.to_bits()),
        ClaimEvaluationStrategy::NonLinearRate {
            spread,
            day_counter,
            strike,
            payoff_ops,
        } => format!(
            "nonlinear:{:x}:{day_counter:?}:{:x}:{payoff_ops:?}",
            spread.to_bits(),
            strike.to_bits()
        ),
        ClaimEvaluationStrategy::SpotPayoff {
            observation_date,
            strike,
            payoff_ops,
        } => format!(
            "spot:{observation_date:?}:{:x}:{payoff_ops:?}",
            strike.to_bits()
        ),
        ClaimEvaluationStrategy::ExerciseContingent {
            exercise_date,
            exercise_group,
            inner,
        } => format!(
            "exercise:{exercise_date:?}:{exercise_group}:{}",
            claim_key(inner)?
        ),
        ClaimEvaluationStrategy::PathDependent { .. }
        | ClaimEvaluationStrategy::Scripted { .. } => return None,
    };
    Some(format!(
        "{:?}:{:?}:{:?}:{:?}:{strategy};",
        (
            claim.trade_id(),
            claim.leg_id(),
            claim.idx(),
            claim.payment_date(),
            claim.fixing_date()
        ),
        (
            claim.accrual_start(),
            claim.accrual_end(),
            claim.currency(),
            claim.foreign_currency()
        ),
        (
            claim.notional().to_bits(),
            claim.side(),
            claim.index(),
            claim.realized_fixing().map(f64::to_bits)
        ),
        claim.partial_fixing().map(|partial| (
            partial.realized_accrual_factor.to_bits(),
            partial.original_accrual_start
        ))
    ))
}

//! Monte Carlo exposure evaluator.
//!
//! [`ExposureEvaluator`] computes per-trade NPV cubes and optionally
//! XVA values with sensitivities via the Savine parallel AAD pattern.
//!
//! Two modes:
//! - `evaluate` -- generic `T`, returns NPV cubes only.
//! - `evaluate_with_xva` -- `DualFwd`-only free function, returns cubes
//!   plus XVA values and sensitivities.

use std::collections::HashMap;

use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    ad::{dual::DualFwd, scalar::Scalar, tape::Tape},
    math::solvers::solvertraits::Matrix,
    time::date::Date,
    utils::errors::Result,
    xva::{
        aggregator::PfeAggregatorFactory, contigentclaim::ContingentClaim,
        visitors::marketmodel::MarketModel,
    },
};

/// Per-trade NPV cube: `npvs[path][date]`.
pub struct NpvCube {
    pub trade_id: String,
    pub dates: Vec<Date>,
    /// `npvs[path][date]` -- each inner `Vec` has length `dates.len()`.
    pub npvs: Matrix<f64>,
}

impl NpvCube {
    /// Expected Positive Exposure at each date, averaged over paths.
    #[must_use]
    pub fn epe(&self) -> Vec<f64> {
        let n_dates = self.dates.len();
        let n_paths = self.npvs.len();
        if n_paths == 0 {
            return vec![0.0; n_dates];
        }
        let inv_n = 1.0 / f64::from(u32::try_from(n_paths).unwrap_or(u32::MAX));
        (0..n_dates)
            .map(|d| {
                let sum: f64 = self.npvs.iter().map(|path| path[d].max(0.0)).sum();
                sum * inv_n
            })
            .collect()
    }

    /// Expected Negative Exposure at each date, averaged over paths.
    #[must_use]
    pub fn ene(&self) -> Vec<f64> {
        let n_dates = self.dates.len();
        let n_paths = self.npvs.len();
        if n_paths == 0 {
            return vec![0.0; n_dates];
        }
        let inv_n = 1.0 / f64::from(u32::try_from(n_paths).unwrap_or(u32::MAX));
        (0..n_dates)
            .map(|d| {
                let sum: f64 = self.npvs.iter().map(|path| path[d].min(0.0)).sum();
                sum * inv_n
            })
            .collect()
    }

    /// Expected Exposure (unconditional mean) at each date, averaged over paths.
    #[must_use]
    pub fn ee(&self) -> Vec<f64> {
        let n_dates = self.dates.len();
        let n_paths = self.npvs.len();
        if n_paths == 0 {
            return vec![0.0; n_dates];
        }
        let inv_n = 1.0 / f64::from(u32::try_from(n_paths).unwrap_or(u32::MAX));
        (0..n_dates)
            .map(|d| {
                let sum: f64 = self.npvs.iter().map(|path| path[d]).sum();
                sum * inv_n
            })
            .collect()
    }
}

/// Result of an exposure evaluation.
pub struct ExposureResult {
    /// Per-trade NPV cubes.
    pub cubes: Vec<NpvCube>,
    /// Optional XVA values (populated only by `evaluate_with_xva`).
    pub xva_values: Option<Vec<(String, f64)>>,
    /// Optional sensitivities (populated only by `evaluate_with_xva`).
    pub sensitivities: Option<Vec<(String, f64)>>,
}

/// Monte Carlo exposure evaluator.
///
/// Generic over `T: Scalar`. Use [`evaluate`](Self::evaluate) for cube-only
/// computation. For XVA values with sensitivities, use the free function
/// [`evaluate_with_xva`].
pub struct ExposureEvaluator<'a, T: Scalar> {
    dates: Vec<Date>,
    model: &'a dyn MarketModel<T>,
}

impl<'a, T: Scalar + 'static> ExposureEvaluator<'a, T> {
    /// Creates a new evaluator for the given dates and market model.
    pub fn new(dates: Vec<Date>, model: &'a dyn MarketModel<T>) -> Self {
        Self { dates, model }
    }

    /// Runs the Monte Carlo simulation and returns per-trade NPV cubes.
    ///
    /// # Errors
    /// Returns an error if claim evaluation fails for any path.
    pub fn evaluate(&self, trades: &HashMap<String, &[ContingentClaim]>) -> Result<ExposureResult> {
        let n_paths = self.model.n_paths();
        let n_dates = self.dates.len();
        let dates = &self.dates;
        let trade_ids: Vec<String> = trades.keys().cloned().collect();

        let cubes_map = (0..n_paths)
            .into_par_iter()
            .try_fold(
                || -> HashMap<String, Vec<Vec<f64>>> {
                    trade_ids
                        .iter()
                        .map(|id| (id.clone(), Vec::new()))
                        .collect()
                },
                |mut acc, i| -> Result<HashMap<String, Vec<Vec<f64>>>> {
                    if let Some(scenario) = self.model.generate_path(i) {
                        for (trade_id, claims) in trades {
                            let mut npvs = vec![0.0_f64; n_dates];
                            for (d, date_responses) in scenario.iter().enumerate() {
                                let eval_date = dates[d];
                                for claim in *claims {
                                    if claim.payment_date() > eval_date {
                                        if let Some(idx) = claim.idx() {
                                            let value =
                                                claim.evaluate::<T>(&date_responses[idx])?;
                                            npvs[d] += value.value();
                                        }
                                    }
                                }
                            }
                            if let Some(cube) = acc.get_mut(trade_id.as_str()) {
                                cube.push(npvs);
                            }
                        }
                    }
                    Ok(acc)
                },
            )
            .try_reduce(
                || {
                    trade_ids
                        .iter()
                        .map(|id| (id.clone(), Vec::new()))
                        .collect()
                },
                |mut a, b| {
                    for (id, paths) in b {
                        a.entry(id).or_default().extend(paths);
                    }
                    Ok(a)
                },
            )?;

        let cubes: Vec<NpvCube> = cubes_map
            .into_iter()
            .map(|(trade_id, npvs)| NpvCube {
                trade_id,
                dates: dates.clone(),
                npvs,
            })
            .collect();

        Ok(ExposureResult {
            cubes,
            xva_values: None,
            sensitivities: None,
        })
    }
}

/// Callback type for [`XvaModelSetup::with_model`].
///
/// Receives a thread-local `MarketModel<DualFwd>` and the tracked
/// `(label, DualFwd)` leaves, returns an arbitrary `Result<R>`.
pub type ModelCallback<'a, R> =
    dyn FnMut(&dyn MarketModel<DualFwd>, &[(String, DualFwd)]) -> Result<R> + 'a;

/// Per-thread model construction for the Savine parallel AAD loop.
///
/// Each rayon thread calls [`with_model`](Self::with_model) once. The
/// implementation creates owned `DualFwd` curves, calls
/// `put_pillars_on_tape`, builds a `MarketModel<DualFwd>` that borrows
/// those curves, and invokes the callback with the model and its tracked
/// leaves. Everything stays on the stack of `with_model`, so no
/// self-referential struct is needed.
pub trait XvaModelSetup: Send + Sync {
    /// Number of Monte Carlo paths to generate.
    fn n_paths(&self) -> usize;

    /// Build a per-thread model and invoke `callback` with it.
    ///
    /// Must be called after `start_recording_fwd` and before
    /// `set_mark_fwd`. The callback receives:
    /// - `model` -- a `MarketModel<DualFwd>` whose curves have pillars on
    ///   the current thread's tape.
    /// - `leaves` -- tracked `(label, DualFwd)` pairs whose adjoints carry
    ///   curve sensitivities after the backward pass.
    /// # Errors
    /// Returns an error if model construction or callback execution fails.
    fn with_model<R>(&self, dates: &[Date], callback: &mut ModelCallback<'_, R>) -> Result<R>;
}

/// Per-thread accumulation result for the parallel AAD loop.
struct ChunkResult {
    cubes: HashMap<String, Vec<Vec<f64>>>,
    xva_accums: Vec<f64>,
    sensitivities: Vec<(String, f64)>,
}

/// Parallel AAD evaluation.
///
/// Per rayon thread:
/// 1. `rewind_to_init_fwd` then `start_recording_fwd`.
/// 2. `model_setup.with_model()` creates per-thread model with pillar
///    leaves on tape (pre-mark).
/// 3. Inside the callback: create aggregators (pre-mark), `set_mark_fwd`,
///    then path loop: `rewind_to_mark` -> `generate_path` -> evaluate
///    claims -> aggregate -> `backward_to_mark`.
/// 4. `propagate_mark_to_start` then read pillar + aggregator leaf adjoints.
///
/// Returns NPV cubes, per-aggregator XVA values, and total sensitivities
/// summed across threads.
///
/// # Errors
/// Returns an error if model construction, claim evaluation, or adjoint
/// propagation fails for any thread or path.
pub fn evaluate_with_xva<S: XvaModelSetup, H: std::hash::BuildHasher + Sync>(
    dates: &[Date],
    trades: &HashMap<String, &[ContingentClaim], H>,
    factories: &[&dyn PfeAggregatorFactory],
    model_setup: &S,
) -> Result<ExposureResult> {
    let n_paths = model_setup.n_paths();
    let n_dates = dates.len();
    let n_aggs = factories.len();
    let trade_ids: Vec<String> = trades.keys().cloned().collect();

    // Build chunks (one per rayon thread)
    let n_threads = rayon::current_num_threads();
    let chunk_size = n_paths.div_ceil(n_threads);

    let chunks: Vec<(usize, usize)> = (0..n_threads)
        .map(|t| {
            let start = t * chunk_size;
            let end = (start + chunk_size).min(n_paths);
            (start, end)
        })
        .filter(|(s, e)| s < e)
        .collect();

    let chunk_results: Vec<ChunkResult> = chunks
        .into_par_iter()
        .map(|(start, end)| -> Result<ChunkResult> {
            Tape::rewind_to_init_fwd();
            Tape::start_recording_fwd();

            model_setup.with_model(dates, &mut |model, model_leaves| {
                let bundles: Vec<_> = factories
                    .iter()
                    .map(|f| f.create_aggregator(dates[0], dates))
                    .collect();

                Tape::set_mark_fwd();

                let mut xva_accums = vec![0.0_f64; n_aggs];
                let mut cubes: HashMap<String, Vec<Vec<f64>>> = trade_ids
                    .iter()
                    .map(|id| (id.clone(), Vec::new()))
                    .collect();

                for i in start..end {
                    Tape::rewind_to_mark_fwd();

                    if let Some(scenario) = model.generate_path(i) {
                        let mut total = DualFwd::zero();

                        for (ns_id, claims) in trades {
                            let mut ns_npvs = vec![DualFwd::zero(); n_dates];
                            let mut ns_npvs_f64 = vec![0.0_f64; n_dates];
                            for (d, date_responses) in scenario.iter().enumerate() {
                                let eval_date = dates[d];
                                for claim in *claims {
                                    if claim.payment_date() > eval_date {
                                        if let Some(idx) = claim.idx() {
                                            let value = claim.evaluate(&date_responses[idx])?;
                                            ns_npvs[d] = ns_npvs[d].add_val(value);
                                            ns_npvs_f64[d] += value.value();
                                        }
                                    }
                                }
                            }
                            if let Some(cube) = cubes.get_mut(ns_id.as_str()) {
                                cube.push(ns_npvs_f64);
                            }

                            // Per-netting-set aggregation.
                            for (a, bundle) in bundles.iter().enumerate() {
                                let c_p = bundle.aggregator.aggregate_path(&ns_npvs, dates);
                                xva_accums[a] += c_p.value();
                                total = total.add_val(c_p);
                            }
                        }

                        if total.is_on_tape() {
                            total.backward_to_mark()?;
                        }
                    }
                }

                Tape::propagate_mark_to_start_fwd()?;

                let mut sensitivities = Vec::new();
                for (label, leaf) in model_leaves {
                    if let Ok(adj) = leaf.adjoint() {
                        sensitivities.push((label.clone(), adj.value()));
                    }
                }
                for bundle in &bundles {
                    for (label, leaf) in &bundle.leaves {
                        if let Ok(adj) = leaf.adjoint() {
                            sensitivities.push((label.clone(), adj.value()));
                        }
                    }
                }

                Ok(ChunkResult {
                    cubes,
                    xva_accums,
                    sensitivities,
                })
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let result = reduce_chunk_results(&chunk_results, factories, dates, &trade_ids, n_aggs);

    Ok(result)
}

/// Merges per-thread chunk results into a single [`ExposureResult`].
fn reduce_chunk_results(
    chunk_results: &[ChunkResult],
    factories: &[&dyn PfeAggregatorFactory],
    dates: &[Date],
    trade_ids: &[String],
    n_aggs: usize,
) -> ExposureResult {
    let mut total_xva = vec![0.0_f64; n_aggs];
    let mut sens_map: HashMap<String, f64> = HashMap::new();
    let mut merged_cubes: HashMap<String, Vec<Vec<f64>>> = trade_ids
        .iter()
        .map(|id| (id.clone(), Vec::new()))
        .collect();

    for chunk in chunk_results {
        for (a, &v) in chunk.xva_accums.iter().enumerate() {
            total_xva[a] += v;
        }
        for (label, adj) in &chunk.sensitivities {
            *sens_map.entry(label.clone()).or_insert(0.0) += adj;
        }
        for (id, paths) in &chunk.cubes {
            if let Some(entry) = merged_cubes.get_mut(id.as_str()) {
                entry.extend(paths.iter().cloned());
            }
        }
    }

    let xva_values: Vec<(String, f64)> = factories
        .iter()
        .enumerate()
        .map(|(a, f)| (f.name().to_string(), total_xva[a]))
        .collect();

    let sensitivities: Vec<(String, f64)> = sens_map.into_iter().collect();

    let cubes: Vec<NpvCube> = merged_cubes
        .into_iter()
        .map(|(trade_id, npvs)| NpvCube {
            trade_id,
            dates: dates.to_vec(),
            npvs,
        })
        .collect();

    ExposureResult {
        cubes,
        xva_values: Some(xva_values),
        sensitivities: Some(sensitivities),
    }
}

#[cfg(feature = "plot")]
impl crate::utils::plot::Plot for NpvCube {
    #[allow(clippy::too_many_lines)]
    fn plot(&self, path: &str) -> crate::utils::errors::Result<()> {
        use crate::time::daycounter::DayCounter;
        use crate::utils::errors::QSError;
        use crate::utils::plot::plotting::{
            AreaSeries, BitMapBackend, ChartBuilder, Color, IntoDrawingArea, IntoFont, LineSeries,
            PathElement, SeriesLabelPosition, BG_COLOR, BLACK, EE_COLOR, ENE_COLOR, EPE_COLOR,
            GRID_COLOR, ZERO_LINE_COLOR,
        };

        let dc = DayCounter::Actual365;
        let ref_date = self.dates[0];
        let epe = self.epe();
        let ene = self.ene();
        let ee = self.ee();

        // Find the last date with non-zero exposure, then include one more
        // point so the plot shows the drop to zero at expiry.
        let last_active = epe
            .iter()
            .zip(ene.iter())
            .zip(ee.iter())
            .rposition(|((e, n), ee)| e.abs() > 1e-12 || n.abs() > 1e-12 || ee.abs() > 1e-12)
            .map_or(self.dates.len(), |i| (i + 2).min(self.dates.len()))
            .min(self.dates.len());

        let dates = &self.dates[..last_active];
        let times: Vec<f64> = dates
            .iter()
            .map(|d| dc.year_fraction(ref_date, *d))
            .collect();
        let epe = &epe[..last_active];
        let ene = &ene[..last_active];
        let ee = &ee[..last_active];

        let all_vals: Vec<f64> = epe
            .iter()
            .chain(ene.iter())
            .chain(ee.iter())
            .copied()
            .collect();
        let y_min_raw = all_vals.iter().copied().fold(f64::MAX, f64::min);
        let y_max_raw = all_vals.iter().copied().fold(f64::MIN, f64::max);

        let margin = ((y_max_raw - y_min_raw).abs() * 0.10).max(1.0);
        let y_min = y_min_raw - margin;
        let y_max = y_max_raw + margin;
        let t_max = times.last().copied().unwrap_or(1.0);

        let root = BitMapBackend::new(path, (1024, 576)).into_drawing_area();
        root.fill(&BG_COLOR)
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;

        let mut chart = ChartBuilder::on(&root)
            .caption(
                format!("Exposure Profile - {}", self.trade_id),
                ("sans-serif", 24).into_font().color(&BLACK),
            )
            .margin(20)
            .x_label_area_size(45)
            .y_label_area_size(75)
            .build_cartesian_2d(0.0..t_max, y_min..y_max)
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;

        chart
            .configure_mesh()
            .x_desc("Time (years)")
            .y_desc("Exposure")
            .axis_desc_style(("sans-serif", 16))
            .label_style(("sans-serif", 13))
            .light_line_style(GRID_COLOR)
            .bold_line_style(GRID_COLOR.mix(0.6))
            .draw()
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;

        chart
            .draw_series(LineSeries::new(
                [(0.0, 0.0), (t_max, 0.0)],
                ZERO_LINE_COLOR.stroke_width(1),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;

        chart
            .draw_series(AreaSeries::new(
                times.iter().zip(epe.iter()).map(|(&t, &v)| (t, v)),
                0.0,
                EPE_COLOR.mix(0.15),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;
        chart
            .draw_series(LineSeries::new(
                times.iter().zip(epe.iter()).map(|(&t, &v)| (t, v)),
                EPE_COLOR.stroke_width(2),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?
            .label("EPE")
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 18, y)], EPE_COLOR.stroke_width(2))
            });

        chart
            .draw_series(AreaSeries::new(
                times.iter().zip(ene.iter()).map(|(&t, &v)| (t, v)),
                0.0,
                ENE_COLOR.mix(0.15),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;
        chart
            .draw_series(LineSeries::new(
                times.iter().zip(ene.iter()).map(|(&t, &v)| (t, v)),
                ENE_COLOR.stroke_width(2),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?
            .label("ENE")
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 18, y)], ENE_COLOR.stroke_width(2))
            });

        chart
            .draw_series(LineSeries::new(
                times.iter().zip(ee.iter()).map(|(&t, &v)| (t, v)),
                EE_COLOR.stroke_width(1),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?
            .label("EE")
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 18, y)], EE_COLOR.stroke_width(1))
            });

        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperRight)
            .margin(10)
            .background_style(BG_COLOR.mix(0.9))
            .border_style(GRID_COLOR)
            .label_font(("sans-serif", 14))
            .draw()
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;

        root.present()
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ad::dual::DualFwd,
        ad::scalar::Scalar,
        core::{collateral::SingleCurveCSADiscountPolicy, pillars::Pillars, trade::Side},
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        instruments::rates::{makeswap::MakeSwap, swap::SwapTrade},
        math::interpolation::interpolator::Interpolator,
        models::lgm::{lgmcomponents::LgmRateModel, lgmmarketmodel::LgmMarketModel},
        rates::{
            compounding::Compounding, interestrate::RateDefinition,
            yieldtermstructure::discounttermstructure::DiscountTermStructure,
        },
        time::{
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
            schedule::MakeSchedule,
        },
        xva::{
            aggregator::{CvaAggregator, CvaFactory, PfeAggregator, PfeAggregatorFactory},
            nettingset::NettingSet,
            visitors::preprocessorexecutor::PreprocessorExecutor,
        },
    };

    fn make_flat_curve(ref_date: Date, rate: f64) -> (Vec<Date>, Vec<f64>) {
        let dc = DayCounter::Actual365;
        let mut dates = vec![ref_date];
        let mut dfs = vec![1.0_f64];
        for y in 1..=5 {
            let d = ref_date.advance(y, TimeUnit::Years);
            let t = dc.year_fraction(ref_date, d);
            dates.push(d);
            dfs.push((-rate * t).exp());
        }
        (dates, dfs)
    }

    struct TestModelSetup {
        curve_dates: Vec<Date>,
        discount_factors: Vec<f64>,
        lambda: f64,
        sigma: f64,
        n_paths: usize,
        seed: u64,
        ref_date: Date,
        dc: DayCounter,
        requests: Vec<crate::xva::visitors::preprocessorexecutor::SimulationRequest>,
    }

    impl XvaModelSetup for TestModelSetup {
        fn n_paths(&self) -> usize {
            self.n_paths
        }

        fn with_model<R>(&self, dates: &[Date], callback: &mut ModelCallback<'_, R>) -> Result<R> {
            let dfs: Vec<DualFwd> = self
                .discount_factors
                .iter()
                .map(|&v| DualFwd::scalar(v))
                .collect();

            let n_inner = self.discount_factors.len() - 1;
            let pillar_values: Vec<DualFwd> = self.discount_factors[1..]
                .iter()
                .map(|&v| DualFwd::scalar(v))
                .collect();
            let pillar_labels: Vec<String> =
                (0..n_inner).map(|i| format!("DF_{}", i + 1)).collect();

            let mut curve = DiscountTermStructure::<DualFwd>::new(
                self.curve_dates.clone(),
                dfs,
                self.dc,
                Interpolator::LogLinear,
                true,
            )
            .unwrap()
            .with_pillar_values(pillar_values)
            .unwrap()
            .with_pillar_labels(pillar_labels)
            .unwrap();

            curve.put_pillars_on_tape();

            let leaves: Vec<(String, DualFwd)> = curve
                .pillars()
                .unwrap_or_default()
                .into_iter()
                .map(|(label, &val)| (label, val))
                .collect();

            let rate_model = LgmRateModel::new(
                DualFwd::scalar(self.lambda),
                DualFwd::scalar(self.sigma),
                &curve,
            );

            let mut model =
                LgmMarketModel::new(Currency::USD, MarketIndex::SOFR, self.ref_date, self.dc)
                    .with_n_paths(self.n_paths)
                    .with_seed(self.seed);

            model.add_curve_model(MarketIndex::SOFR, rate_model);
            model.set_evaluation_dates(dates.to_vec());
            model.set_requests(self.requests.clone());

            callback(&model, &leaves)
        }
    }
    struct TestData {
        ref_date: Date,
        dc: DayCounter,
        curve_dates: Vec<Date>,
        discount_factors: Vec<f64>,
        claims: Vec<crate::xva::contigentclaim::ContingentClaim>,
        requests: Vec<crate::xva::visitors::preprocessorexecutor::SimulationRequest>,
        sim_dates: Vec<Date>,
    }

    fn setup() -> TestData {
        let dc = DayCounter::Actual365;
        let ref_date = Date::new(2025, 1, 15);
        let (curve_dates, discount_factors) = make_flat_curve(ref_date, 0.04);

        let swap = MakeSwap::<f64>::default()
            .with_identifier("USD_IRS_5Y".to_string())
            .with_start_date(ref_date)
            .with_maturity_date(ref_date.advance(5, TimeUnit::Years))
            .with_fixed_rate(0.038)
            .with_notional(10_000_000.0)
            .with_rate_definition(RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Simple,
                Frequency::Semiannual,
            ))
            .with_currency(Currency::USD)
            .with_market_index(MarketIndex::SOFR)
            .with_side(Side::LongReceive)
            .with_fixed_leg_frequency(Frequency::Quarterly)
            .with_floating_leg_frequency(Frequency::Semiannual)
            .build()
            .unwrap();
        let irs_trade = SwapTrade::new(swap, ref_date, 10_000_000.0, Side::LongReceive);
        let claims = irs_trade.into_contingent_claims().unwrap();

        let discount_policy = SingleCurveCSADiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let mut ns = NettingSet::new(claims, Box::new(discount_policy));
        let mut inspector = PreprocessorExecutor::new();
        inspector.visit(std::iter::once(&mut ns));
        let requests = inspector.requests().to_vec();
        let claims = ns.into_claims();

        let max_maturity = ref_date.advance(5, TimeUnit::Years);
        let schedule = MakeSchedule::new(ref_date, max_maturity)
            .with_frequency(Frequency::Quarterly)
            .build()
            .unwrap();
        let sim_dates = schedule.dates().clone();

        TestData {
            ref_date,
            dc,
            curve_dates,
            discount_factors,
            claims,
            requests,
            sim_dates,
        }
    }

    fn build_f64_curve(
        curve_dates: &[Date],
        dfs: &[f64],
        dc: DayCounter,
        bump_index: Option<usize>,
        bump: f64,
    ) -> DiscountTermStructure<f64> {
        let mut dfs = dfs.to_vec();
        if let Some(j) = bump_index {
            dfs[j + 1] += bump;
        }
        DiscountTermStructure::<f64>::new(
            curve_dates.to_vec(),
            dfs,
            dc,
            Interpolator::LogLinear,
            true,
        )
        .unwrap()
    }

    fn compute_cva_from_cube(
        cube: &NpvCube,
        credit_spread: f64,
        recovery: f64,
        n_paths: usize,
        ref_date: Date,
        dates: &[Date],
    ) -> f64 {
        let cva_agg = CvaAggregator::<f64>::new(credit_spread, recovery, n_paths, ref_date, dates);
        cube.npvs
            .iter()
            .map(|npvs| cva_agg.aggregate_path(npvs, dates))
            .sum::<f64>()
    }

    fn compute_cva_f64(
        td: &TestData,
        bump_index: Option<usize>,
        bump: f64,
        credit_spread: f64,
        recovery: f64,
        n_paths: usize,
    ) -> f64 {
        let curve = build_f64_curve(
            &td.curve_dates,
            &td.discount_factors,
            td.dc,
            bump_index,
            bump,
        );
        let rate_model = LgmRateModel::new(0.05_f64, 0.01_f64, &curve);
        let mut model = LgmMarketModel::new(Currency::USD, MarketIndex::SOFR, td.ref_date, td.dc)
            .with_n_paths(n_paths)
            .with_seed(42);
        model.add_curve_model(MarketIndex::SOFR, rate_model);
        model.set_evaluation_dates(td.sim_dates.clone());
        model.set_requests(td.requests.clone());

        let evaluator = ExposureEvaluator::<f64>::new(td.sim_dates.clone(), &model);
        let mut trades_map = HashMap::new();
        trades_map.insert("USD_IRS_5Y".to_string(), td.claims.as_slice());
        let result = evaluator.evaluate(&trades_map).unwrap();

        compute_cva_from_cube(
            &result.cubes[0],
            credit_spread,
            recovery,
            n_paths,
            td.ref_date,
            &td.sim_dates,
        )
    }

    fn run_aad(td: &TestData, n_paths: usize, credit_spread: f64, recovery: f64) -> ExposureResult {
        let model_setup = TestModelSetup {
            curve_dates: td.curve_dates.clone(),
            discount_factors: td.discount_factors.clone(),
            lambda: 0.05,
            sigma: 0.01,
            n_paths,
            seed: 42,
            ref_date: td.ref_date,
            dc: td.dc,
            requests: td.requests.clone(),
        };
        let cva_factory = CvaFactory {
            credit_spread,
            recovery,
            n_paths,
        };
        let factories: Vec<&dyn PfeAggregatorFactory> = vec![&cva_factory];
        let mut trades: HashMap<String, &[_]> = HashMap::new();
        trades.insert("USD_IRS_5Y".to_string(), td.claims.as_slice());
        evaluate_with_xva(&td.sim_dates, &trades, &factories, &model_setup).unwrap()
    }

    #[test]
    fn cva_credit_sensitivities_vs_bump() {
        let td = setup();
        let n_paths: usize = 1_000;
        let credit_spread = 0.01;
        let recovery = 0.40;

        let result = run_aad(&td, n_paths, credit_spread, recovery);

        let aad_cva = result
            .xva_values
            .as_ref()
            .unwrap()
            .iter()
            .find(|(n, _)| n == "CVA")
            .unwrap()
            .1;
        let aad_sens: HashMap<String, f64> = result
            .sensitivities
            .as_ref()
            .unwrap()
            .iter()
            .cloned()
            .collect();

        let cube = &result.cubes[0];
        let base_cva = compute_cva_from_cube(
            cube,
            credit_spread,
            recovery,
            n_paths,
            td.ref_date,
            &td.sim_dates,
        );
        let cva_err = ((base_cva - aad_cva) / aad_cva).abs();
        assert!(
            cva_err < 1e-6,
            "Base CVA mismatch: AAD={aad_cva}, Cube={base_cva}"
        );

        let h = 1e-5;
        let cva_cs_up = compute_cva_from_cube(
            cube,
            credit_spread + h,
            recovery,
            n_paths,
            td.ref_date,
            &td.sim_dates,
        );
        let cva_cs_dn = compute_cva_from_cube(
            cube,
            credit_spread - h,
            recovery,
            n_paths,
            td.ref_date,
            &td.sim_dates,
        );
        let bump_cs = (cva_cs_up - cva_cs_dn) / (2.0 * h);
        let aad_cs = *aad_sens.get("CVA.credit_spread").unwrap_or(&0.0);

        let cva_rec_up = compute_cva_from_cube(
            cube,
            credit_spread,
            recovery + h,
            n_paths,
            td.ref_date,
            &td.sim_dates,
        );
        let cva_rec_dn = compute_cva_from_cube(
            cube,
            credit_spread,
            recovery - h,
            n_paths,
            td.ref_date,
            &td.sim_dates,
        );
        let bump_rec = (cva_rec_up - cva_rec_dn) / (2.0 * h);
        let aad_rec = *aad_sens.get("CVA.recovery").unwrap_or(&0.0);

        for (label, aad_val, bump_val) in [
            ("CVA.credit_spread", aad_cs, bump_cs),
            ("CVA.recovery", aad_rec, bump_rec),
        ] {
            let rel_err = if aad_val.abs() > 1e-10 {
                ((bump_val - aad_val) / aad_val).abs()
            } else {
                0.0
            };
            assert!(
                rel_err < 0.02,
                "{label}: AAD={aad_val:.4}, Bump={bump_val:.4}, rel err={:.4}%",
                rel_err * 100.0
            );
        }
    }

    #[test]
    fn cva_curve_sensitivities_vs_bump() {
        let td = setup();
        let n_paths: usize = 1_000;
        let credit_spread = 0.01;
        let recovery = 0.40;

        let result = run_aad(&td, n_paths, credit_spread, recovery);

        let aad_sens: HashMap<String, f64> = result
            .sensitivities
            .as_ref()
            .unwrap()
            .iter()
            .cloned()
            .collect();

        let h = 1e-4;
        let n_pillars = td.discount_factors.len() - 1;

        let mut max_rel_err = 0.0_f64;
        for j in 0..n_pillars {
            let label = format!("DF_{}", j + 1);
            let aad_val = *aad_sens.get(&label).unwrap_or(&0.0);
            if aad_val.abs() < 1e-6 {
                continue;
            }

            let cva_up = compute_cva_f64(&td, Some(j), h, credit_spread, recovery, n_paths);
            let cva_dn = compute_cva_f64(&td, Some(j), -h, credit_spread, recovery, n_paths);
            let bump_val = (cva_up - cva_dn) / (2.0 * h);

            let rel_err = if aad_val.abs() > 1e-10 {
                ((bump_val - aad_val) / aad_val).abs()
            } else {
                0.0
            };

            max_rel_err = max_rel_err.max(rel_err);
        }

        assert!(
            max_rel_err < 0.05,
            "Max relative error {:.2}% exceeds 5% tolerance",
            max_rel_err * 100.0
        );
    }
}

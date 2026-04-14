//! XVA evaluator with optional Savine-style parallel AAD for sensitivities.
//!
//! Two entry points:
//!
//! * [`XvaEvaluator::evaluate_cube`] -- generic `T`, stores per-trade NPV
//!   matrices, no tape involved.
//! * [`evaluate_xva`] -- free function, `DualFwd`-only, runs the Savine
//!   per-thread AAD loop with pluggable aggregator factories, returns XVA
//!   values and dXVA/dparam.
//!
//! The Savine loop needs a per-thread model whose curves have pillars on
//! the thread-local tape.  Because `LgmRateModel` *borrows* its curve,
//! we cannot return both curve and model from a factory.  Rust forbids
//! self-referential structs.  [`XvaModelSetup::with_model`] solves this
//! with a callback: the implementation creates curves and model on its
//! stack, then hands them to the callback which runs the full path loop
//! within that scope.

use std::collections::HashMap;

use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    ad::{dual::DualFwd, scalar::Scalar, tape::Tape},
    math::solvers::solvertraits::Matrix,
    time::date::Date,
    utils::errors::Result,
    xva::{
        contigentclaim::ContingentClaim, va::aggregator::PfeAggregatorFactory,
        visitors::marketmodel::MarketModel,
    },
};

/// Result of an XVA evaluation.
pub struct XvaResult {
    /// Per-aggregator name and f64 value (e.g. `"CVA"` -> 4475.0).
    pub xva_values: Vec<(String, f64)>,
    /// dXVA/dparam -- total sensitivities, summed across threads.
    pub sensitivities: Vec<(String, f64)>,
    /// Optional per-trade NPV cubes (cube-only mode or when requested).
    pub cubes: Option<Vec<NpvCube>>,
}

/// Per-trade NPV cube: `npvs[path][date]`.
pub struct NpvCube {
    pub trade_id: String,
    pub dates: Vec<Date>,
    /// `npvs[path][date]` -- each inner `Vec` has length `dates.len()`.
    pub npvs: Matrix<f64>,
}

/// Monte Carlo XVA evaluator.
///
/// Generic over `T: Scalar`:
/// * With `T = f64`  -> use [`evaluate_cube`](Self::evaluate_cube).
/// * With `T = DualFwd` -> use [`evaluate_xva`](Self::evaluate_xva) for
///   values + tape-based sensitivities.
pub struct XvaEvaluator<'a, T: Scalar> {
    dates: Vec<Date>,
    model: &'a dyn MarketModel<T>,
}

impl<'a, T: Scalar + 'static> XvaEvaluator<'a, T> {
    /// Creates a new evaluator for the given dates and market model.
    pub fn new(dates: Vec<Date>, model: &'a dyn MarketModel<T>) -> Self {
        Self { dates, model }
    }
}

impl<'a, T: Scalar + 'static> XvaEvaluator<'a, T> {
    /// Runs the Monte Carlo simulation and returns per-trade NPV cubes.
    ///
    /// No tape is used; the returned [`XvaResult`] has empty `xva_values`
    /// and `sensitivities`.
    pub fn evaluate_cube(&self, trades: &HashMap<String, &[ContingentClaim]>) -> Result<XvaResult> {
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

        Ok(XvaResult {
            xva_values: vec![],
            sensitivities: vec![],
            cubes: Some(cubes),
        })
    }
}

/// Per-thread model construction for the Savine parallel AAD loop.
///
/// Each rayon thread calls [`with_model`](Self::with_model) once.  The
/// implementation creates owned `DualFwd` curves, calls
/// `put_pillars_on_tape`, builds an `LgmMarketModel` (or any
/// `MarketModel<DualFwd>`) that borrows those curves, and invokes the
/// `callback` with the model and its tracked leaves.  Everything stays
/// on the stack of `with_model`, so no self-referential struct is needed.
pub trait XvaModelSetup: Send + Sync {
    /// Number of Monte Carlo paths to generate.
    fn n_paths(&self) -> usize;

    /// Build a per-thread model and invoke `callback` with it.
    ///
    /// Must be called **after** `start_recording_fwd` and **before**
    /// `set_mark_fwd`.  The callback receives:
    /// * `model` -- a `MarketModel<DualFwd>` whose curves have pillars on
    ///   the current thread's tape.
    /// * `leaves` -- tracked `(label, DualFwd)` pairs whose adjoints carry
    ///   curve sensitivities after the backward pass.
    fn with_model<R>(
        &self,
        dates: &[Date],
        callback: &mut dyn FnMut(&dyn MarketModel<DualFwd>, &[(String, DualFwd)]) -> R,
    ) -> R;
}

/// Savine-style parallel AAD evaluation.
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
/// Returns per-aggregator XVA values and total sensitivities summed
/// across threads.
pub fn evaluate_xva<S: XvaModelSetup>(
    dates: &[Date],
    trades: &HashMap<String, &[ContingentClaim]>,
    factories: &[&dyn PfeAggregatorFactory],
    model_setup: &S,
) -> Result<XvaResult> {
    let n_paths = model_setup.n_paths();
    let n_dates = dates.len();
    let n_aggs = factories.len();

    // Build chunks (one per rayon thread)
    let n_threads = rayon::current_num_threads();
    let chunk_size = (n_paths + n_threads - 1) / n_threads;

    let chunks: Vec<(usize, usize)> = (0..n_threads)
        .map(|t| {
            let start = t * chunk_size;
            let end = (start + chunk_size).min(n_paths);
            (start, end)
        })
        .filter(|(s, e)| s < e)
        .collect();

    // Per-chunk output
    struct ChunkResult {
        xva_accums: Vec<f64>,
        sensitivities: Vec<(String, f64)>,
    }

    // Parallel execution
    let chunk_results: Vec<ChunkResult> = chunks
        .into_par_iter()
        .map(|(start, end)| -> Result<ChunkResult> {
            // 1. Fresh tape
            Tape::rewind_to_init_fwd();
            Tape::start_recording_fwd();

            // 2. Per-thread model via callback (pre-mark leaves)
            model_setup.with_model(dates, &mut |model, model_leaves| {
                // 3. Create per-thread aggregators (pre-mark leaves)
                let bundles: Vec<_> = factories
                    .iter()
                    .map(|f| f.create_aggregator(dates[0], dates))
                    .collect();

                // 4. Set mark -- everything above is pre-mark
                Tape::set_mark_fwd();

                // 5. Path loop
                let mut xva_accums = vec![0.0_f64; n_aggs];

                for i in start..end {
                    Tape::rewind_to_mark_fwd();

                    if let Some(scenario) = model.generate_path(i) {
                        // Portfolio NPVs per simulation date
                        let mut portfolio_npvs = vec![DualFwd::zero(); n_dates];

                        for (_trade_id, claims) in trades {
                            for (d, date_responses) in scenario.iter().enumerate() {
                                let eval_date = dates[d];
                                for claim in *claims {
                                    if claim.payment_date() > eval_date {
                                        if let Some(idx) = claim.idx() {
                                            let value = claim.evaluate(&date_responses[idx])?;
                                            portfolio_npvs[d] = portfolio_npvs[d].add_val(value);
                                        }
                                    }
                                }
                            }
                        }

                        // Aggregate all measures, sum into a single root
                        let mut total = DualFwd::zero();
                        for (a, bundle) in bundles.iter().enumerate() {
                            let c_p = bundle.aggregator.aggregate_path(&portfolio_npvs, dates);
                            xva_accums[a] += c_p.value();
                            total = total.add_val(c_p);
                        }

                        // Single backward pass to mark
                        if total.is_on_tape() {
                            total.backward_to_mark()?;
                        }
                    }
                }

                // 6. Propagate accumulated adjoints through pre-mark graph
                Tape::propagate_mark_to_start_fwd()?;

                // 7. Read leaf adjoints (curves + aggregators)
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
                    xva_accums,
                    sensitivities,
                })
            })
        })
        .collect::<Result<Vec<_>>>()?;

    // Reduce across chunks
    let mut total_xva = vec![0.0_f64; n_aggs];
    let mut sens_map: HashMap<String, f64> = HashMap::new();

    for chunk in &chunk_results {
        for (a, &v) in chunk.xva_accums.iter().enumerate() {
            total_xva[a] += v;
        }
        for (label, adj) in &chunk.sensitivities {
            *sens_map.entry(label.clone()).or_insert(0.0) += adj;
        }
    }

    let xva_values: Vec<(String, f64)> = factories
        .iter()
        .enumerate()
        .map(|(a, f)| (f.name().to_string(), total_xva[a]))
        .collect();

    let sensitivities: Vec<(String, f64)> = sens_map.into_iter().collect();

    Ok(XvaResult {
        xva_values,
        sensitivities,
        cubes: None,
    })
}

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        collateral::{DiscountPolicy, SingleCurveCSADiscountPolicy},
        marketdatahandling::discountrequest::DiscountRequest,
        pillars::Pillars,
        pricingcontext::PricingContext,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::Interpolator,
    models::lgm::{
        lgmcomponents::{LgmFxModel, LgmRateModel},
        lgmmarketmodel::LgmMarketModel,
    },
    rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
    time::{
        date::Date,
        daycounter::DayCounter,
        enums::{Frequency, TimeUnit},
        schedule::MakeSchedule,
    },
    utils::errors::{QSError, Result},
    xva::{
        contigentclaim::ContingentClaim,
        va::aggregator::{CvaFactory, PfeAggregatorFactory},
        visitors::{
            exposureevaluator::{evaluate_with_xva, ExposureResult, XvaModelSetup},
            inspector::SimulationRequest,
            marketmodel::MarketModel,
        },
    },
};

/// LGM model parameters for a single rate curve.
#[derive(Clone, Serialize, Deserialize)]
pub struct LgmModelConfig {
    pub market_index: MarketIndex,
    pub lambda: f64,
    pub sigma: f64,
}

/// FX model parameters for a single currency pair.
#[derive(Clone, Serialize, Deserialize)]
pub struct FxModelConfig {
    /// Foreign currency (domestic is always the engine's base currency).
    pub foreign_currency: Currency,
    /// FX volatility.
    pub fx_vol: f64,
    /// Correlation between domestic rate factor and FX spot.
    #[serde(default)]
    pub rho: f64,
}

/// Configuration for the XVA engine.
#[derive(Clone, Serialize, Deserialize)]
pub struct XvaEngineConfig {
    /// LGM model parameters, one per rate curve.
    pub model_configs: Vec<LgmModelConfig>,
    /// FX model parameters, one per foreign currency.
    #[serde(default)]
    pub fx_configs: Vec<FxModelConfig>,
    /// Number of Monte Carlo paths.
    pub n_paths: usize,
    /// RNG seed.
    pub seed: u64,
    /// Simulation frequency (e.g. Monthly, Quarterly).
    pub frequency: Frequency,
    /// Credit spread for CVA calculation.
    pub credit_spread: f64,
    /// Recovery rate for CVA calculation.
    pub recovery: f64,
}

// ---------------------------------------------------------------------------
// XvaEngine — high-level entry point
// ---------------------------------------------------------------------------

/// High-level XVA engine.
///
/// Takes a fully initialised [`PricingContext`] (with bootstrapped curves)
/// and an [`XvaEngineConfig`], then runs the Savine parallel AAD loop to
/// produce exposure cubes, XVA values, and sensitivities.
///
/// # Example
/// ```ignore
/// let mut ctx = PricingContext::new()
///     .with_quote_store(quotes)
///     .with_curve_configurations(curve_specs);
/// ctx.initialize()?;
///
/// let config = XvaEngineConfig { /* ... */ };
/// let engine = XvaEngine::new(&ctx, config)?;
/// let result = engine.run(&mut trades)?;
/// ```
pub struct XvaEngine {
    setup: InternalModelSetup,
    frequency: Frequency,
    credit_spread: f64,
    recovery: f64,
}

impl XvaEngine {
    /// Creates a new engine from an initialised [`PricingContext`].
    ///
    /// Snapshots the f64 curve data from every discount curve referenced
    /// in `config.model_configs`. The curves must already be bootstrapped
    /// in the context.
    ///
    /// # Errors
    /// Returns an error if a required discount curve is missing or has no nodes.
    pub fn new(context: &PricingContext, config: XvaEngineConfig) -> Result<Self> {
        let store = context.constructed_elements();

        let mut curves = HashMap::new();
        let mut model_configs = HashMap::new();

        for mc in &config.model_configs {
            let elem = store.discount_curve(&mc.market_index).ok_or_else(|| {
                QSError::NotFoundErr(format!(
                    "Discount curve not found for index {:?}",
                    mc.market_index
                ))
            })?;

            // Snapshot f64 data from the already-bootstrapped curve.
            let borrowed = elem.curve();
            let nodes = borrowed
                .nodes()
                .ok_or_else(|| QSError::NotFoundErr("Curve has no nodes".into()))?;
            let dates: Vec<Date> = nodes.iter().map(|(d, _)| *d).collect();
            let dfs: Vec<f64> = nodes.iter().map(|(_, v)| v.value()).collect();
            let pillar_labels = borrowed.pillar_labels().unwrap_or_default();
            let pillar_values: Vec<f64> = borrowed
                .pillars()
                .unwrap_or_default()
                .iter()
                .map(|(_, v)| v.value())
                .collect();
            let ift = borrowed.ift_sensitivities().map(|s| s.to_vec());
            let dc = borrowed.day_counter().unwrap_or(DayCounter::Actual365);

            curves.insert(
                mc.market_index.clone(),
                CurveSnapshot {
                    dates,
                    discount_factors: dfs,
                    day_counter: dc,
                    interpolator: Interpolator::LogLinear,
                    pillar_labels,
                    pillar_values,
                    ift_sensitivities: ift,
                },
            );
            model_configs.insert(mc.market_index.clone(), mc.clone());
        }

        // Snapshot FX spots from the FxStore.
        let mut fx_spots = HashMap::new();
        let fx_store = context.fx_store();
        let domestic = context.base_currency();
        for fx_cfg in &config.fx_configs {
            // Get rate as: 1 foreign = X domestic (e.g. 1 CLP = 0.00111 USD)
            let rate = fx_store
                .get_fx_rate(fx_cfg.foreign_currency, domestic)
                .map_err(|_| {
                    QSError::NotFoundErr(format!(
                        "FX spot not found for {}/{}",
                        fx_cfg.foreign_currency, domestic
                    ))
                })?;
            fx_spots.insert(fx_cfg.foreign_currency, rate.value());
        }

        Ok(Self {
            setup: InternalModelSetup {
                curves,
                model_configs,
                fx_configs: config.fx_configs,
                fx_spots,
                domestic_currency: domestic,
                domestic_index: context.base_index().clone(),
                reference_date: context.evaluation_date(),
                day_counter: DayCounter::Actual365,
                n_paths: config.n_paths,
                seed: config.seed,
                requests: Vec::new(), // filled in run()
            },
            frequency: config.frequency,
            credit_spread: config.credit_spread,
            recovery: config.recovery,
        })
    }

    /// Runs the full XVA pipeline.
    ///
    /// Assigns flat-vector indices to every claim, builds simulation dates,
    /// and runs the Savine parallel AAD loop.
    ///
    /// # Errors
    /// Returns an error if simulation or evaluation fails.
    pub fn run(
        &mut self,
        trades: &mut HashMap<String, Vec<ContingentClaim>>,
    ) -> Result<ExposureResult> {
        // 1. Assign indices and collect requests across all trades.
        let discount_policy = SingleCurveCSADiscountPolicy::new(
            self.setup.domestic_index.clone(),
            self.setup.domestic_currency,
        );
        let mut requests = Vec::new();
        let mut offset = 0_usize;
        for claims in trades.values_mut() {
            for claim in claims.iter_mut() {
                let mut req = claim.simulation_request();
                if let Ok(disc_idx) = discount_policy.accept(claim) {
                    req.discount_request =
                        Some(DiscountRequest::new(disc_idx, claim.payment_date()));
                }
                claim.set_idx(offset);
                requests.push(req);
                offset += 1;
            }
        }
        self.setup.requests = requests;

        // 2. Build simulation dates.
        let max_maturity = trades
            .values()
            .flat_map(|v| v.iter())
            .map(|c| c.payment_date())
            .max()
            .unwrap_or(self.setup.reference_date.advance(1, TimeUnit::Years));

        let schedule = MakeSchedule::new(self.setup.reference_date, max_maturity)
            .with_frequency(self.frequency)
            .build()?;
        let sim_dates = schedule.dates().clone();

        // 3. Aggregator factories.
        let cva_factory = CvaFactory {
            credit_spread: self.credit_spread,
            recovery: self.recovery,
            n_paths: self.setup.n_paths,
        };
        let factories: Vec<&dyn PfeAggregatorFactory> = vec![&cva_factory];

        // 4. Build trade slice map.
        let trade_slices: HashMap<String, &[ContingentClaim]> = trades
            .iter()
            .map(|(id, v)| (id.clone(), v.as_slice()))
            .collect();

        // 5. Run.
        evaluate_with_xva(&sim_dates, &trade_slices, &factories, &self.setup)
    }
}

/// Snapshot of f64 curve data extracted from the PricingContext.
/// Each rayon thread uses this to build a thread-local DualFwd curve.
#[derive(Clone)]
struct CurveSnapshot {
    dates: Vec<Date>,
    discount_factors: Vec<f64>,
    day_counter: DayCounter,
    interpolator: Interpolator,
    pillar_labels: Vec<String>,
    pillar_values: Vec<f64>,
    ift_sensitivities: Option<Vec<Vec<f64>>>,
}

impl CurveSnapshot {
    /// Build a `DiscountTermStructure<DualFwd>` on the current thread's tape.
    fn build_dualfwd_curve(&self) -> DiscountTermStructure<DualFwd> {
        let dfs: Vec<DualFwd> = self
            .discount_factors
            .iter()
            .map(|&v| DualFwd::scalar(v))
            .collect();
        let pvs: Vec<DualFwd> = self
            .pillar_values
            .iter()
            .map(|&v| DualFwd::scalar(v))
            .collect();

        let mut curve = DiscountTermStructure::<DualFwd>::new(
            self.dates.clone(),
            dfs,
            self.day_counter,
            self.interpolator,
            true,
        )
        .expect("CurveSnapshot: failed to create DualFwd curve")
        .with_pillar_values(pvs)
        .expect("CurveSnapshot: failed to set pillar values")
        .with_pillar_labels(self.pillar_labels.clone())
        .expect("CurveSnapshot: failed to set pillar labels");

        if let Some(ref sens) = self.ift_sensitivities {
            curve = curve.with_ift_sensitivities(sens.clone());
        }

        curve.put_pillars_on_tape();
        curve
    }
}

/// Internal model setup implementing `XvaModelSetup`.
struct InternalModelSetup {
    curves: HashMap<MarketIndex, CurveSnapshot>,
    model_configs: HashMap<MarketIndex, LgmModelConfig>,
    fx_configs: Vec<FxModelConfig>,
    fx_spots: HashMap<Currency, f64>,
    domestic_currency: Currency,
    domestic_index: MarketIndex,
    reference_date: Date,
    day_counter: DayCounter,
    n_paths: usize,
    seed: u64,
    requests: Vec<SimulationRequest>,
}

// Safety: all fields are owned plain data (Vec, HashMap, f64, etc.). No Rc/RefCell.
unsafe impl Send for InternalModelSetup {}
unsafe impl Sync for InternalModelSetup {}

impl XvaModelSetup for InternalModelSetup {
    fn n_paths(&self) -> usize {
        self.n_paths
    }

    fn with_model<R>(
        &self,
        dates: &[Date],
        callback: &mut dyn FnMut(&dyn MarketModel<DualFwd>, &[(String, DualFwd)]) -> R,
    ) -> R {
        // 1. Build DualFwd curves and collect leaves.
        let mut built_curves: Vec<(MarketIndex, DiscountTermStructure<DualFwd>)> = Vec::new();
        let mut all_leaves: Vec<(String, DualFwd)> = Vec::new();

        for (idx, snap) in &self.curves {
            let curve = snap.build_dualfwd_curve();
            let leaves: Vec<(String, DualFwd)> = curve
                .pillars()
                .unwrap_or_default()
                .into_iter()
                .map(|(label, &val)| (label, val))
                .collect();
            all_leaves.extend(leaves);
            built_curves.push((idx.clone(), curve));
        }

        // 2. Build rate models for curve_models (moved into the market model).
        //    Also build separate rate model instances for FX model references.
        let mut fx_rate_models: Vec<(MarketIndex, LgmRateModel<'_, DualFwd>)> = Vec::new();

        let mut model = LgmMarketModel::new(
            self.domestic_currency,
            self.domestic_index.clone(),
            self.reference_date,
            self.day_counter,
        )
        .with_n_paths(self.n_paths)
        .with_seed(self.seed);

        for (idx, curve) in &built_curves {
            let cfg = self
                .model_configs
                .get(idx)
                .expect("Model config missing for curve");
            let rate_model = LgmRateModel::new(
                DualFwd::scalar(cfg.lambda),
                DualFwd::scalar(cfg.sigma),
                curve,
            );
            model.add_curve_model(idx.clone(), rate_model);

            // If any FX config references this curve's currency, build an extra
            // rate model for the FX model to borrow.
            if !self.fx_configs.is_empty() {
                let fx_rate = LgmRateModel::new(
                    DualFwd::scalar(cfg.lambda),
                    DualFwd::scalar(cfg.sigma),
                    curve,
                );
                fx_rate_models.push((idx.clone(), fx_rate));
            }
        }

        // 3. Build FX models from the separate rate model instances.
        //    Find domestic and foreign rate models by index.
        let find_fx_rate =
            |idx: &MarketIndex| -> Option<usize> {
                fx_rate_models.iter().position(|(i, _)| i == idx)
            };

        for fx_cfg in &self.fx_configs {
            let dom_pos = find_fx_rate(&self.domestic_index)
                .expect("Domestic rate model not found for FX");
            // Find the foreign index by currency
            let foreign_index = self
                .model_configs
                .iter()
                .find(|(_, mc)| {
                    mc.market_index
                        .rate_index_details()
                        .map_or(false, |d| d.currency() == fx_cfg.foreign_currency)
                })
                .map(|(_, mc)| &mc.market_index)
                .expect("Foreign rate model not found for FX");
            let for_pos = find_fx_rate(foreign_index)
                .expect("Foreign rate model not found for FX");

            // SAFETY: dom_pos != for_pos (domestic != foreign currency).
            // We need two simultaneous immutable borrows from the Vec.
            let (dom_rate, for_rate) = if dom_pos < for_pos {
                let (left, right) = fx_rate_models.split_at(for_pos);
                (&left[dom_pos].1, &right[0].1)
            } else {
                let (left, right) = fx_rate_models.split_at(dom_pos);
                (&right[0].1, &left[for_pos].1)
            };

            let spot = *self
                .fx_spots
                .get(&fx_cfg.foreign_currency)
                .expect("FX spot missing for currency");

            let fx_model = LgmFxModel::new(
                dom_rate,
                for_rate,
                DualFwd::scalar(fx_cfg.fx_vol),
                DualFwd::scalar(spot),
                DualFwd::scalar(fx_cfg.rho),
            );
            model.add_fx_model(fx_cfg.foreign_currency, fx_model);
        }

        model.set_evaluation_dates(dates.to_vec());
        model.set_requests(self.requests.clone());

        callback(&model, &all_leaves)
    }
}

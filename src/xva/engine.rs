use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{pillars::Pillars, pricingcontext::PricingContext},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::Interpolator,
    models::{
        hullwhite::hullwhitemodel::HullWhite,
        lgm::{
            lgmcomponents::{LgmFxModel, LgmRateModel},
            lgmmarketmodel::LgmMarketModel,
        },
    },
    quotes::{fixingstore::FixingStore, quote::Level},
    rates::yieldtermstructure::{
        discounttermstructure::DiscountTermStructure,
        interestratestermstructure::InterestRatesTermStructure,
    },
    time::{
        date::Date,
        daycounter::DayCounter,
        enums::{Frequency, TimeUnit},
        schedule::MakeSchedule,
    },
    utils::errors::{QSError, Result},
    volatility::volatilitysource::VolatilitySourceConfiguration,
    xva::{
        aggregator::{
            CreditCurveCvaFactory, CvaFactory, FundingCurveFvaFactory, FvaFactory,
            PfeAggregatorFactory,
        },
        contigentclaim::ContingentClaim,
        csa::CsaTerms,
        nettingset::NettingSet,
        visitors::{
            exposureevaluator::{evaluate_with_xva, ExposureResult, ModelCallback, XvaModelSetup},
            fixingpreprocessor::FixingPreprocessor,
            marketmodel::MarketModel,
            preprocessorexecutor::{PreprocessorExecutor, SimulationRequest},
        },
    },
};

/// LGM model parameters for a single rate curve.
///
/// The short-rate volatility is either a flat `sigma` or a
/// [`VolatilitySourceConfiguration`]: `Constant`, or `Calibrated` against a
/// volatility surface (caplets) or cube (swaptions) constructed in the
/// pricing context. When both are set, `volatility` takes precedence.
///
/// Curves without dynamics of their own (e.g. FX-implied collateral curves
/// such as `Collateral(CLP, USD)`) must instead set [`Self::driver`].
#[derive(Clone, Serialize, Deserialize)]
pub struct LgmModelConfig {
    pub market_index: MarketIndex,
    /// Mean reversion. Required unless [`Self::driver`] is set.
    #[serde(default)]
    pub lambda: Option<f64>,
    /// Flat short-rate volatility. Ignored when [`Self::volatility`] is set.
    #[serde(default)]
    pub sigma: Option<f64>,
    /// Volatility source (`Constant` or `Calibrated` from a constructed
    /// surface/cube). Takes precedence over [`Self::sigma`].
    #[serde(default)]
    pub volatility: Option<VolatilitySourceConfiguration>,
    /// Rate model that drives this curve's dynamics.
    ///
    /// Use for curves that carry no volatility of their own, e.g. FX-implied
    /// collateral curves: under the standard deterministic cross-currency
    /// basis assumption, `Collateral(CLP, USD)` evolves with the CLP
    /// risk-free model (ICP) — its vol is implied by the driver's curve vol
    /// together with the FX vol, whose quanto effect is already carried by
    /// the driver's factor drift under the domestic measure.
    ///
    /// During simulation the curve's discount factors are reconstructed with
    /// the driver's simulated factor, mean reversion and sigma schedule, but
    /// from the curve's *own* initial term structure, so the time-0
    /// cross-currency basis is preserved and evolves deterministically.
    /// Mutually exclusive with `lambda`, `sigma` and `volatility`; the
    /// driver must itself be a non-derived model config.
    ///
    /// ```json
    /// { "market_index": { "Collateral": ["CLP", "USD"] }, "driver": "ICP" }
    /// ```
    #[serde(default)]
    pub driver: Option<MarketIndex>,
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
///
/// Contains only the simulation/model setup. Credit, funding and
/// collateral (CSA) parameters are per client and belong to each
/// [`NettingSet`]'s [`CsaTerms`].
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
}

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
    /// Snapshots of the bootstrapped credit (survival) curves, keyed by
    /// [`MarketIndex::Credit`]. Used to build per-counterparty CVA
    /// aggregators when a netting set's CSA references a credit curve.
    credit_curves: HashMap<MarketIndex, CreditCurveSnapshot>,
    /// Snapshots of every bootstrapped discount curve in the context, keyed
    /// by index. Used to derive per-counterparty funding spreads when a
    /// netting set's CSA references a funding curve.
    funding_curves: HashMap<MarketIndex, CurveSnapshot>,
}

impl XvaEngine {
    /// Creates a new engine from an initialised [`PricingContext`].
    ///
    /// Snapshots the f64 curve data from every discount curve referenced
    /// in `config.model_configs`. The curves must already be bootstrapped
    /// in the context. Model configs with a `Calibrated` volatility source
    /// are calibrated here, against the volatility surfaces/cubes
    /// constructed in the context.
    ///
    /// # Errors
    /// Returns an error if a required discount curve, volatility surface or
    /// cube is missing, if the curve has no nodes, or if calibration fails.
    pub fn new(context: &PricingContext, config: XvaEngineConfig) -> Result<Self> {
        let store = context.constructed_elements();

        let mut curves = HashMap::new();
        let mut model_params = HashMap::new();

        // Resolve base models first; derived configs (with a `driver`)
        // inherit the driver's resolved parameters in a second pass.
        for mc in config.model_configs.iter().filter(|mc| mc.driver.is_none()) {
            let (snapshot, params) = Self::snapshot_model_curve(context, mc)?;
            curves.insert(mc.market_index.clone(), snapshot);
            model_params.insert(mc.market_index.clone(), params);
        }
        for mc in config.model_configs.iter().filter(|mc| mc.driver.is_some()) {
            let (snapshot, params) = Self::snapshot_derived_curve(context, mc, &model_params)?;
            curves.insert(mc.market_index.clone(), snapshot);
            model_params.insert(mc.market_index.clone(), params);
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

        // Snapshot bootstrapped credit (survival) curves for per-counterparty CVA.
        let mut credit_curves = HashMap::new();
        for (index, element) in store.credit_curves() {
            let borrowed = element.curve();
            let nodes = borrowed.nodes().ok_or_else(|| {
                QSError::NotFoundErr(format!("Credit curve {index} has no nodes"))
            })?;
            // Node 0 is the reference date with S = 1; pillars follow.
            let pillar_dates: Vec<Date> = nodes.iter().skip(1).map(|(d, _)| *d).collect();
            let survivals: Vec<f64> = nodes.iter().skip(1).map(|(_, v)| v.value()).collect();
            let mut labels = borrowed.pillar_labels().unwrap_or_default();
            if labels.len() != pillar_dates.len() {
                labels = (0..pillar_dates.len())
                    .map(|i| format!("{index}.pillar_{i}"))
                    .collect();
            }
            let dc = borrowed.day_counter().unwrap_or(DayCounter::Actual365);
            credit_curves.insert(
                index.clone(),
                CreditCurveSnapshot {
                    pillar_dates,
                    survivals,
                    labels,
                    day_counter: dc,
                },
            );
        }

        // Snapshot every bootstrapped discount curve for funding-curve FVA.
        let funding_curves = Self::snapshot_discount_curves(store);

        Ok(Self {
            setup: InternalModelSetup {
                curves,
                model_params,
                fx_configs: config.fx_configs,
                fx_spots,
                domestic_currency: domestic,
                domestic_index: context.base_index().clone(),
                reference_date: context.evaluation_date(),
                day_counter: DayCounter::Actual365,
                n_paths: config.n_paths,
                seed: config.seed,
                requests: Vec::new(), // filled in run()
                fixing_store: context.fixing_store().clone(),
            },
            frequency: config.frequency,
            credit_curves,
            funding_curves,
        })
    }

    /// Runs the full XVA pipeline.
    ///
    /// Builds a [`PreprocessorExecutor`] with preprocessing steps (fixing resolution),
    /// runs it on all netting sets, then launches the Savine
    /// parallel AAD evaluation loop.
    ///
    /// Each [`NettingSet`] carries its own [`CsaTerms`]
    /// (collateral discounting plus credit/funding parameters), from which the
    /// per-client CVA and FVA aggregators are built.
    ///
    /// # Errors
    /// Returns an error if any netting set lacks CSA terms, or if simulation
    /// or evaluation fails.
    pub fn run(
        &mut self,
        netting_sets: &mut HashMap<String, NettingSet>,
    ) -> Result<ExposureResult> {
        // 1. Build preprocessor pipeline.
        let fixing_pp = FixingPreprocessor::new(
            self.setup.reference_date,
            self.setup.day_counter,
            self.setup.fixing_store.clone(),
        );

        let mut inspector = PreprocessorExecutor::new().with_preprocessor(Box::new(fixing_pp));

        // 2. Visit all netting sets in-place, assigning global indices.
        inspector.visit(netting_sets.values_mut());
        self.setup.requests = inspector.requests().to_vec();

        // 2b. Validate that every resolved discount index has a simulated
        // curve. The LGM model silently skips unknown curves, which would
        // otherwise produce missing discounts downstream.
        for req in &self.setup.requests {
            if let Some(discount_request) = &req.discount_request {
                let index = discount_request.market_index();
                if !self.setup.curves.contains_key(&index) {
                    return Err(QSError::NotFoundErr(format!(
                        "Discount curve {index} resolved by a CSA discount policy is not \
                         configured in the XVA engine; add an LgmModelConfig (and bootstrap \
                         the curve) for it"
                    )));
                }
            }
        }

        // 3. Build simulation dates.
        let max_maturity = netting_sets
            .values()
            .flat_map(|ns| ns.claims().iter())
            .map(ContingentClaim::payment_date)
            .max()
            .unwrap_or_else(|| self.setup.reference_date.advance(1, TimeUnit::Years));

        let schedule = MakeSchedule::new(self.setup.reference_date, max_maturity)
            .with_frequency(self.frequency)
            .build()?;
        let sim_dates = schedule.dates().clone();

        // 3b. System-curve discount factors DF(0, t_d) at the simulation
        // dates, taken from the engine's base (domestic) curve. Exposures at
        // future dates are values as of t_d; multiplying by these
        // deterministic DFs expresses every XVA in present-value terms on the
        // system curve. (Deterministic approximation: no rate sensitivity is
        // propagated through this discounting term.)
        let system_curve = self
            .setup
            .curves
            .get(&self.setup.domestic_index)
            .ok_or_else(|| {
                QSError::NotFoundErr(format!(
                    "System (base) discount curve {} not configured in the XVA engine",
                    self.setup.domestic_index
                ))
            })?;
        let system_ts = DiscountTermStructure::<f64>::new(
            system_curve.dates.clone(),
            system_curve.discount_factors.clone(),
            system_curve.day_counter,
            system_curve.interpolator,
            true,
        )?;
        let system_dfs: Vec<f64> = sim_dates
            .iter()
            .map(|d| system_ts.discount_factor(*d))
            .collect::<Result<Vec<f64>>>()?;

        // 4. Per-netting-set aggregator factories from each client's CSA terms.
        let mut factories: HashMap<String, Vec<Box<dyn PfeAggregatorFactory>>> = HashMap::new();
        for (id, ns) in netting_sets.iter() {
            let csa = ns.csa_terms().ok_or_else(|| {
                QSError::InvalidValueErr(format!(
                    "Netting set '{id}' has no CSA terms; build it with NettingSet::with_csa_terms"
                ))
            })?;

            // CVA: bootstrapped credit curve when assigned, flat spread otherwise.
            let cva_factory: Box<dyn PfeAggregatorFactory> =
                if let Some(credit_index) = &csa.credit_index {
                    let snapshot = self.credit_curves.get(credit_index).ok_or_else(|| {
                        QSError::NotFoundErr(format!(
                            "Netting set '{id}' references credit curve {credit_index}, but it \
                             was not bootstrapped in the pricing context"
                        ))
                    })?;
                    Box::new(CreditCurveCvaFactory {
                        pillar_dates: snapshot.pillar_dates.clone(),
                        pillar_survivals: snapshot.survivals.clone(),
                        pillar_labels: snapshot.labels.clone(),
                        recovery: csa.recovery,
                        n_paths: self.setup.n_paths,
                        day_counter: snapshot.day_counter,
                        system_dfs: Some(system_dfs.clone()),
                    })
                } else {
                    Box::new(CvaFactory {
                        credit_spread: csa.credit_spread,
                        recovery: csa.recovery,
                        n_paths: self.setup.n_paths,
                        system_dfs: Some(system_dfs.clone()),
                    })
                };

            // FVA: funding curve when assigned, explicit spread term
            // structure otherwise, flat spread as fallback.
            let fva_factory = self.build_fva_factory(id, csa, &system_ts, &system_dfs)?;

            factories.insert(id.clone(), vec![cva_factory, fva_factory]);
        }

        // 5. Build netting-set slice map.
        let ns_slices: HashMap<String, &[_]> = netting_sets
            .iter()
            .map(|(id, ns)| (id.clone(), ns.claims()))
            .collect();

        // 6. Run.
        evaluate_with_xva(&sim_dates, &ns_slices, &factories, &self.setup)
    }

    /// Snapshots the f64 data of a model's bootstrapped discount curve and
    /// resolves its LGM parameters (calibrating the sigma schedule when a
    /// volatility source is configured).
    ///
    /// # Errors
    /// Returns an error if the curve is missing or empty, or if sigma
    /// resolution fails.
    fn snapshot_model_curve(
        context: &PricingContext,
        mc: &LgmModelConfig,
    ) -> Result<(CurveSnapshot, LgmResolvedParams)> {
        let snapshot = Self::snapshot_curve_data(context, &mc.market_index)?;

        // Resolve the short-rate sigma schedule: calibrate against the
        // constructed vol surface/cube when a volatility source is
        // configured, otherwise use the flat sigma. Calibrated models
        // also retain the vol-quote pillars and the IFT sensitivities
        // `d(sigma_i)/d(vol_i)` so the AAD pass can report XVA
        // sensitivities to the market vol quotes.
        let (sigma_schedule, vol_pillars) = Self::resolve_sigma_schedule(
            context,
            mc,
            &snapshot.dates,
            &snapshot.discount_factors,
            snapshot.day_counter,
        )?;
        let lambda = mc.lambda.ok_or_else(|| {
            QSError::InvalidValueErr(format!(
                "LgmModelConfig for {} must set `lambda` (or use `driver`)",
                mc.market_index
            ))
        })?;

        Ok((
            snapshot,
            LgmResolvedParams {
                lambda,
                sigma_schedule,
                vol_pillars,
                driver: None,
            },
        ))
    }

    /// Resolves a derived-curve config: the curve has no dynamics of its
    /// own and is reconstructed from its driver's simulated factor and
    /// parameters (deterministic basis), with its own initial term
    /// structure.
    ///
    /// # Errors
    /// Returns an error if the config also sets `lambda`/`sigma`/`volatility`,
    /// or if the driver is missing or itself derived.
    fn snapshot_derived_curve(
        context: &PricingContext,
        mc: &LgmModelConfig,
        resolved: &HashMap<MarketIndex, LgmResolvedParams>,
    ) -> Result<(CurveSnapshot, LgmResolvedParams)> {
        if mc.lambda.is_some() || mc.sigma.is_some() || mc.volatility.is_some() {
            return Err(QSError::InvalidValueErr(format!(
                "LgmModelConfig for {}: `driver` is mutually exclusive with `lambda`, `sigma` \
                 and `volatility` — the curve inherits its driver's dynamics",
                mc.market_index
            )));
        }
        let driver = mc.driver.clone().ok_or_else(|| {
            QSError::UnexpectedErr("snapshot_derived_curve called without driver".into())
        })?;
        let driver_params = resolved.get(&driver).ok_or_else(|| {
            QSError::NotFoundErr(format!(
                "Driver model {driver} for {} must be configured as a non-derived \
                 LgmModelConfig",
                mc.market_index
            ))
        })?;
        let snapshot = Self::snapshot_curve_data(context, &mc.market_index)?;
        Ok((
            snapshot,
            LgmResolvedParams {
                lambda: driver_params.lambda,
                sigma_schedule: driver_params.sigma_schedule.clone(),
                vol_pillars: None,
                driver: Some(driver),
            },
        ))
    }

    /// Snapshots the f64 data of a bootstrapped discount curve.
    ///
    /// # Errors
    /// Returns an error if the curve is missing or empty.
    fn snapshot_curve_data(context: &PricingContext, index: &MarketIndex) -> Result<CurveSnapshot> {
        let store = context.constructed_elements();
        let elem = store.discount_curve(index).ok_or_else(|| {
            QSError::NotFoundErr(format!("Discount curve not found for index {index:?}"))
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
        let dc = borrowed.day_counter().unwrap_or(DayCounter::Actual365);
        let ift_sensitivities = borrowed.ift_sensitivities().map(<[Vec<f64>]>::to_vec);

        Ok(CurveSnapshot {
            dates,
            discount_factors: dfs,
            day_counter: dc,
            interpolator: Interpolator::LogLinear,
            pillar_labels,
            pillar_values,
            ift_sensitivities,
        })
    }

    /// Resolves an LGM model config into a piecewise-constant sigma schedule
    /// plus, for calibrated models, the vol-quote pillars carrying the
    /// calibration IFT sensitivities `d(sigma_i)/d(vol_i)`.
    ///
    /// # Errors
    /// Returns an error if neither `sigma` nor `volatility` is set, if the
    /// volatility source is unsupported, or if calibration fails.
    fn resolve_sigma_schedule(
        context: &PricingContext,
        mc: &LgmModelConfig,
        dates: &[Date],
        dfs: &[f64],
        dc: DayCounter,
    ) -> Result<SigmaResolution> {
        let Some(volatility) = &mc.volatility else {
            return mc.sigma.map_or_else(
                || {
                    Err(QSError::InvalidValueErr(format!(
                        "LgmModelConfig for {} must set either `sigma` or `volatility`",
                        mc.market_index
                    )))
                },
                |sigma| Ok((vec![(0.0, sigma)], None)),
            );
        };
        let curve_f64 = DiscountTermStructure::<f64>::new(
            dates.to_vec(),
            dfs.to_vec(),
            dc,
            Interpolator::LogLinear,
            true,
        )?;
        match volatility {
            VolatilitySourceConfiguration::Constant { value } => Ok((vec![(0.0, *value)], None)),
            VolatilitySourceConfiguration::Calibrated(calibration) => {
                let lambda = mc.lambda.ok_or_else(|| {
                    QSError::InvalidValueErr(format!(
                        "LgmModelConfig for {} must set `lambda` to calibrate",
                        mc.market_index
                    ))
                })?;
                let mut hw = HullWhite::new(lambda, &curve_f64);
                hw.calibrate_with_configuration(
                    calibration,
                    context.constructed_elements(),
                    context.quote_store(),
                    &curve_f64,
                    Level::Mid,
                )?;
                let vol_func = hw.vol_func().ok_or_else(|| {
                    QSError::UnexpectedErr("Calibration produced no vol function".into())
                })?;
                let schedule: Vec<(f64, f64)> = vol_func.iter().copied().collect();
                let vol_pillars = match (vol_func.ift_sensitivities(), hw.calibration_quality()) {
                    (Some(ift), Some(quality)) => Some(
                        quality
                            .records
                            .iter()
                            .enumerate()
                            .map(|(i, r)| VolPillar {
                                label: r.identifier.clone(),
                                market_vol: r.market_vol,
                                dsigma_dvol: ift[i][i],
                            })
                            .collect(),
                    ),
                    _ => None,
                };
                Ok((schedule, vol_pillars))
            }
            VolatilitySourceConfiguration::Surface { .. }
            | VolatilitySourceConfiguration::Cube { .. } => Err(QSError::InvalidValueErr(
                "Lgm supports Constant or Calibrated volatility sources; sampling a \
                 surface/cube directly would misuse Black vols as short-rate vols"
                    .into(),
            )),
        }
    }

    /// Snapshots the f64 data of every bootstrapped discount curve in the
    /// store (used for funding-curve FVA lookups).
    fn snapshot_discount_curves(
        store: &crate::core::marketdatahandling::constructedelementstore::ConstructedElementStore,
    ) -> HashMap<MarketIndex, CurveSnapshot> {
        let mut snapshots = HashMap::new();
        for (index, element) in store.discount_curves() {
            let borrowed = element.curve();
            let Some(nodes) = borrowed.nodes() else {
                continue;
            };
            snapshots.insert(
                index.clone(),
                CurveSnapshot {
                    dates: nodes.iter().map(|(d, _)| *d).collect(),
                    discount_factors: nodes.iter().map(|(_, v)| v.value()).collect(),
                    day_counter: borrowed.day_counter().unwrap_or(DayCounter::Actual365),
                    interpolator: Interpolator::LogLinear,
                    pillar_labels: borrowed.pillar_labels().unwrap_or_default(),
                    pillar_values: borrowed
                        .pillars()
                        .unwrap_or_default()
                        .iter()
                        .map(|(_, v)| v.value())
                        .collect(),
                    ift_sensitivities: borrowed.ift_sensitivities().map(<[Vec<f64>]>::to_vec),
                },
            );
        }
        snapshots
    }

    /// Builds the FVA aggregator factory for one netting set from its CSA
    /// terms: funding curve when assigned (with any explicit spread curve
    /// applied as an additive overlay on top), explicit spread term
    /// structure otherwise, flat spread as fallback.
    fn build_fva_factory(
        &self,
        id: &str,
        csa: &CsaTerms,
        system_ts: &DiscountTermStructure<f64>,
        system_dfs: &[f64],
    ) -> Result<Box<dyn PfeAggregatorFactory>> {
        // Optional overlay: explicit funding spread curve applied on top of
        // the curve-implied basis (spread over the funding index).
        let (overlay_dates, overlay_spreads, overlay_labels) =
            if let Some(spread_curve) = &csa.funding_spread_curve {
                spread_curve.validate()?;
                let labels = spread_curve
                    .dates
                    .iter()
                    .map(|d| format!("funding_spread.{d}"))
                    .collect();
                (
                    spread_curve.dates.clone(),
                    spread_curve.spreads.clone(),
                    labels,
                )
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };

        if let Some(funding_index) = &csa.funding_index {
            let snapshot = self.funding_curves.get(funding_index).ok_or_else(|| {
                QSError::NotFoundErr(format!(
                    "Netting set '{id}' references funding curve {funding_index}, but it was \
                     not bootstrapped in the pricing context"
                ))
            })?;
            let funding_ts = DiscountTermStructure::<f64>::new(
                snapshot.dates.clone(),
                snapshot.discount_factors.clone(),
                snapshot.day_counter,
                snapshot.interpolator,
                true,
            )?;
            // Forward funding spreads over the system curve, one per
            // funding-curve node bucket (assigned to the bucket's right
            // endpoint).
            let dc = snapshot.day_counter;
            let ref_date = self.setup.reference_date;
            let mut pillar_dates = Vec::new();
            let mut pillar_spreads = Vec::new();
            let mut prev = ref_date;
            for date in snapshot.dates.iter().filter(|d| **d > ref_date) {
                let dt = dc.year_fraction(prev, *date);
                if dt <= 0.0 {
                    continue;
                }
                let fwd_df_funding =
                    funding_ts.discount_factor(*date)? / funding_ts.discount_factor(prev)?;
                let fwd_df_system =
                    system_ts.discount_factor(*date)? / system_ts.discount_factor(prev)?;
                pillar_spreads.push((fwd_df_system / fwd_df_funding).ln() / dt);
                pillar_dates.push(*date);
                prev = *date;
            }
            let mut labels = snapshot.pillar_labels.clone();
            if labels.len() != pillar_dates.len() {
                labels = pillar_dates
                    .iter()
                    .map(|d| format!("{funding_index}.{d}"))
                    .collect();
            }
            Ok(Box::new(FundingCurveFvaFactory {
                pillar_dates,
                pillar_spreads,
                pillar_labels: labels,
                overlay_dates,
                overlay_spreads,
                overlay_labels,
                n_paths: self.setup.n_paths,
                day_counter: dc,
                system_dfs: Some(system_dfs.to_vec()),
            }))
        } else if let Some(spread_curve) = &csa.funding_spread_curve {
            spread_curve.validate()?;
            let labels = spread_curve
                .dates
                .iter()
                .map(|d| format!("funding_spread.{d}"))
                .collect();
            Ok(Box::new(FundingCurveFvaFactory {
                pillar_dates: spread_curve.dates.clone(),
                pillar_spreads: spread_curve.spreads.clone(),
                pillar_labels: labels,
                overlay_dates: Vec::new(),
                overlay_spreads: Vec::new(),
                overlay_labels: Vec::new(),
                n_paths: self.setup.n_paths,
                day_counter: DayCounter::Actual365,
                system_dfs: Some(system_dfs.to_vec()),
            }))
        } else {
            Ok(Box::new(FvaFactory {
                funding_spread: csa.funding_spread,
                n_paths: self.setup.n_paths,
                system_dfs: Some(system_dfs.to_vec()),
            }))
        }
    }
}

/// Snapshot of f64 curve data extracted from the `PricingContext`.
/// Each rayon thread uses this to build a thread-local `DualFwd` curve.
#[derive(Clone)]
struct CurveSnapshot {
    dates: Vec<Date>,
    discount_factors: Vec<f64>,
    day_counter: DayCounter,
    interpolator: Interpolator,
    pillar_labels: Vec<String>,
    pillar_values: Vec<f64>,
    /// Bootstrap IFT matrix `d(DF_i)/d(quote_j)`. When present, the rebuilt
    /// per-thread curve connects its discount factors to the quote pillar
    /// leaves so the AAD pass yields dXVA/dquote sensitivities.
    ift_sensitivities: Option<Vec<Vec<f64>>>,
}


/// Snapshot of a bootstrapped credit (survival) curve. The reference-date
/// node (`S = 1`) is excluded.
#[derive(Clone)]
struct CreditCurveSnapshot {
    pillar_dates: Vec<Date>,
    survivals: Vec<f64>,
    labels: Vec<String>,
    day_counter: DayCounter,
}

/// Resolved sigma schedule plus optional calibrated vol-quote pillars.
type SigmaResolution = (Vec<(f64, f64)>, Option<Vec<VolPillar>>);

/// Resolved LGM parameters: mean reversion plus a (possibly calibrated)
/// piecewise-constant sigma schedule.
#[derive(Clone)]
struct LgmResolvedParams {
    lambda: f64,
    sigma_schedule: Vec<(f64, f64)>,
    /// Vol-quote pillars aligned with `sigma_schedule` (calibrated models
    /// only): quote label, market vol, and IFT sensitivity `d(sigma)/d(vol)`.
    vol_pillars: Option<Vec<VolPillar>>,
    /// Set for derived curves: the rate model whose simulated factor and
    /// parameters drive this curve (deterministic-basis reconstruction).
    driver: Option<MarketIndex>,
}

impl LgmResolvedParams {
    /// Builds the per-thread `DualFwd` sigma schedule. Calibrated sigmas are
    /// rebuilt as tape expressions connected to vol-quote leaves via the
    /// calibration IFT sensitivities:
    ///   `sigma_i = sigma_i0 + (dsigma/dvol)_i * (v_i - v_i0)`
    /// so the backward pass yields dXVA/dvol. The vol leaves are appended to
    /// `all_leaves` under their quote labels.
    fn dualfwd_schedule(&self, all_leaves: &mut Vec<(String, DualFwd)>) -> Vec<(f64, DualFwd)> {
        self.vol_pillars.as_ref().map_or_else(
            || {
                self.sigma_schedule
                    .iter()
                    .map(|&(t, s)| (t, DualFwd::scalar(s)))
                    .collect()
            },
            |pillars| {
                self.sigma_schedule
                    .iter()
                    .zip(pillars)
                    .map(|(&(t, s), vp)| {
                        let leaf = DualFwd::new(vp.market_vol);
                        all_leaves.push((vp.label.clone(), leaf));
                        let delta: DualFwd = (leaf - DualFwd::scalar(vp.market_vol)).into();
                        let sigma: DualFwd =
                            (DualFwd::scalar(s) + DualFwd::scalar(vp.dsigma_dvol) * delta).into();
                        (t, sigma)
                    })
                    .collect()
            },
        )
    }
}

/// One calibrated sigma pillar traced back to its market vol quote.
#[derive(Clone)]
struct VolPillar {
    label: String,
    market_vol: f64,
    dsigma_dvol: f64,
}

impl CurveSnapshot {
    /// Build a `DiscountTermStructure<DualFwd>` on the current thread's tape.
    ///
    /// # Errors
    /// Returns an error if curve construction or pillar assignment fails.
    fn build_dualfwd_curve(&self) -> Result<DiscountTermStructure<DualFwd>> {
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
        )?
        .with_pillar_values(pvs)?
        .with_pillar_labels(self.pillar_labels.clone())?;

        if let Some(ift) = &self.ift_sensitivities {
            curve = curve.with_ift_sensitivities(ift.clone());
        }

        curve.put_pillars_on_tape();
        Ok(curve)
    }
}

/// Internal model setup implementing `XvaModelSetup`.
struct InternalModelSetup {
    curves: HashMap<MarketIndex, CurveSnapshot>,
    model_params: HashMap<MarketIndex, LgmResolvedParams>,
    fx_configs: Vec<FxModelConfig>,
    fx_spots: HashMap<Currency, f64>,
    domestic_currency: Currency,
    domestic_index: MarketIndex,
    reference_date: Date,
    day_counter: DayCounter,
    n_paths: usize,
    seed: u64,
    requests: Vec<SimulationRequest>,
    fixing_store: FixingStore,
}

// Safety: all fields are owned plain data (Vec, HashMap, f64, etc.). No Rc/RefCell.
unsafe impl Send for InternalModelSetup {}
unsafe impl Sync for InternalModelSetup {}

impl InternalModelSetup {
    /// Builds the LGM rate models on top of the rebuilt `DualFwd` curves and
    /// registers them (plus any derived-curve driver links) on `model`.
    /// Returns separate rate-model instances for FX models to borrow.
    ///
    /// Sigma schedules are built once per base model; derived curves share
    /// their driver's schedule (same tape leaves) so both curves respond to
    /// the same vol quotes.
    fn add_rate_models<'c>(
        &self,
        built_curves: &'c [(MarketIndex, DiscountTermStructure<DualFwd>)],
        all_leaves: &mut Vec<(String, DualFwd)>,
        model: &mut LgmMarketModel<'c, DualFwd>,
    ) -> Result<Vec<(MarketIndex, LgmRateModel<'c, DualFwd>)>> {
        let mut schedules: HashMap<MarketIndex, Vec<(f64, DualFwd)>> = HashMap::new();
        for (idx, _) in built_curves {
            let params = self.model_params.get(idx).ok_or_else(|| {
                QSError::NotFoundErr(format!("Model config missing for curve {idx:?}"))
            })?;
            if params.driver.is_none() {
                schedules.insert(idx.clone(), params.dualfwd_schedule(all_leaves));
            }
        }

        let mut fx_rate_models: Vec<(MarketIndex, LgmRateModel<'c, DualFwd>)> = Vec::new();
        for (idx, curve) in built_curves {
            let params = self.model_params.get(idx).ok_or_else(|| {
                QSError::NotFoundErr(format!("Model config missing for curve {idx:?}"))
            })?;
            let schedule_key = params.driver.as_ref().unwrap_or(idx);
            let schedule = schedules
                .get(schedule_key)
                .ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Sigma schedule missing for model {schedule_key} (curve {idx})"
                    ))
                })?
                .clone();
            let rate_model = LgmRateModel::new_piecewise(
                DualFwd::scalar(params.lambda),
                schedule.clone(),
                curve,
            )?;
            model.add_curve_model(idx.clone(), rate_model);
            if let Some(d) = &params.driver {
                model.set_curve_driver(idx.clone(), d.clone());
            }

            // If any FX config references this curve's currency, build an extra
            // rate model for the FX model to borrow.
            if !self.fx_configs.is_empty() {
                let fx_rate =
                    LgmRateModel::new_piecewise(DualFwd::scalar(params.lambda), schedule, curve)?;
                fx_rate_models.push((idx.clone(), fx_rate));
            }
        }
        Ok(fx_rate_models)
    }
}

impl XvaModelSetup for InternalModelSetup {
    fn n_paths(&self) -> usize {
        self.n_paths
    }

    fn with_model<R>(&self, dates: &[Date], callback: &mut ModelCallback<'_, R>) -> Result<R> {
        // 1. Build DualFwd curves and collect leaves.
        let mut built_curves: Vec<(MarketIndex, DiscountTermStructure<DualFwd>)> = Vec::new();
        let mut all_leaves: Vec<(String, DualFwd)> = Vec::new();

        for (idx, snap) in &self.curves {
            let curve = snap.build_dualfwd_curve()?;
            let leaves: Vec<(String, DualFwd)> = curve
                .pillars()
                .unwrap_or_default()
                .into_iter()
                .map(|(label, &val)| (label, val))
                .collect();
            all_leaves.extend(leaves);
            built_curves.push((idx.clone(), curve));
        }

        // 2. Build rate models for curve_models (moved into the market model)
        //    plus separate instances for FX models to borrow.
        let mut model = LgmMarketModel::new(
            self.domestic_currency,
            self.domestic_index.clone(),
            self.reference_date,
            self.day_counter,
        )
        .with_n_paths(self.n_paths)
        .with_seed(self.seed);

        let fx_rate_models = self.add_rate_models(&built_curves, &mut all_leaves, &mut model)?;

        // 3. Build FX models from the separate rate model instances.
        //    Find domestic and foreign rate models by index.
        let find_fx_rate = |idx: &MarketIndex| -> Option<usize> {
            fx_rate_models.iter().position(|(i, _)| i == idx)
        };

        for fx_cfg in &self.fx_configs {
            let dom_pos = find_fx_rate(&self.domestic_index).ok_or_else(|| {
                QSError::NotFoundErr("Domestic rate model not found for FX".into())
            })?;
            // Find the foreign index by currency
            let foreign_index = self
                .model_params
                .keys()
                .find(|idx| {
                    idx.rate_index_details()
                        .is_ok_and(|d| d.currency() == fx_cfg.foreign_currency)
                })
                .ok_or_else(|| {
                    QSError::NotFoundErr("Foreign rate model not found for FX".into())
                })?;
            let for_pos = find_fx_rate(foreign_index).ok_or_else(|| {
                QSError::NotFoundErr("Foreign rate model not found for FX".into())
            })?;

            // SAFETY: dom_pos != for_pos (domestic != foreign currency).
            // We need two simultaneous immutable borrows from the Vec.
            let (dom_rate, for_rate) = if dom_pos < for_pos {
                let (left, right) = fx_rate_models.split_at(for_pos);
                (&left[dom_pos].1, &right[0].1)
            } else {
                let (left, right) = fx_rate_models.split_at(dom_pos);
                (&right[0].1, &left[for_pos].1)
            };

            let spot = *self.fx_spots.get(&fx_cfg.foreign_currency).ok_or_else(|| {
                QSError::NotFoundErr(format!(
                    "FX spot missing for currency {}",
                    fx_cfg.foreign_currency
                ))
            })?;

            // FX spot and vol are tracked tape leaves so the backward pass
            // yields dXVA/dspot and dXVA/dvol.
            let pair = format!("{}{}", fx_cfg.foreign_currency, self.domestic_currency);
            let spot_leaf = DualFwd::new(spot);
            let fx_vol_leaf = DualFwd::new(fx_cfg.fx_vol);
            all_leaves.push((format!("FX.{pair}.spot"), spot_leaf));
            all_leaves.push((format!("FX.{pair}.vol"), fx_vol_leaf));

            let fx_model = LgmFxModel::new(
                dom_rate,
                for_rate,
                fx_vol_leaf,
                spot_leaf,
                DualFwd::scalar(fx_cfg.rho),
            );
            model.add_fx_model(fx_cfg.foreign_currency, fx_model);
        }

        model.set_evaluation_dates(dates.to_vec());
        model.set_requests(self.requests.clone());

        callback(&model, &all_leaves)
    }
}

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    ad::{scalar::Scalar, Recorded},
    core::{
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            constructedelementstore::ConstructedElementStore,
            discountrequest::DiscountRequest,
            fxrequest::FxRequest,
            marketdata::{MarketDataProvider, MarketDataRequest},
        },
        pillars::Pillars,
        pricingcontext::PricingContext,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::Interpolator,
    models::{
        hullwhite::hullwhitemodel::HullWhite,
        lgm::{
            lgmcomponents::{LgmFxModel, LgmRateModel},
            lgmmarketmodel::LgmMarketModel,
        },
        modelconfiguration::{GaussianRateParameterSource, ParameterSource},
    },
    quotes::{fixingstore::FixingStore, quote::Level, quoteselector::QuoteSelector},
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
    volatility::modelcalibration::CalibrationSource,
    xva::{
        aggregator::{
            CreditCurveCvaFactory, CvaFactory, FundingCurveFvaFactory, FvaFactory,
            XvaAggregatorFactory,
        },
        contigentclaim::ContingentClaim,
        csa::CsaTerms,
        execution::{binding_key, XvaExecution},
        nettingset::NettingSet,
        visitors::{
            exposureevaluator::{
                evaluate_with_margin, ExposureResult, ModelCallback, VariationMarginSimulation,
                XvaModelSetup,
            },
            fixingpreprocessor::FixingPreprocessor,
            marketmodel::MarketModel,
            preprocessorexecutor::{PreprocessorExecutor, SimulationRequest},
        },
    },
};

/// LGM parameters for one rate curve in the XVA simulation.
///
/// `lambda` is measured in inverse years and controls the decay of
/// long-maturity factor loadings. `sigma` is an annualized absolute rate
/// volatility. A value of `0.01` means 100 bp per square-root year.
///
/// Parameters are either `Fixed { sigma }` or `Calibrated` against a
/// volatility surface (caplets) or cube (swaptions) constructed in the
/// pricing context.
///
/// FX-implied collateral curves such as `Collateral(CLP, USD)` set
/// [`Self::driver`] to reuse another curve's stochastic factor.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LgmModelConfig {
    /// Curve or rate index represented by this model configuration.
    pub market_index: MarketIndex,
    /// Mean-reversion speed in inverse years. Larger positive values reduce
    /// distant-maturity exposure to the current factor. Zero selects the
    /// continuous non-mean-reverting limit. Required unless [`Self::driver`]
    /// is set.
    #[serde(default)]
    pub lambda: Option<f64>,
    /// Source of the Gaussian parameters. Required unless [`Self::driver`] is
    /// set.
    #[serde(default)]
    pub parameter_source: Option<GaussianRateParameterSource>,
    /// Rate model that drives this curve's dynamics.
    ///
    /// An FX-implied collateral curve can reuse a risk-free curve's Gaussian
    /// factor. For example, `Collateral(CLP, USD)` can use `ICP` as its driver.
    /// The driver supplies `lambda`, sigma, and the simulated state.
    ///
    /// The derived curve supplies its own initial term structure. Its discount
    /// factors combine that term structure with the driver's state, preserving
    /// the time-zero cross-currency basis. A driver configuration leaves
    /// `lambda` and `parameter_source` empty.
    ///
    /// ```json
    /// { "market_index": { "Collateral": ["CLP", "USD"] }, "driver": "ICP" }
    /// ```
    #[serde(default)]
    pub driver: Option<MarketIndex>,
}

/// FX model parameters for a foreign currency against the engine base currency.
///
/// The simulated spot is quoted as base-currency units per one unit of
/// `foreign_currency` and follows a flat-volatility lognormal process coupled
/// to the domestic and foreign LGM rate factors.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FxModelConfig {
    /// Foreign currency (domestic is always the engine's base currency).
    pub foreign_currency: Currency,
    /// Annualized lognormal FX volatility as a decimal. A value of `0.12`
    /// means 12% per square-root year.
    pub fx_vol: f64,
    /// Correlation between domestic-rate and FX-spot Brownian shocks, in
    /// `[-1, 1]`. This is used both to correlate path shocks and in the
    /// domestic-measure quanto drift.
    #[serde(default)]
    pub rho: f64,
}

/// Configuration for the XVA engine.
///
/// Contains only the simulation/model setup. Credit, funding and
/// collateral (CSA) parameters are per client and belong to each
/// [`NettingSet`]'s [`CsaTerms`].
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XvaEngineConfig {
    /// LGM model parameters, one per rate curve.
    pub model_configs: Vec<LgmModelConfig>,
    /// FX model parameters, one per foreign currency.
    #[serde(default)]
    pub fx_configs: Vec<FxModelConfig>,
    /// Number of Monte Carlo paths. Use an even value so every Sobol point has
    /// an antithetic partner. At least 2,048 paths are recommended for
    /// optional exposure profiles.
    pub n_paths: usize,
    /// Deterministic Owen-scrambling seed for the Sobol sequence.
    pub seed: u64,
    /// Simulation-grid frequency (for example monthly or quarterly). Finer
    /// grids reduce time-discretization error at increased runtime and memory.
    pub frequency: Frequency,
}

/// High-level XVA engine.
///
/// Takes a fully initialised [`PricingContext`] (with bootstrapped curves)
/// and an [`XvaEngineConfig`] to produce exposure cubes, XVA values and first
/// sensitivities. Supported simulations prepare and reuse a native path program
/// inside this engine. Other layouts capture each path and execute native gradients.
///
/// [`Self::run`] accepts a mutable map of netting-set identifiers to
/// [`NettingSet`] values, each constructed with CSA terms; it does not accept
/// raw trades. Curves are rebuilt from node snapshots with `LogLinear`
/// interpolation. Deterministic system-curve discount factors applied to XVA
/// aggregators are frozen `f64` inputs and contribute no AD discounting risk.
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
    configuration: XvaEngineConfig,
    execution: Option<XvaExecution>,
    execution_key: Option<String>,
    code_reused: bool,
}

struct XvaRunInputs {
    dates: Vec<Date>,
    factories: HashMap<String, Vec<Box<dyn XvaAggregatorFactory>>>,
    margins: HashMap<String, VariationMarginSimulation>,
}

impl XvaEngine {
    /// Creates a new engine from an initialised [`PricingContext`].
    ///
    /// Snapshots the f64 curve data from every discount curve referenced in
    /// `config.model_configs`. The initialized context supplies the
    /// bootstrapped curves and constructed volatility surfaces or cubes.
    /// `Calibrated` model configurations are calibrated during construction.
    ///
    /// # Errors
    /// Returns an error if a required discount curve, volatility surface or
    /// cube is missing, if the curve has no nodes, or if calibration fails.
    pub fn new(context: &PricingContext, config: XvaEngineConfig) -> Result<Self> {
        Self::from_market(
            context,
            context.constructed_elements(),
            context.quote_store(),
            config,
        )
    }

    /// Creates an XVA engine using an independent market-data provider.
    ///
    /// Model curves, credit and funding curves are requested without generating
    /// preconfigured Monte Carlo simulations. Volatility markets are requested
    /// only when a model calibrates to them.
    ///
    /// # Errors
    /// Returns an error when market construction or model calibration fails.
    pub fn with_market_provider(
        context: &PricingContext,
        provider: &(impl MarketDataProvider + QuoteSelector),
        config: XvaEngineConfig,
    ) -> Result<Self> {
        let mut requests: Vec<_> = context
            .curve_configurations()
            .iter()
            .map(|configuration| ConstructedElementRequest::DiscountCurve {
                market_index: configuration.market_index().clone(),
            })
            .chain(
                context
                    .credit_curve_configurations()
                    .iter()
                    .map(|configuration| ConstructedElementRequest::CreditCurve {
                        market_index: configuration.market_index().clone(),
                    }),
            )
            .collect();
        for model in &config.model_configs {
            if let Some(ParameterSource::Calibrated(calibration)) = &model.parameter_source {
                requests.push(match calibration.source() {
                    CalibrationSource::Surface { market_index } => {
                        ConstructedElementRequest::VolatilitySurface {
                            market_index: market_index.clone(),
                        }
                    }
                    CalibrationSource::Cube { market_index } => {
                        ConstructedElementRequest::VolatilityCube {
                            market_index: market_index.clone(),
                        }
                    }
                });
            }
        }
        let market = provider.handle_request(
            &MarketDataRequest::default().with_constructed_elements_request(requests),
        )?;
        Self::from_market(context, market.constructed_elements(), provider, config)
    }

    /// Builds the XVA market setup from resolved curves and configured model parameters.
    fn from_market(
        context: &PricingContext,
        store: &ConstructedElementStore,
        selector: &impl QuoteSelector,
        config: XvaEngineConfig,
    ) -> Result<Self> {
        let configuration = config.clone();
        let mut curves = HashMap::new();
        let mut model_params = HashMap::new();

        // Resolve base models first; derived configs (with a `driver`)
        // inherit the driver's resolved parameters in a second pass.
        for mc in config.model_configs.iter().filter(|mc| mc.driver.is_none()) {
            let (snapshot, params) =
                Self::snapshot_model_curve(store, selector, context.quote_level(), mc)?;
            curves.insert(mc.market_index.clone(), snapshot);
            model_params.insert(mc.market_index.clone(), params);
        }
        for mc in config.model_configs.iter().filter(|mc| mc.driver.is_some()) {
            let (snapshot, params) = Self::snapshot_derived_curve(store, mc, &model_params)?;
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
            configuration,
            execution: None,
            execution_key: None,
            code_reused: false,
            frequency: config.frequency,
            credit_curves,
            funding_curves,
        })
    }

    /// Runs the full XVA pipeline.
    ///
    /// Builds a [`PreprocessorExecutor`] with preprocessing steps (fixing resolution),
    /// runs it on changed netting-set bindings, then evaluates the owned native
    /// path program for supported layouts. Other layouts capture each path and
    /// execute compiled gradients in parallel.
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
        let key = binding_key(netting_sets);
        if key.is_some() && key == self.execution_key {
            if let Some(execution) = &self.execution {
                return execution.evaluate();
            }
        }
        // Native path preparation also avoids repeated per-path capture in small runs.
        if key.is_some() {
            self.prepare(netting_sets)?;
            if let Some(execution) = &self.execution {
                return execution.evaluate();
            }
        }
        let inputs = self.prepare_run(netting_sets)?;
        let trades = netting_sets
            .iter()
            .map(|(id, set)| (id.clone(), set.claims()))
            .collect::<HashMap<_, _>>();
        evaluate_with_margin(
            &inputs.dates,
            &trades,
            &inputs.factories,
            &inputs.margins,
            &self.setup,
        )
    }

    /// Prepares the existing engine's path calculation for repeated execution.
    ///
    /// `run` returns the same cubes, adjustment values and first-risk labels.
    /// Mutated claims or CSA terms invalidate the binding. Supported simulations
    /// prepare on first use; an explicit call moves that cost before `run`.
    ///
    /// # Errors
    /// Rejects unsupported scripted/path-dependent claims and propagates market,
    /// preprocessing and native compilation errors.
    pub fn prepare(&mut self, netting_sets: &mut HashMap<String, NettingSet>) -> Result<()> {
        if binding_key(netting_sets).is_none() {
            return Err(QSError::NotImplementedErr(
                "prepared XVA claim strategy".into(),
            ));
        }
        let inputs = self.prepare_run(netting_sets)?;
        let trades = netting_sets
            .iter()
            .map(|(id, set)| (id.clone(), set.claims()))
            .collect::<HashMap<_, _>>();
        let execution = XvaExecution::prepare(
            &inputs.dates,
            &trades,
            &inputs.factories,
            &inputs.margins,
            &self.setup,
            self.execution.as_ref(),
        )?;
        self.code_reused = self
            .execution
            .as_ref()
            .is_some_and(|old| execution.shares_code_with(old));
        self.execution = Some(execution);
        self.execution_key = binding_key(netting_sets);
        Ok(())
    }

    /// Refreshes market snapshots and calibration, retaining compatible native code.
    ///
    /// The next preparation or `run` refreshes coefficients and their quote
    /// Jacobian. No numerical binding from the previous market is evaluated.
    ///
    /// # Errors
    /// Propagates construction and calibration errors from the current context.
    pub fn update_market(&mut self, context: &PricingContext) -> Result<()> {
        let mut refreshed = Self::new(context, self.configuration.clone())?;
        refreshed.execution = self.execution.take();
        *self = refreshed;
        Ok(())
    }

    /// Whether the latest preparation reused an earlier machine-code allocation.
    #[must_use]
    pub const fn preparation_reused_code(&self) -> bool {
        self.code_reused
    }

    /// Resolves claims, simulation dates, collateral and adjustment factories for one binding.
    #[allow(clippy::too_many_lines)]
    fn prepare_run(
        &mut self,
        netting_sets: &mut HashMap<String, NettingSet>,
    ) -> Result<XvaRunInputs> {
        for ns in netting_sets.values() {
            if let Some(csa) = ns.csa_terms() {
                csa.validate()?;
            }
        }
        // 1. Build preprocessor pipeline.
        let fixing_pp = FixingPreprocessor::new(
            self.setup.reference_date,
            self.setup.day_counter,
            self.setup.fixing_store.clone(),
        );

        let mut inspector = PreprocessorExecutor::new().with_preprocessor(Box::new(fixing_pp));

        // 2. Visit all netting sets in-place, assigning global indices.
        let mut ordered_sets = netting_sets.iter_mut().collect::<Vec<_>>();
        ordered_sets.sort_by_key(|(id, _)| *id);
        inspector.visit(ordered_sets.into_iter().map(|(_, set)| set));
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
            .map_or_else(|| self.setup.reference_date.advance(1, TimeUnit::Years), Ok)?;

        let schedule = MakeSchedule::new(self.setup.reference_date, max_maturity)
            .with_frequency(self.frequency)
            .build()?;
        let sim_dates = schedule.dates().clone();
        let margins = self.build_variation_margin_requests(netting_sets, &sim_dates)?;

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
        let mut factories: HashMap<String, Vec<Box<dyn XvaAggregatorFactory>>> = HashMap::new();
        for (id, ns) in netting_sets.iter() {
            let csa = ns.csa_terms().ok_or_else(|| {
                QSError::InvalidValueErr(format!(
                    "Netting set '{id}' has no CSA terms; build it with NettingSet::with_csa_terms"
                ))
            })?;

            // CVA: bootstrapped credit curve when assigned, flat spread otherwise.
            let cva_factory: Box<dyn XvaAggregatorFactory> =
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
            let fva_factory =
                self.build_fva_factory(id, csa, &system_ts, &system_dfs, &sim_dates)?;

            factories.insert(id.clone(), vec![cva_factory, fva_factory]);
        }

        Ok(XvaRunInputs {
            dates: sim_dates,
            factories,
            margins,
        })
    }

    /// Builds collateral remuneration requests for netting sets with variation margin enabled.
    fn build_variation_margin_requests(
        &mut self,
        netting_sets: &HashMap<String, NettingSet>,
        dates: &[Date],
    ) -> Result<HashMap<String, VariationMarginSimulation>> {
        let mut margins = HashMap::new();
        for (id, ns) in netting_sets {
            let Some(csa) = ns.csa_terms().filter(|terms| terms.variation_margin) else {
                continue;
            };
            if !self.setup.curves.contains_key(&csa.collateral_index) {
                return Err(QSError::NotFoundErr(format!(
                    "Netting set '{id}' requires simulated CSA remuneration curve {}",
                    csa.collateral_index
                )));
            }
            let fx_request_index = if csa.collateral_currency == self.setup.domestic_currency {
                None
            } else {
                if !self.setup.fx_spots.contains_key(&csa.collateral_currency) {
                    return Err(QSError::NotFoundErr(format!(
                        "Netting set '{id}' requires an FX model for collateral currency {}",
                        csa.collateral_currency
                    )));
                }
                let slot = self.setup.requests.len();
                self.setup.requests.push(SimulationRequest {
                    fx_request: Some(FxRequest::single(csa.collateral_currency)),
                    ..SimulationRequest::default()
                });
                Some(slot)
            };
            let accrual_request_indices = dates
                .iter()
                .skip(1)
                .map(|&date| {
                    let slot = self.setup.requests.len();
                    self.setup.requests.push(SimulationRequest {
                        expiration_date: Some(date),
                        discount_request: Some(DiscountRequest::new(
                            csa.collateral_index.clone(),
                            date,
                        )),
                        ..SimulationRequest::default()
                    });
                    slot
                })
                .collect();
            margins.insert(
                id.clone(),
                VariationMarginSimulation::new(
                    csa.clone(),
                    accrual_request_indices,
                    fx_request_index,
                ),
            );
        }
        Ok(margins)
    }

    /// Snapshots the f64 data of a model's bootstrapped discount curve and
    /// resolves its LGM parameters (calibrating the sigma schedule when
    /// calibrated parameters are configured).
    ///
    /// # Errors
    /// Returns an error if the curve is missing or empty, or if sigma
    /// resolution fails.
    fn snapshot_model_curve(
        store: &ConstructedElementStore,
        selector: &impl QuoteSelector,
        level: Level,
        mc: &LgmModelConfig,
    ) -> Result<(CurveSnapshot, LgmResolvedParams)> {
        let snapshot = Self::snapshot_curve_data(store, &mc.market_index)?;

        // Resolve the short-rate sigma schedule: calibrate against the
        // constructed vol surface/cube when calibrated parameters are
        // configured, otherwise use the fixed sigma. Calibrated models
        // also retain the vol-quote pillars and the IFT sensitivities
        // `d(sigma_i)/d(vol_i)` so the AAD pass can report XVA
        // sensitivities to the market vol quotes.
        let (sigma_schedule, vol_pillars) = Self::resolve_sigma_schedule(
            store,
            selector,
            level,
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
    /// Returns an error if the config also sets `lambda` or
    /// `parameter_source`. An error is also returned when the driver is
    /// missing or is itself derived.
    fn snapshot_derived_curve(
        store: &ConstructedElementStore,
        mc: &LgmModelConfig,
        resolved: &HashMap<MarketIndex, LgmResolvedParams>,
    ) -> Result<(CurveSnapshot, LgmResolvedParams)> {
        if mc.lambda.is_some() || mc.parameter_source.is_some() {
            return Err(QSError::InvalidValueErr(format!(
                "LgmModelConfig for {}: `driver` is mutually exclusive with `lambda` and \
                 `parameter_source` because the curve inherits its driver's dynamics",
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
        let snapshot = Self::snapshot_curve_data(store, &mc.market_index)?;
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
    fn snapshot_curve_data(
        store: &ConstructedElementStore,
        index: &MarketIndex,
    ) -> Result<CurveSnapshot> {
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
    /// Returns an error if `parameter_source` is missing or calibration
    /// fails.
    fn resolve_sigma_schedule(
        store: &ConstructedElementStore,
        selector: &impl QuoteSelector,
        level: Level,
        mc: &LgmModelConfig,
        dates: &[Date],
        dfs: &[f64],
        dc: DayCounter,
    ) -> Result<SigmaResolution> {
        let Some(parameter_source) = &mc.parameter_source else {
            return Err(QSError::InvalidValueErr(format!(
                "LgmModelConfig for {} must set `parameter_source`",
                mc.market_index
            )));
        };
        let curve_f64 = DiscountTermStructure::<f64>::new(
            dates.to_vec(),
            dfs.to_vec(),
            dc,
            Interpolator::LogLinear,
            true,
        )?;
        match parameter_source {
            ParameterSource::Fixed(fixed) => {
                fixed.validate()?;
                Ok((vec![(0.0, fixed.sigma)], None))
            }
            ParameterSource::Calibrated(calibration) => {
                let lambda = mc.lambda.ok_or_else(|| {
                    QSError::InvalidValueErr(format!(
                        "LgmModelConfig for {} must set `lambda` to calibrate",
                        mc.market_index
                    ))
                })?;
                let mut hw = HullWhite::new(lambda, &curve_f64);
                hw.calibrate_with_configuration(calibration, store, selector, &curve_f64, level)?;
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
        }
    }

    /// Snapshots the f64 data of every bootstrapped discount curve in the
    /// store (used for funding-curve FVA lookups).
    fn snapshot_discount_curves(
        store: &ConstructedElementStore,
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
        sim_dates: &[Date],
    ) -> Result<Box<dyn XvaAggregatorFactory>> {
        if let Some(curve) = &csa.funding_spread_curve {
            for date in sim_dates {
                let spread = curve.zero_rate(*date)?;
                if !spread.is_finite() {
                    return Err(QSError::InvalidValueErr(format!(
                        "Non-finite funding spread at {date}"
                    )));
                }
            }
        }

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
                spread_curve: csa.funding_spread_curve.clone(),
                n_paths: self.setup.n_paths,
                day_counter: dc,
                system_dfs: Some(system_dfs.to_vec()),
            }))
        } else if csa.funding_spread_curve.is_some() {
            Ok(Box::new(FundingCurveFvaFactory {
                pillar_dates: Vec::new(),
                pillar_spreads: Vec::new(),
                pillar_labels: Vec::new(),
                spread_curve: csa.funding_spread_curve.clone(),
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
/// Each rayon thread uses this to build a thread-local `Recorded` curve.
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
    /// Builds the per-thread `Recorded` sigma schedule. Calibrated sigmas are
    /// rebuilt as capture expressions connected to vol-quote leaves via the
    /// calibration IFT sensitivities:
    ///   `sigma_i = sigma_i0 + (dsigma/dvol)_i * (v_i - v_i0)`
    /// so the differentiate pass yields dXVA/dvol. The vol leaves are appended to
    /// `all_leaves` under their quote labels.
    fn recorded_schedule(&self, all_leaves: &mut Vec<(String, Recorded)>) -> Vec<(f64, Recorded)> {
        self.vol_pillars.as_ref().map_or_else(
            || {
                self.sigma_schedule
                    .iter()
                    .map(|&(t, s)| (t, Recorded::scalar(s)))
                    .collect()
            },
            |pillars| {
                self.sigma_schedule
                    .iter()
                    .zip(pillars)
                    .map(|(&(t, s), vp)| {
                        let leaf = Recorded::new(vp.market_vol);
                        all_leaves.push((vp.label.clone(), leaf));
                        let delta: Recorded = leaf - Recorded::scalar(vp.market_vol);
                        let sigma: Recorded =
                            Recorded::scalar(s) + Recorded::scalar(vp.dsigma_dvol) * delta;
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
    /// Build a `DiscountTermStructure<Recorded>` on the current thread's capture.
    ///
    /// # Errors
    /// Returns an error if curve construction or pillar assignment fails.
    fn build_recorded_curve(&self) -> Result<DiscountTermStructure<Recorded>> {
        let dfs: Vec<Recorded> = self
            .discount_factors
            .iter()
            .map(|&v| Recorded::scalar(v))
            .collect();
        let pvs: Vec<Recorded> = self
            .pillar_values
            .iter()
            .map(|&v| Recorded::scalar(v))
            .collect();

        let mut curve = DiscountTermStructure::<Recorded>::new(
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

        curve.register_pillars();
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
    /// Builds the LGM rate models on top of the rebuilt `Recorded` curves and
    /// registers them (plus any derived-curve driver links) on `model`.
    /// Returns separate rate-model instances for FX models to borrow.
    ///
    /// Sigma schedules are built once per base model; derived curves share
    /// their driver's schedule (same capture leaves) so both curves respond to
    /// the same vol quotes.
    fn add_rate_models<'c>(
        &self,
        built_curves: &'c [(MarketIndex, DiscountTermStructure<Recorded>)],
        all_leaves: &mut Vec<(String, Recorded)>,
        model: &mut LgmMarketModel<'c, Recorded>,
    ) -> Result<Vec<(MarketIndex, LgmRateModel<'c, Recorded>)>> {
        let mut schedules: HashMap<MarketIndex, Vec<(f64, Recorded)>> = HashMap::new();
        for (idx, _) in built_curves {
            let params = self.model_params.get(idx).ok_or_else(|| {
                QSError::NotFoundErr(format!("Model config missing for curve {idx:?}"))
            })?;
            if params.driver.is_none() {
                schedules.insert(idx.clone(), params.recorded_schedule(all_leaves));
            }
        }

        let mut fx_rate_models: Vec<(MarketIndex, LgmRateModel<'c, Recorded>)> = Vec::new();
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
                Recorded::scalar(params.lambda),
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
                    LgmRateModel::new_piecewise(Recorded::scalar(params.lambda), schedule, curve)?;
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
        // 1. Build Recorded curves and collect leaves.
        let mut built_curves: Vec<(MarketIndex, DiscountTermStructure<Recorded>)> = Vec::new();
        let mut all_leaves: Vec<(String, Recorded)> = Vec::new();

        let mut ordered_curves: Vec<_> = self.curves.iter().collect();
        ordered_curves.sort_by_cached_key(|(index, _)| index.to_string());
        for (idx, snap) in ordered_curves {
            let curve = snap.build_recorded_curve()?;
            let leaves: Vec<(String, Recorded)> = curve
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

            // FX spot and vol are tracked capture leaves so the differentiate pass
            // yields dXVA/dspot and dXVA/dvol.
            let pair = format!("{}{}", fx_cfg.foreign_currency, self.domestic_currency);
            let spot_leaf = Recorded::new(spot);
            let fx_vol_leaf = Recorded::new(fx_cfg.fx_vol);
            all_leaves.push((format!("FX.{pair}.spot"), spot_leaf));
            all_leaves.push((format!("FX.{pair}.vol"), fx_vol_leaf));

            let fx_model = LgmFxModel::new(
                dom_rate,
                for_rate,
                fx_vol_leaf,
                spot_leaf,
                Recorded::scalar(fx_cfg.rho),
            );
            model.add_fx_model(fx_cfg.foreign_currency, fx_model);
        }

        model.set_evaluation_dates(dates.to_vec());
        model.set_requests(self.requests.clone());

        callback(&model, &all_leaves)
    }
}

#[cfg(test)]
mod preparation_tests {
    use super::*;
    use crate::prelude::*;
    use crate::xva::claimevaluationstrategy::ClaimEvaluationStrategy;
    use std::collections::BTreeMap;
    use std::{cell::RefCell, rc::Rc};

    /// Builds the XVA engine, claims and market used by preparation regressions.
    fn fixture(
        margin: bool,
        discount: f64,
    ) -> Result<(XvaEngine, HashMap<String, NettingSet>, PricingContext)> {
        let reference = Date::new(2025, 1, 1);
        let maturity = Date::new(2027, 1, 1);
        let curve = DiscountTermStructure::<Recorded>::new(
            vec![reference, Date::new(2026, 1, 1), maturity],
            vec![
                Recorded::one(),
                Recorded::scalar(0.97),
                Recorded::scalar(discount),
            ],
            DayCounter::Actual365,
            Interpolator::LogLinear,
            true,
        )?
        .with_pillar_values(vec![Recorded::scalar(0.97), Recorded::scalar(discount)])?
        .with_pillar_labels(vec!["SOFR.1Y".into(), "SOFR.2Y".into()])?
        .with_ift_sensitivities(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
        let mut elements = ConstructedElementStore::default();
        elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
        );
        let context = PricingContext::new()
            .with_quote_store(QuoteStore::new(reference))
            .with_constructed_elements(elements)
            .with_base_currency(Currency::USD)
            .with_base_index(MarketIndex::SOFR);
        let engine = XvaEngine::new(
            &context,
            XvaEngineConfig {
                model_configs: vec![LgmModelConfig {
                    market_index: MarketIndex::SOFR,
                    lambda: Some(0.05),
                    driver: None,
                    parameter_source: Some(ParameterSource::Fixed(
                        GaussianRateModelParameters::new(0.015),
                    )),
                }],
                fx_configs: vec![],
                n_paths: 64,
                seed: 42,
                frequency: Frequency::Quarterly,
            },
        )?;
        let terms = CsaTerms {
            collateral_index: MarketIndex::SOFR,
            collateral_currency: Currency::USD,
            variation_margin: margin,
            counterparty_threshold: 15.0,
            own_threshold: 20.0,
            minimum_transfer_amount: 5.0,
            credit_spread: 0.01,
            recovery: 0.4,
            funding_spread: 0.005,
            funding_spread_curve: None,
            funding_index: None,
            credit_index: None,
        };
        let fixed = ContingentClaim::new(
            "fixed".into(),
            0,
            maturity,
            None,
            None,
            None,
            Currency::USD,
            None,
            1.0,
            Side::LongReceive,
            ClaimEvaluationStrategy::Deterministic { amount: 400.0 },
            Some(MarketIndex::SOFR),
        );
        let floating = ContingentClaim::new(
            "floating".into(),
            1,
            maturity,
            Some(reference),
            Some(reference),
            Some(maturity),
            Currency::USD,
            None,
            5_000.0,
            Side::PayShort,
            ClaimEvaluationStrategy::LinearRate {
                spread: 0.001,
                day_counter: DayCounter::Actual365,
            },
            Some(MarketIndex::SOFR),
        );
        Ok((
            engine,
            HashMap::from([(
                "client".into(),
                NettingSet::with_csa_terms(vec![fixed, floating], terms),
            )]),
            context,
        ))
    }
    /// Indexes adjustment results by netting set and measure.
    fn values(result: &ExposureResult) -> BTreeMap<String, f64> {
        result
            .xva_values
            .as_ref()
            .into_iter()
            .flatten()
            .map(|value| {
                (
                    format!("{}.{}", value.netting_set, value.measure),
                    value.value,
                )
            })
            .collect()
    }
    /// Indexes every published first-risk row for exact label comparison.
    fn risks(result: &ExposureResult) -> BTreeMap<String, f64> {
        result
            .sensitivities
            .as_ref()
            .into_iter()
            .flatten()
            .cloned()
            .collect()
    }
    /// Compares complete prepared and per-path numerical results in the XVA fixture.
    fn check(actual: &ExposureResult, expected: &ExposureResult) {
        assert_eq!(actual.cubes.len(), expected.cubes.len());
        for (actual, expected) in actual.cubes.iter().zip(&expected.cubes) {
            assert_eq!(actual.trade_id, expected.trade_id);
            assert_eq!(actual.dates, expected.dates);
            assert_eq!(actual.npvs.len(), expected.npvs.len());
            for (actual, expected) in actual
                .npvs
                .iter()
                .flatten()
                .zip(expected.npvs.iter().flatten())
            {
                assert!(
                    (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
                    "cube {actual} vs {expected}"
                );
            }
        }
        let actual_values = values(actual);
        let expected_values = values(expected);
        assert_eq!(actual_values.len(), expected_values.len());
        for (label, actual) in actual_values {
            let expected = expected_values[&label];
            assert!(
                (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
                "{label}: {actual} vs {expected}"
            );
        }
        let actual_risks = risks(actual);
        let expected_risks = risks(expected);
        assert_eq!(
            actual_risks.keys().collect::<Vec<_>>(),
            expected_risks.keys().collect::<Vec<_>>()
        );
        for (label, actual) in actual_risks {
            let expected = expected_risks[&label];
            assert!(
                (actual - expected).abs() <= 1e-8 * expected.abs().max(1.0),
                "risk {label}: {actual} vs {expected}"
            );
        }
    }
    /// Evaluates the per-path compiler route independently of the prepared program.
    fn reference(
        engine: &mut XvaEngine,
        sets: &mut HashMap<String, NettingSet>,
    ) -> Result<ExposureResult> {
        let inputs = engine.prepare_run(sets)?;
        let trades = sets
            .iter()
            .map(|(id, set)| (id.clone(), set.claims()))
            .collect::<HashMap<_, _>>();
        evaluate_with_margin(
            &inputs.dates,
            &trades,
            &inputs.factories,
            &inputs.margins,
            &engine.setup,
        )
    }

    /// Checks automatic preparation matches reference for small runs.
    #[test]
    fn automatic_preparation_matches_reference_for_small_runs() -> Result<()> {
        for margin in [false, true] {
            let (mut engine, mut sets, _) = fixture(margin, 0.94)?;
            let expected = reference(&mut engine, &mut sets)?;
            assert!(engine.setup.n_paths < 1_000);
            check(&engine.run(&mut sets)?, &expected);
            assert!(engine.execution.is_some());
            assert!(!engine.preparation_reused_code());
            check(&engine.run(&mut sets)?, &expected);
            check(&engine.run(&mut sets)?, &expected);
            engine.prepare(&mut sets)?;
            assert!(engine.preparation_reused_code());
            check(&engine.run(&mut sets)?, &expected);
        }
        Ok(())
    }

    /// Checks quote refresh reuses code and refreshes values and risk.
    #[test]
    fn quote_refresh_reuses_code_and_refreshes_values_and_risk() -> Result<()> {
        let (mut engine, mut sets, _) = fixture(true, 0.94)?;
        engine.prepare(&mut sets)?;
        let old = engine.run(&mut sets)?;
        let (mut fresh, mut fresh_sets, context) = fixture(true, 0.91)?;
        let expected = reference(&mut fresh, &mut fresh_sets)?;
        engine.update_market(&context)?;
        engine.prepare(&mut sets)?;
        assert!(engine.preparation_reused_code());
        let current = engine.run(&mut sets)?;
        check(&current, &expected);
        assert_ne!(values(&old), values(&current));
        assert_ne!(risks(&old), risks(&current));
        Ok(())
    }

    /// Checks claim mutation invalidates prepared values.
    #[test]
    fn claim_mutation_invalidates_prepared_values() -> Result<()> {
        let (mut engine, mut sets, _) = fixture(false, 0.94)?;
        engine.prepare(&mut sets)?;
        let old = engine.run(&mut sets)?;
        sets.get_mut("client").unwrap().claims_vec_mut().remove(0);
        let expected = reference(&mut engine, &mut sets)?;
        engine.prepare(&mut sets)?;
        let current = engine.run(&mut sets)?;
        check(&current, &expected);
        assert_ne!(values(&old), values(&current));
        Ok(())
    }
}

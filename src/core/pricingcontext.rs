use std::collections::{BTreeMap, HashMap};

use crate::{
    core::{
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            constructedelementstore::ConstructedElementStore,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricerconfiguration::{InstrumentType, PricerConfiguration},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    models::modelconfiguration::SimulationConfiguration,
    quotes::{
        fixingstore::FixingStore, fxstore::FxStore, quote::Level, quoteselector::QuoteSelector,
        quotestore::QuoteStore, scenario::Scenario,
    },
    rates::bootstrapping::{
        bootstrapdiscountpolicy::BootstrapDiscountPolicy,
        creditcurvebootstrapper::CreditCurveBootstrapper,
        creditcurveconfiguration::CreditCurveConfiguration, curveconfiguration::CurveConfiguration,
        multicurvebootstrapper::MultiCurveBootstrapper,
    },
    simulations::simulationbuilder::SimulationBuilder,
    time::date::Date,
    utils::errors::{QSError, Result},
    volatility::{
        volatilitycubebuilder::VolatilityCubeBuilder,
        volatilitycubeconfiguration::VolatilityCubeConfiguration,
        volatilitysurfacebuilder::VolatilitySurfaceBuilder,
        volatilitysurfaceconfiguration::VolatilitySurfaceConfiguration,
    },
};

/// Manages the context for instrument evaluation, including market data access, quote level preferences,
/// base currency settings, and a list of model parameter sets for multiple model types.
#[derive(Clone, Default)]
pub struct PricingContext {
    /// The quote store provides access to direct market data quotes and reference date information.
    quote_store: QuoteStore,
    /// Quote level used for market construction and model calibration.
    quote_level: Level,
    /// Scenario shocks applied to the quote store before bootstrapping.
    scenarios: Vec<Scenario>,
    /// The quote store with scenario shocks applied (built on `initialize`).
    shocked_quote_store: Option<QuoteStore>,
    /// The fixing store provides access to historical fixing values for indices and other reference data.
    fixing_store: FixingStore,
    /// Exchange rate store for FX spot rates used in cross-currency discounting.
    fx_store: FxStore,
    /// Curve specifications for curve construction.    
    curve_configurations: Vec<CurveConfiguration>,
    /// Credit (survival) curve specifications.
    credit_curve_configurations: Vec<CreditCurveConfiguration>,
    /// Volatility surface specifications.
    volatility_surface_configurations: Vec<VolatilitySurfaceConfiguration>,
    /// Volatility cube specifications.
    volatility_cube_configurations: Vec<VolatilityCubeConfiguration>,
    /// Model-driven Monte Carlo simulation specifications.
    simulation_configurations: Vec<SimulationConfiguration>,
    /// Instrument-level pricer selections and their typed model assignments.
    pricer_configurations: Vec<PricerConfiguration>,
    /// Constructed market data elements, such as discount curves, volatility surfaces, among others.
    constructed_elements: ConstructedElementStore,
    /// The base currency for pricing and reporting results.
    base_currency: Currency,
    /// Base remuneration index
    base_index: MarketIndex,
}

impl PricingContext {
    /// Copies market inputs and applies quote and zero-rate pillar shocks together.
    fn scenario_market_inputs(&self) -> Result<(Option<QuoteStore>, Vec<CurveConfiguration>)> {
        // Apply shocks to copies of both quoted and explicit curve inputs.
        let mut curve_configurations = self.curve_configurations.clone();
        let shocked_quotes = if self.scenarios.is_empty() {
            None
        } else {
            let mut store = self.quote_store.clone();
            for scenario in &self.scenarios {
                let mut matched = scenario.apply_matching_quotes(&mut store);
                for configuration in &mut curve_configurations {
                    matched += configuration.apply_scenario(scenario, store.reference_date());
                }
                if matched == 0 {
                    return Err(QSError::NotFoundErr(format!(
                        "Scenario target '{}' matched no quotes or zero-rate pillars",
                        scenario.target()
                    )));
                }
            }
            Some(store)
        };
        Ok((shocked_quotes, curve_configurations))
    }

    /// Sets the default collateral policy used to construct curves.
    #[must_use]
    pub fn with_csa_policy(
        mut self,
        policy: &crate::core::collateral::SingleCurveCSADiscountPolicy,
    ) -> Self {
        self.base_currency = policy.collateral_currency();
        self.base_index = policy.collateral_index().clone();
        self
    }

    /// Returns the default collateral policy for derivative valuation.
    #[must_use]
    pub fn csa_policy(&self) -> crate::core::collateral::SingleCurveCSADiscountPolicy {
        crate::core::collateral::SingleCurveCSADiscountPolicy::new(
            self.base_index.clone(),
            self.base_currency,
        )
    }

    /// Creates a new pricing data context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            quote_store: QuoteStore::default(),
            quote_level: Level::Mid,
            scenarios: Vec::new(),
            shocked_quote_store: None,
            fixing_store: FixingStore::default(),
            fx_store: FxStore::default(),
            curve_configurations: Vec::new(),
            credit_curve_configurations: Vec::new(),
            volatility_surface_configurations: Vec::new(),
            volatility_cube_configurations: Vec::new(),
            simulation_configurations: Vec::new(),
            pricer_configurations: Vec::new(),
            constructed_elements: ConstructedElementStore::default(),
            base_currency: Currency::USD, // Default base currency
            base_index: MarketIndex::SOFR,
        }
    }

    /// Returns the active market data store. When scenarios are set and the
    /// context is initialized, the scenario-shocked store is returned;
    /// otherwise the base store.
    #[must_use]
    pub fn quote_store(&self) -> &QuoteStore {
        self.shocked_quote_store
            .as_ref()
            .unwrap_or(&self.quote_store)
    }

    /// Returns the base (unshocked) quote store.
    #[must_use]
    pub const fn base_quote_store(&self) -> &QuoteStore {
        &self.quote_store
    }

    /// Returns the fixings store.
    #[must_use]
    pub const fn fixing_store(&self) -> &FixingStore {
        &self.fixing_store
    }

    /// Returns the exchange rate store.
    #[must_use]
    pub const fn fx_store(&self) -> &FxStore {
        &self.fx_store
    }

    /// Sets the quote store.
    #[must_use]
    pub fn with_quote_store(mut self, quote_store: QuoteStore) -> Self {
        self.quote_store = quote_store;
        self.shocked_quote_store = None;
        self
    }

    /// Selects the quote level for market construction and model calibration.
    /// Defaults to [`Level::Mid`]. Call [`Self::initialize`] after changing it
    /// to rebuild configured market objects. Missing levels produce an error.
    #[must_use]
    pub const fn with_quote_level(mut self, quote_level: Level) -> Self {
        self.quote_level = quote_level;
        self
    }

    /// Returns the quote level used for market construction and calibration.
    #[must_use]
    pub const fn quote_level(&self) -> Level {
        self.quote_level
    }

    /// Sets the scenario shocks applied to the quote store on `initialize`.
    #[must_use]
    pub fn with_scenarios(mut self, scenarios: Vec<Scenario>) -> Self {
        self.scenarios = scenarios;
        self.shocked_quote_store = None;
        self
    }

    /// Returns the scenarios applied to the quote store on `initialize`.
    #[must_use]
    pub const fn scenarios(&self) -> &Vec<Scenario> {
        &self.scenarios
    }

    /// Sets the base currency of the context.
    #[must_use]
    pub const fn with_base_currency(mut self, base_currency: Currency) -> Self {
        self.base_currency = base_currency;
        self
    }

    /// Sets the base collateral remuneration index.
    #[must_use]
    pub fn with_base_index(mut self, base_index: MarketIndex) -> Self {
        self.base_index = base_index;
        self
    }

    /// Sets the fixing store.
    #[must_use]
    pub fn with_fixing_store(mut self, fixing_store: FixingStore) -> Self {
        self.fixing_store = fixing_store;
        self
    }

    /// Sets the FX store.
    #[must_use]
    pub fn with_fx_store(mut self, fx_store: FxStore) -> Self {
        self.fx_store = fx_store;
        self
    }

    /// Sets the constructed elements store, replacing any previously registered elements.
    #[must_use]
    pub fn with_constructed_elements(
        mut self,
        constructed_elements: ConstructedElementStore,
    ) -> Self {
        self.constructed_elements = constructed_elements;
        self
    }

    /// Sets the curve configurations for bootstrapping.
    #[must_use]
    pub fn with_curve_configurations(mut self, configs: Vec<CurveConfiguration>) -> Self {
        self.curve_configurations = configs;
        self
    }

    /// Sets the credit curve configurations for bootstrapping.
    #[must_use]
    pub fn with_credit_curve_configurations(
        mut self,
        configs: Vec<CreditCurveConfiguration>,
    ) -> Self {
        self.credit_curve_configurations = configs;
        self
    }

    /// Sets the volatility surface configurations.
    #[must_use]
    pub fn with_volatility_surface_configurations(
        mut self,
        configs: Vec<VolatilitySurfaceConfiguration>,
    ) -> Self {
        self.volatility_surface_configurations = configs;
        self
    }

    /// Sets the volatility cube configurations.
    #[must_use]
    pub fn with_volatility_cube_configurations(
        mut self,
        configs: Vec<VolatilityCubeConfiguration>,
    ) -> Self {
        self.volatility_cube_configurations = configs;
        self
    }

    /// Sets the model-driven simulation configurations.
    #[must_use]
    pub fn with_simulation_configurations(mut self, configs: Vec<SimulationConfiguration>) -> Self {
        self.simulation_configurations = configs;
        self
    }

    /// Sets instrument-level pricer configurations loaded from typed data or
    /// JSON.
    #[must_use]
    pub fn with_pricer_configurations(mut self, configs: Vec<PricerConfiguration>) -> Self {
        self.pricer_configurations = configs;
        self
    }

    /// Returns the curve configurations used for bootstrapping.
    #[must_use]
    pub const fn curve_configurations(&self) -> &Vec<CurveConfiguration> {
        &self.curve_configurations
    }

    /// Returns the credit curve configurations used for bootstrapping.
    #[must_use]
    pub const fn credit_curve_configurations(&self) -> &Vec<CreditCurveConfiguration> {
        &self.credit_curve_configurations
    }

    /// Returns the volatility surface configurations.
    #[must_use]
    pub const fn volatility_surface_configurations(&self) -> &Vec<VolatilitySurfaceConfiguration> {
        &self.volatility_surface_configurations
    }

    /// Returns the volatility cube configurations.
    #[must_use]
    pub const fn volatility_cube_configurations(&self) -> &Vec<VolatilityCubeConfiguration> {
        &self.volatility_cube_configurations
    }

    /// Returns the model-driven simulation configurations.
    #[must_use]
    pub const fn simulation_configurations(&self) -> &Vec<SimulationConfiguration> {
        &self.simulation_configurations
    }

    /// Returns all instrument-level pricer configurations.
    #[must_use]
    pub const fn pricer_configurations(&self) -> &Vec<PricerConfiguration> {
        &self.pricer_configurations
    }

    /// Returns the configuration for one instrument family, if supplied.
    #[must_use]
    pub fn pricer_configuration(
        &self,
        instrument_type: InstrumentType,
    ) -> Option<&PricerConfiguration> {
        self.pricer_configurations
            .iter()
            .find(|configuration| configuration.instrument_type() == instrument_type)
    }

    /// Returns the current reference date.
    #[must_use]
    pub const fn evaluation_date(&self) -> Date {
        self.quote_store.reference_date()
    }

    /// Returns the constructed elements store.
    #[must_use]
    pub const fn constructed_elements(&self) -> &ConstructedElementStore {
        &self.constructed_elements
    }

    /// Returns a mutable reference to the constructed elements store.
    pub const fn constructed_elements_mut(&mut self) -> &mut ConstructedElementStore {
        &mut self.constructed_elements
    }

    /// Returns the base currency.
    #[must_use]
    pub const fn base_currency(&self) -> Currency {
        self.base_currency
    }

    /// Returns the base index.
    #[must_use]
    pub const fn base_index(&self) -> &MarketIndex {
        &self.base_index
    }

    /// Validates pricing configuration and constructs all configured market
    /// objects in dependency order.
    ///
    /// # Errors
    /// Returns an error for duplicate or incompatible pricer configuration,
    /// an unmatched scenario, or failed curve, volatility, or simulation
    /// construction.
    pub fn initialize(&mut self) -> Result<()> {
        let mut configured_instruments = std::collections::HashSet::new();
        for configuration in &self.pricer_configurations {
            configuration.validate().map_err(|error| {
                error.context(format!(
                    "validating pricer configuration for {:?}",
                    configuration.instrument_type()
                ))
            })?;
            if !configured_instruments.insert(configuration.instrument_type()) {
                return Err(QSError::InvalidValueErr(format!(
                    "Duplicate pricer configuration for instrument type {:?}",
                    configuration.instrument_type()
                )));
            }
        }

        let (shocked_quotes, curve_configurations) = self.scenario_market_inputs()?;
        self.shocked_quote_store = shocked_quotes;

        // Bootstrap discount curves.
        let policy = BootstrapDiscountPolicy::new(self.base_index.clone(), self.base_currency);
        let bootstrapper = MultiCurveBootstrapper::new(curve_configurations, policy)
            .with_fx_store(self.fx_store.clone())
            .with_fixing_store(self.fixing_store.clone());
        let curves = bootstrapper
            .bootstrap(self.quote_store(), self.quote_level)
            .map_err(|error| error.context("bootstrapping discount curves"))?;
        for (index, curve) in curves {
            self.constructed_elements
                .discount_curves_mut()
                .insert(index, curve);
        }

        // Bootstrap credit (survival) curves. Runs after the discount curves
        // since CDS pillar instruments are discounted with them.
        if !self.credit_curve_configurations.is_empty() {
            let credit_bootstrapper =
                CreditCurveBootstrapper::new(self.credit_curve_configurations.clone());
            let credit_curves = credit_bootstrapper
                .bootstrap(
                    self.quote_store(),
                    self.quote_level,
                    self.constructed_elements.discount_curves(),
                )
                .map_err(|error| error.context("bootstrapping credit curves"))?;
            for (index, curve) in credit_curves {
                self.constructed_elements
                    .credit_curves_mut()
                    .insert(index, curve);
            }
        }

        // Build volatility surfaces.
        if !self.volatility_surface_configurations.is_empty() {
            let surface_builder =
                VolatilitySurfaceBuilder::new(self.volatility_surface_configurations.clone());
            let surfaces = surface_builder
                .build(self.quote_store(), self.quote_level)
                .map_err(|error| error.context("initializing volatility surfaces"))?;
            for (index, surface) in surfaces {
                self.constructed_elements
                    .volatility_surfaces_mut()
                    .insert(index, surface);
            }
        }

        // Build volatility cubes.
        if !self.volatility_cube_configurations.is_empty() {
            let cube_builder =
                VolatilityCubeBuilder::new(self.volatility_cube_configurations.clone());
            let cubes = cube_builder
                .build(self.quote_store(), self.quote_level)
                .map_err(|error| error.context("initializing volatility cubes"))?;
            for (index, cube) in cubes {
                self.constructed_elements
                    .volatility_cubes_mut()
                    .insert(index, cube);
            }
        }

        // Build model-driven Monte Carlo simulations. Runs last so that
        // models can consume the constructed curves, surfaces, and cubes.
        if !self.simulation_configurations.is_empty() {
            let simulation_builder = SimulationBuilder::new(self.simulation_configurations.clone());
            let simulations = simulation_builder
                .build(
                    &self.constructed_elements,
                    self.quote_store(),
                    &self.fixing_store,
                    self.quote_level,
                )
                .map_err(|error| error.context("building Monte Carlo simulations"))?;
            for (index, simulation) in simulations {
                self.constructed_elements
                    .simulations_mut()
                    .insert(index, simulation);
            }
        }

        Ok(())
    }
}

impl MarketDataProvider for PricingContext {
    fn evaluation_date(&self) -> Date {
        self.quote_store.reference_date()
    }

    // this needs to be refactored
    #[allow(clippy::too_many_lines)]
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData> {
        // 1. Resolve constructed elements from the internal store.
        let mut constructed_elements = ConstructedElementStore::default();
        if let Some(element_requests) = request.constructed_elements_request() {
            for req in element_requests {
                match req {
                    ConstructedElementRequest::DiscountCurve { market_index } => {
                        let curve = self
                            .constructed_elements
                            .discount_curve(market_index)
                            .ok_or_else(|| {
                                QSError::NotFoundErr(format!(
                                    "Discount curve not found for index {market_index}"
                                ))
                            })?;
                        constructed_elements
                            .discount_curves_mut()
                            .insert(market_index.clone(), curve.clone());
                    }
                    ConstructedElementRequest::OptionalDividendCurve { market_index } => {
                        if let Some(curve) = self.constructed_elements.dividend_curve(market_index)
                        {
                            constructed_elements
                                .dividend_curves_mut()
                                .insert(market_index.clone(), curve.clone());
                        }
                    }
                    ConstructedElementRequest::DividendCurve { market_index } => {
                        let curve = self
                            .constructed_elements
                            .dividend_curve(market_index)
                            .ok_or_else(|| {
                                QSError::NotFoundErr(format!(
                                    "Dividend curve not found for index {market_index}"
                                ))
                            })?;
                        constructed_elements
                            .dividend_curves_mut()
                            .insert(market_index.clone(), curve.clone());
                    }
                    ConstructedElementRequest::CreditCurve { market_index } => {
                        let curve = self
                            .constructed_elements
                            .credit_curve(market_index)
                            .ok_or_else(|| {
                                QSError::NotFoundErr(format!(
                                    "Credit curve not found for index {market_index}"
                                ))
                            })?;
                        constructed_elements
                            .credit_curves_mut()
                            .insert(market_index.clone(), curve.clone());
                    }
                    ConstructedElementRequest::VolatilitySurface { market_index } => {
                        let surface = self
                            .constructed_elements
                            .volatility_surface(market_index)
                            .ok_or_else(|| {
                                QSError::NotFoundErr(format!(
                                    "Volatility surface not found for index {market_index}"
                                ))
                            })?;
                        constructed_elements
                            .volatility_surfaces_mut()
                            .insert(market_index.clone(), surface.clone());
                    }
                    ConstructedElementRequest::VolatilityCube { market_index } => {
                        let cube = self
                            .constructed_elements
                            .volatility_cube(market_index)
                            .ok_or_else(|| {
                                QSError::NotFoundErr(format!(
                                    "Volatility cube not found for index {market_index}"
                                ))
                            })?;
                        constructed_elements
                            .volatility_cubes_mut()
                            .insert(market_index.clone(), cube.clone());
                    }
                    // probably this will be moved out
                    ConstructedElementRequest::Simulation { market_index } => {
                        let sim = self
                            .constructed_elements
                            .simulations()
                            .get(market_index)
                            .ok_or_else(|| {
                                QSError::NotFoundErr(format!(
                                    "Simulation not found for index {market_index}"
                                ))
                            })?;
                        constructed_elements
                            .simulations_mut()
                            .insert(market_index.clone(), sim.clone());
                    }
                }
            }
        }

        // 2. Resolve fixings from the fixing store.
        let mut fixings: HashMap<MarketIndex, BTreeMap<Date, f64>> = HashMap::new();
        if let Some(fixing_requests) = request.fixings_request() {
            for fix_req in fixing_requests {
                let market_index = fix_req.market_index();
                let date = fix_req.date();
                let value = match self.fixing_store.fixing(market_index, date) {
                    Ok(value) => value,
                    Err(_) if fix_req.missing_fixing_allowed() => continue,
                    Err(error) => return Err(error),
                };
                fixings
                    .entry(market_index.clone())
                    .or_default()
                    .insert(date, value);
            }
        }

        // 3. Resolve FX rates from the FX store.
        // this approach is not ideal, it could lead to sensitivities in unnatural parities
        let mut fx_store = FxStore::new();
        if let Some(fx_requests) = request.fx_request() {
            for fx_req in fx_requests {
                if let Some(quote_ccy) = fx_req.quote() {
                    let rate = self.fx_store.get_fx_rate(fx_req.base(), quote_ccy)?;
                    fx_store.add_fx_rate(fx_req.base(), quote_ccy, rate);
                }
            }
        }

        // 4. Assemble final MarketData.
        let mut pricer_configurations = HashMap::new();
        if let Some(configuration_requests) = request.pricer_configuration_request() {
            for instrument_type in configuration_requests {
                let configuration = self
                    .pricer_configurations
                    .iter()
                    .find(|configuration| configuration.instrument_type() == *instrument_type)
                    .ok_or_else(|| {
                        QSError::NotFoundErr(format!(
                            "Pricer configuration not found for instrument type {instrument_type:?}"
                        ))
                    })?;
                pricer_configurations.insert(*instrument_type, configuration.clone());
            }
        }

        let calibration_quotes = request
            .calibration_request()
            .map(|calibration| {
                let mut quotes = QuoteStore::new(self.evaluation_date());
                for identifier in calibration.resolve_instrument_ids(&constructed_elements)? {
                    let quote = self.quote_store().select(&identifier).ok_or_else(|| {
                        QSError::NotFoundErr(format!("Calibration quote not found: {identifier}"))
                    })?;
                    quotes.add_quote(quote);
                }
                Ok::<_, QSError>(quotes)
            })
            .transpose()?;
        let mut md = MarketData::new(fixings, constructed_elements)
            .with_quote_level(self.quote_level)
            .with_fx_store(fx_store)
            .with_pricer_configurations(pricer_configurations);
        if let Some(quotes) = calibration_quotes {
            md = md.with_calibration_quotes(quotes);
        }

        Ok(md)
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use serde_json::json;

    type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    const DEPOSIT: &str = "FixedRateDeposit_USD_SOFR_1D";
    const SWAP: &str = "OIS_USD_SOFR_5Y";
    const CDS: &str = "Cds_TEST_USD_5Y";
    const CAPLET: &str = "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.04_Straddle_Black";
    const SWAPTION: &str = "Swaption_USD_SOFR_1Y_5Y_Absolute_0.04_Black";

    fn context(level: Level, only_selected_level: bool) -> TestResult<PricingContext> {
        let mut quotes = QuoteStore::new(Date::new(2026, 2, 24));
        let surface_quotes: Vec<_> = ["1Y", "2Y"]
            .into_iter()
            .flat_map(|expiry| {
                ["0.02", "0.04", "0.06"].into_iter().map(move |strike| {
                    format!("CapletFloorlet_USD_SOFR_3M_{expiry}_Absolute_{strike}_Straddle_Black")
                })
            })
            .collect();
        for (identifier, mid, bid, ask) in [
            (DEPOSIT, 0.04, 0.03, 0.05),
            (SWAP, 0.04, 0.03, 0.05),
            (CDS, 0.01, 0.005, 0.02),
            (SWAPTION, 0.25, 0.20, 0.35),
        ]
        .into_iter()
        .chain(
            surface_quotes
                .iter()
                .map(|id| (id.as_str(), 0.20, 0.15, 0.30)),
        ) {
            quotes.add_quote(Quote::new(
                identifier.parse()?,
                QuoteLevels::new(
                    (!only_selected_level || level == Level::Mid).then_some(mid),
                    (!only_selected_level || level == Level::Bid).then_some(bid),
                    (!only_selected_level || level == Level::Ask).then_some(ask),
                ),
            ));
        }
        Ok(PricingContext::new()
            .with_quote_level(level)
            .with_quote_store(quotes)
            .with_curve_configurations(vec![serde_json::from_value(json!({
                "market_index": "SOFR", "quotes": [DEPOSIT, SWAP]
            }))?])
            .with_credit_curve_configurations(vec![CreditCurveConfiguration::new(
                MarketIndex::Credit("TEST".into()),
                Currency::USD,
                MarketIndex::SOFR,
                0.4,
                vec![CDS.into()],
            )])
            .with_volatility_surface_configurations(vec![VolatilitySurfaceConfiguration::new(
                MarketIndex::SOFR,
                VolatilityType::Black,
                SmileType::Strike,
                surface_quotes,
            )])
            .with_volatility_cube_configurations(vec![VolatilityCubeConfiguration::new(
                MarketIndex::SOFR,
                VolatilityType::Black,
                SmileType::Strike,
                vec![SWAPTION.into()],
            )])
            .with_simulation_configurations(vec![serde_json::from_value(json!({
                "market_index": "SOFR",
                "model": {"HullWhite": {"alpha": 0.05, "parameter_source": {
                    "Calibrated": {"source": {"Surface": {"market_index": "SOFR"}},
                        "calibration_basket": {"strike": "Atm"}}
                }}},
                "n_paths": 4, "seed": 42, "horizon": "1Y", "frequency": "Quarterly"
            }))?]))
    }

    #[test]
    fn selected_level_reaches_all_builders_and_xva_calibration() -> TestResult {
        for level in [Level::Mid, Level::Bid, Level::Ask] {
            // With no other levels present, any hard-coded mid in a downstream
            // calibrator fails, even if its constructed surface uses the right side.
            let mut ctx = context(level, true)?;
            ctx.initialize()?;
            let elements = ctx.constructed_elements();
            assert!(elements
                .credit_curve(&MarketIndex::Credit("TEST".into()))
                .is_some());
            let simulation = elements
                .simulations()
                .get(&MarketIndex::SOFR)
                .ok_or("simulation")?;
            assert_eq!(simulation.simulation().borrow().n_paths(), 4);
            let surface = elements
                .volatility_surface(&MarketIndex::SOFR)
                .ok_or("surface")?
                .surface();
            let expected = ctx
                .quote_store()
                .quote(CAPLET)
                .ok_or("caplet")?
                .levels()
                .value(level)?;
            assert!((surface.pillars().ok_or("pillars")?[0].1.value() - expected).abs() < 1e-12);
            let cube = elements
                .volatility_cube(&MarketIndex::SOFR)
                .ok_or("cube")?
                .cube();
            let expected = ctx
                .quote_store()
                .quote(SWAPTION)
                .ok_or("swaption")?
                .levels()
                .value(level)?;
            assert!((cube.pillars().ok_or("pillars")?[0].1.value() - expected).abs() < 1e-12);

            let market = ctx.handle_request(
                &MarketDataRequest::default()
                    .with_calibration_request(ModelCalibrationConfiguration::new(
                        CalibrationSource::Surface {
                            market_index: MarketIndex::SOFR,
                        },
                    ))
                    .with_constructed_elements_request(vec![
                        ConstructedElementRequest::VolatilitySurface {
                            market_index: MarketIndex::SOFR,
                        },
                    ]),
            )?;
            assert_eq!(market.quote_level(), level);
            assert!(market.calibration_quotes().is_some());

            let config = serde_json::from_value(json!({
                "model_configs": [{"market_index": "SOFR", "lambda": 0.05,
                    "parameter_source": {"Calibrated": {
                        "source": {"Surface": {"market_index": "SOFR"}},
                        "calibration_basket": {"strike": "Atm"}
                    }}
                }],
                "n_paths": 4, "seed": 42, "frequency": "Quarterly"
            }))?;
            XvaEngine::new(&ctx, config)?;
        }
        Ok(())
    }

    #[test]
    fn selecting_and_changing_levels_rebuilds_the_market() -> TestResult {
        assert_eq!(PricingContext::new().quote_level(), Level::Mid);
        assert_eq!(PricingContext::default().quote_level(), Level::Mid);
        assert_eq!(MarketData::default().quote_level(), Level::Mid);
        let mut ctx = context(Level::Bid, false)?;
        let mut discount_factors = Vec::new();
        let mut survivals = Vec::new();
        for level in [Level::Bid, Level::Mid, Level::Ask] {
            ctx = ctx.with_quote_level(level);
            ctx.initialize()?;
            let elements = ctx.constructed_elements();
            discount_factors.push(
                elements
                    .discount_curve(&MarketIndex::SOFR)
                    .ok_or("curve")?
                    .curve()
                    .discount_factor(Date::new(2030, 2, 24))?
                    .value(),
            );
            survivals.push(
                elements
                    .credit_curve(&MarketIndex::Credit("TEST".into()))
                    .ok_or("credit")?
                    .curve()
                    .discount_factor(Date::new(2030, 2, 24))?
                    .value(),
            );
        }
        assert!(discount_factors.windows(2).all(|pair| pair[0] > pair[1]));
        assert!(survivals.windows(2).all(|pair| pair[0] > pair[1]));
        Ok(())
    }

    #[test]
    fn missing_selected_level_is_an_error() -> TestResult {
        for level in [Level::Bid, Level::Ask] {
            let mut ctx = context(Level::Mid, true)?.with_quote_level(level);
            let error = ctx
                .initialize()
                .err()
                .ok_or("expected missing level error")?;
            assert!(error.to_string().contains(&format!(
                "No {} quote available",
                format!("{level:?}").to_lowercase()
            )));
            assert!(matches!(error.root_cause(), QSError::NotFoundErr(_)));
            assert!(error.to_string().contains("bootstrapping discount curves"));
            assert!(error.to_string().contains("reading"));
            assert!(error.to_string().contains("SOFR"));
        }
        Ok(())
    }

    #[test]
    fn scenarios_preserve_the_selected_level_and_base_quotes() -> TestResult {
        let mut ctx = context(Level::Ask, true)?.with_scenarios(vec![Scenario::new(
            CAPLET,
            0.01,
            ScenarioType::Absolute,
        )]);
        for _ in 0..2 {
            ctx.initialize()?;
            let surface = ctx
                .constructed_elements()
                .volatility_surface(&MarketIndex::SOFR)
                .ok_or("surface")?
                .surface();
            let pillars = surface.pillars().ok_or("pillars")?;
            let (_, value) = pillars
                .iter()
                .find(|(label, _)| label == CAPLET)
                .ok_or("caplet pillar")?;
            assert!((value.value() - 0.31).abs() < 1e-12);
            assert_eq!(
                ctx.base_quote_store()
                    .quote(CAPLET)
                    .ok_or("caplet")?
                    .levels()
                    .ask(),
                Some(0.30)
            );
        }
        Ok(())
    }

    #[test]
    fn explicit_spread_scenarios_compose_without_changing_base_inputs() -> TestResult {
        let reference = Date::new(2026, 2, 24);
        let date = Date::new(2028, 2, 24);
        let funding = MarketIndex::Other("FUNDING".into());
        let curves = serde_json::from_value(json!([
            {"market_index": {"Other": "BENCHMARK"}, "interpolator": "Linear",
             "construction": {"type": "ZeroRate", "pillars": ["0D", "1Y", "2Y"],
                 "rates": [0.04, 0.04, 0.04], "compounding": "Continuous", "frequency": "Annual"}},
            {"market_index": {"Other": "SPREAD"}, "interpolator": "Linear",
             "construction": {"type": "ZeroRate", "pillars": ["0D", "1Y", "2Y"],
                 "rates": [0.005, 0.01, 0.015], "compounding": "Continuous", "frequency": "Annual"}},
            {"market_index": {"Other": "FUNDING"}, "construction": {"type": "Composite",
                 "base_index": {"Other": "BENCHMARK"}, "spread_index": {"Other": "SPREAD"}}}
        ]))?;
        let mut context = PricingContext::new()
            .with_quote_store(QuoteStore::new(reference))
            .with_curve_configurations(curves);
        let base_inputs = serde_json::to_value(context.curve_configurations())?;
        context.initialize()?;
        let factor = |context: &PricingContext| -> TestResult<f64> {
            Ok(context
                .constructed_elements()
                .discount_curve(&funding)
                .ok_or("funding")?
                .curve()
                .discount_factor(date)?
                .value())
        };
        let base = factor(&context)?;
        context = context.with_scenarios(vec![
            Scenario::new("SPREAD_2028-02-24", 0.01, ScenarioType::Absolute),
            Scenario::new("SPREAD", 0.10, ScenarioType::Relative),
        ]);
        let tau = DayCounter::Actual360.year_fraction(reference, date);
        let expected = base * (-((0.015 + 0.01) * 1.10 - 0.015) * tau).exp();
        for _ in 0..2 {
            context.initialize()?;
            assert!((factor(&context)? - expected).abs() < 1e-12);
            assert_eq!(
                serde_json::to_value(context.curve_configurations())?,
                base_inputs
            );
        }
        context = context.with_scenarios(vec![]);
        context.initialize()?;
        assert!((factor(&context)? - base).abs() < 1e-12);
        Ok(())
    }
}

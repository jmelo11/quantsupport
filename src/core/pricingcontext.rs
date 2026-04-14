use std::collections::{BTreeMap, HashMap};

use crate::{
    core::marketdatahandling::{
        constructedelementrequest::ConstructedElementRequest,
        constructedelementstore::ConstructedElementStore,
        marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    quotes::{fixingstore::FixingStore, fxstore::FxStore, quote::Level, quotestore::QuoteStore},
    rates::bootstrapping::{
        bootstrapdiscountpolicy::BootstrapDiscountPolicy, curveconfiguration::CurveConfiguration,
        multicurvebootstrapper::MultiCurveBootstrapper,
    },
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
#[derive(Default)]
pub struct PricingContext {
    /// The quote store provides access to direct market data quotes and reference date information.
    quote_store: QuoteStore,
    /// The fixing store provides access to historical fixing values for indices and other reference data.
    fixing_store: FixingStore,
    /// Exchange rate store for FX spot rates used in cross-currency discounting.
    fx_store: FxStore,
    /// Curve specifications for curve construction.    
    curve_configurations: Vec<CurveConfiguration>,
    /// Volatility surface specifications.
    volatility_surface_configurations: Vec<VolatilitySurfaceConfiguration>,
    /// Volatility cube specifications.
    volatility_cube_configurations: Vec<VolatilityCubeConfiguration>,
    /// Constructed market data elements, such as discount curves, volatility surfaces, among others.
    constructed_elements: ConstructedElementStore,
    /// The base currency for pricing and reporting results.
    base_currency: Currency,
    /// Base remuneration index
    base_index: MarketIndex,
}

impl PricingContext {
    /// Creates a new pricing data context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            quote_store: QuoteStore::default(),
            fixing_store: FixingStore::default(),
            fx_store: FxStore::default(),
            curve_configurations: Vec::new(),
            volatility_surface_configurations: Vec::new(),
            volatility_cube_configurations: Vec::new(),
            constructed_elements: ConstructedElementStore::default(),
            base_currency: Currency::USD, // Default base currency
            base_index: MarketIndex::SOFR,
        }
    }

    /// Returns the market data store.
    #[must_use]
    pub const fn quote_store(&self) -> &QuoteStore {
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
        self
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
    pub fn constructed_elements_mut(&mut self) -> &mut ConstructedElementStore {
        &mut self.constructed_elements
    }

    /// Returns the base currency.
    #[must_use]
    pub const fn base_currency(&self) -> Currency {
        self.base_currency
    }

    /// Returns the base index.
    #[must_use]
    pub fn base_index(&self) -> &MarketIndex {
        &self.base_index
    }

    /// Placeholder for one-time initialisation (pre-loading caches, etc.).
    ///
    /// # Errors
    /// Returns an error if bootstrapping or volatility surface construction fails.
    pub fn initialize(&mut self) -> Result<()> {
        // Bootstrap discount curves.
        let policy = BootstrapDiscountPolicy::new(self.base_index.clone(), self.base_currency);
        let bootstrapper = MultiCurveBootstrapper::new(self.curve_configurations.clone(), policy)
            .with_fx_store(self.fx_store.clone());
        let curves = bootstrapper.bootstrap(&self.quote_store, Level::Mid)?;
        for (index, curve) in curves {
            self.constructed_elements
                .discount_curves_mut()
                .insert(index.clone(), curve);
        }

        // Build volatility surfaces.
        if !self.volatility_surface_configurations.is_empty() {
            let surface_builder =
                VolatilitySurfaceBuilder::new(self.volatility_surface_configurations.clone());
            let surfaces = surface_builder.build(&self.quote_store, Level::Mid)?;
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
            let cubes = cube_builder.build(&self.quote_store, Level::Mid)?;
            for (index, cube) in cubes {
                self.constructed_elements
                    .volatility_cubes_mut()
                    .insert(index, cube);
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
                let value = self.fixing_store.fixing(market_index, date)?;
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
        let md = MarketData::new(fixings, constructed_elements).with_fx_store(fx_store);

        Ok(md)
    }
}

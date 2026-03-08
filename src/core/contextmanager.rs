use std::collections::{BTreeMap, HashMap};

use crate::{
    core::{
        collateral::CSADiscountPolicy,
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            constructedelementstore::ConstructedElementStore,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    models::ModelParameters,
    quotes::{fixingstore::FixingStore, quote::Level, quotestore::QuoteStore},
    rates::bootstrapping::curvespec::CurveSpec,
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// Manages the context for instrument evaluation, including market data access, quote level preferences,
/// base currency settings, and a list of model parameter sets for multiple model types.
pub struct ContextManager {
    /// The quote store provides access to direct market data quotes and reference date information.
    quote_store: QuoteStore,
    /// The fixing store provides access to historical fixing values for indices and other reference data.
    fixing_store: FixingStore,
    /// The quote level indicates the preferred type of quote (e.g., bid, ask, mid) to be used for market value extraction during pricing.
    quote_level: Level,
    /// The discount policy defines the approach for discounting cashflows.
    #[allow(dead_code)]
    discount_policy: Option<CSADiscountPolicy>,
    /// Base currency for reporting results, allowing for consistent presentation of pricing outputs across different instruments and markets.
    base_currency: Currency,
    /// Model parameters for various models that may be used during pricing, allowing for flexible configuration of model inputs and assumptions.
    models: Vec<ModelParameters>,
    /// Curve specifications for curve construction.
    #[allow(dead_code)]
    curve_specs: Vec<CurveSpec>,

    /// Constructed market data elements, such as discount curves, dividend curves, volatility surfaces, and simulations, that have been built in response to market data requests. This allows for caching and reuse of constructed elements across multiple pricing operations.
    constructed_elements: ConstructedElementStore,
    // Pricer configuration settings, such as the base currency for reporting results.
    // pricer_config: PricerConfig,
}

impl ContextManager {
    /// Creates a new pricing data context.
    #[must_use]
    pub fn new(quote_store: QuoteStore, fixing_store: FixingStore) -> Self {
        Self {
            quote_store,
            fixing_store,
            quote_level: Level::Mid,
            discount_policy: None,
            models: Vec::new(),
            base_currency: Currency::USD,
            curve_specs: Vec::new(),
            constructed_elements: ConstructedElementStore::default(),
        }
    }

    /// Sets the quote level used for market value extraction.
    #[must_use]
    pub const fn with_quote_level(mut self, quote_level: Level) -> Self {
        self.quote_level = quote_level;
        self
    }

    /// Returns the market data provider.
    #[must_use]
    pub const fn quote_store(&self) -> &QuoteStore {
        &self.quote_store
    }

    /// Returns the fixings provider.
    #[must_use]
    pub const fn fixing_store(&self) -> &FixingStore {
        &self.fixing_store
    }

    /// Returns the quote level preference.
    #[must_use]
    pub const fn quote_level(&self) -> Level {
        self.quote_level
    }

    /// Returns the base currency for reporting.
    #[must_use]
    pub const fn base_currency(&self) -> Currency {
        self.base_currency
    }

    /// Sets the base currency.
    #[must_use]
    pub const fn with_base_currency(mut self, base_currency: Currency) -> Self {
        self.base_currency = base_currency;
        self
    }

    /// Returns the current reference date.
    #[must_use]
    pub const fn evaluation_date(&self) -> Date {
        self.quote_store.reference_date()
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

    /// Sets the model parameter list, replacing any previously registered models.
    #[must_use]
    pub fn with_models(mut self, models: &[ModelParameters]) -> Self {
        models.clone_into(&mut self.models);
        self
    }

    /// Returns the full list of model parameters registered in this context.
    #[must_use]
    pub fn models(&self) -> &[ModelParameters] {
        &self.models
    }
}

impl MarketDataProvider for ContextManager {
    fn evaluation_date(&self) -> Date {
        self.quote_store.reference_date()
    }

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

        // 3. Assemble final MarketData with models from this context.
        Ok(MarketData::new(fixings, constructed_elements, &self.models))
    }
}

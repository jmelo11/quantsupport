use std::collections::{BTreeMap, HashMap};

use crate::{
    core::{
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            constructedelementstore::ConstructedElementStore, fixingrequest::FixingRequest,
            fxrequest::FxRequest,
        },
        pricerconfiguration::{InstrumentType, PricerConfiguration},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    quotes::{fxstore::FxStore, quote::Level, quotestore::QuoteStore},
    time::date::Date,
    utils::errors::Result,
    volatility::modelcalibration::ModelCalibrationConfiguration,
};

/// Request for market data, including constructed elements, fixings, FX rates,
/// and instrument-level pricer configurations.
#[derive(Default)]
#[allow(clippy::struct_field_names)]
pub struct MarketDataRequest {
    constructed_elements_request: Option<Vec<ConstructedElementRequest>>,
    fixings_request: Option<Vec<FixingRequest>>,
    fx_request: Option<Vec<FxRequest>>,
    pricer_configuration_request: Option<Vec<InstrumentType>>,
    calibration_request: Option<ModelCalibrationConfiguration>,
}

impl MarketDataRequest {
    /// Creates a new `MarketDataRequest` with the specified constructed elements and fixings requests.
    #[must_use]
    pub const fn new(
        constructed_elements_request: Option<Vec<ConstructedElementRequest>>,
        fixings_request: Option<Vec<FixingRequest>>,
        fx_request: Option<Vec<FxRequest>>,
    ) -> Self {
        Self {
            constructed_elements_request,
            fixings_request,
            fx_request,
            pricer_configuration_request: None,
            calibration_request: None,
        }
    }

    /// Returns the constructed elements request, if any.
    #[must_use]
    pub const fn constructed_elements_request(&self) -> Option<&Vec<ConstructedElementRequest>> {
        self.constructed_elements_request.as_ref()
    }

    /// Returns the fixings request, if any.
    #[must_use]
    pub const fn fixings_request(&self) -> Option<&Vec<FixingRequest>> {
        self.fixings_request.as_ref()
    }

    /// Builder method to set the constructed elements request.
    #[must_use]
    pub fn with_constructed_elements_request(
        mut self,
        request: Vec<ConstructedElementRequest>,
    ) -> Self {
        self.constructed_elements_request = Some(request);
        self
    }

    /// Builder method to set the fixings request.
    #[must_use]
    pub fn with_fixings_request(mut self, request: Vec<FixingRequest>) -> Self {
        self.fixings_request = Some(request);
        self
    }

    /// Returns the FX rate request, if any.
    #[must_use]
    pub const fn fx_request(&self) -> Option<&Vec<FxRequest>> {
        self.fx_request.as_ref()
    }

    /// Builder method to set the FX rate request.
    #[must_use]
    pub fn with_fx_request(mut self, request: Vec<FxRequest>) -> Self {
        self.fx_request = Some(request);
        self
    }

    /// Returns requested instrument-level pricer configurations, if any.
    #[must_use]
    pub const fn pricer_configuration_request(&self) -> Option<&Vec<InstrumentType>> {
        self.pricer_configuration_request.as_ref()
    }

    /// Requests the typed pricer configurations for the supplied instrument
    /// families.
    #[must_use]
    pub fn with_pricer_configuration_request(mut self, request: Vec<InstrumentType>) -> Self {
        self.pricer_configuration_request = Some(request);
        self
    }

    /// Returns the basket whose instrument quote details are required.
    #[must_use]
    pub const fn calibration_request(&self) -> Option<&ModelCalibrationConfiguration> {
        self.calibration_request.as_ref()
    }

    /// Requests quote details for a model calibration basket. Its surface or
    /// cube and calibration curve must also be requested as constructed elements.
    #[must_use]
    pub fn with_calibration_request(mut self, request: ModelCalibrationConfiguration) -> Self {
        self.calibration_request = Some(request);
        self
    }
}

/// Market-data response containing only the curves, fixings, FX rates, and
/// pricer configurations requested for one valuation.
#[derive(Clone)]
pub struct MarketData {
    base_currency: Option<Currency>,
    fixings: HashMap<MarketIndex, BTreeMap<Date, f64>>,
    fx_store: Option<FxStore>,
    constructed_elements: ConstructedElementStore,
    pricer_configurations: HashMap<InstrumentType, PricerConfiguration>,
    calibration_quotes: Option<QuoteStore>,
    quote_level: Level,
    anchor_fixings: bool,
}

impl Default for MarketData {
    fn default() -> Self {
        Self::new(HashMap::new(), ConstructedElementStore::default())
    }
}

impl MarketData {
    /// Creates market data with the supplied fixings and constructed elements.
    #[must_use]
    pub fn new(
        fixings: HashMap<MarketIndex, BTreeMap<Date, f64>>,
        constructed_elements: ConstructedElementStore,
    ) -> Self {
        Self {
            base_currency: None,
            fixings,
            constructed_elements,
            fx_store: None,
            pricer_configurations: HashMap::new(),
            calibration_quotes: None,
            quote_level: Level::Mid,
            anchor_fixings: true,
        }
    }

    /// Whether historical index observations use required fixing-store values.
    #[must_use]
    pub const fn anchor_fixings(&self) -> bool {
        self.anchor_fixings
    }

    /// Selects historical fixing anchoring; false projects every index observation.
    #[must_use]
    pub const fn with_anchor_fixings(mut self, enabled: bool) -> Self {
        self.anchor_fixings = enabled;
        self
    }

    /// Returns the reporting currency supplied by the valuation context.
    /// Providers without a reporting currency leave monetary values in their native units.
    #[must_use]
    pub const fn base_currency(&self) -> Option<Currency> {
        self.base_currency
    }

    /// Attaches the context's reporting currency to this valuation's market data.
    #[must_use]
    pub const fn with_base_currency(mut self, currency: Currency) -> Self {
        self.base_currency = Some(currency);
        self
    }

    /// Returns the fixings.
    #[must_use]
    pub const fn fixings(&self) -> &HashMap<MarketIndex, BTreeMap<Date, f64>> {
        &self.fixings
    }

    /// Returns the constructed elements.
    #[must_use]
    pub const fn constructed_elements(&self) -> &ConstructedElementStore {
        &self.constructed_elements
    }

    /// Returns mutable reference to the constructed elements.
    #[must_use]
    pub const fn constructed_elements_mut(&mut self) -> &mut ConstructedElementStore {
        &mut self.constructed_elements
    }

    /// Returns the exchange-rate store, if any.
    #[must_use]
    pub const fn fx_store(&self) -> Option<&FxStore> {
        self.fx_store.as_ref()
    }

    /// Returns a mutable reference to the exchange-rate store, if any.
    #[must_use]
    pub const fn fx_store_mut(&mut self) -> Option<&mut FxStore> {
        self.fx_store.as_mut()
    }

    /// Builder method to set the exchange-rate store.
    #[must_use]
    pub fn with_fx_store(mut self, store: FxStore) -> Self {
        self.fx_store = Some(store);
        self
    }

    /// Returns requested configurations by instrument family.
    #[must_use]
    pub const fn pricer_configurations(&self) -> &HashMap<InstrumentType, PricerConfiguration> {
        &self.pricer_configurations
    }

    /// Attaches typed instrument-level pricer configurations.
    #[must_use]
    pub fn with_pricer_configurations(
        mut self,
        configurations: HashMap<InstrumentType, PricerConfiguration>,
    ) -> Self {
        self.pricer_configurations = configurations;
        self
    }

    /// Returns the selected calibration instruments' market quote details.
    #[must_use]
    pub const fn calibration_quotes(&self) -> Option<&QuoteStore> {
        self.calibration_quotes.as_ref()
    }

    /// Attaches calibration quote details supplied through the provider.
    #[must_use]
    pub fn with_calibration_quotes(mut self, quotes: QuoteStore) -> Self {
        self.calibration_quotes = Some(quotes);
        self
    }

    /// Returns the quote level used to calibrate models for this valuation.
    #[must_use]
    pub const fn quote_level(&self) -> Level {
        self.quote_level
    }

    /// Sets the quote level used for model calibration. Defaults to mid.
    #[must_use]
    pub const fn with_quote_level(mut self, quote_level: Level) -> Self {
        self.quote_level = quote_level;
        self
    }
}

/// Provider interface for market-data requests.
pub trait MarketDataProvider {
    /// Whether observations before evaluation require historical fixings.
    /// Today and future observations are projected. False projects all dates.
    fn anchor_fixings(&self) -> bool {
        true
    }

    /// Handles a market-data request.
    ///
    /// ## Errors
    /// Returns an error if the market data request cannot be fulfilled.
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData>;

    /// Returns provider evaluation date.
    fn evaluation_date(&self) -> Date;
}

impl<P: MarketDataProvider + ?Sized> MarketDataProvider for &P {
    fn anchor_fixings(&self) -> bool {
        (**self).anchor_fixings()
    }

    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData> {
        (**self).handle_request(request)
    }
    fn evaluation_date(&self) -> Date {
        (**self).evaluation_date()
    }
}

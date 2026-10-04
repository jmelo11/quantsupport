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
    indices::marketindex::MarketIndex,
    quotes::{fxstore::FxStore, quotestore::QuoteStore},
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
#[derive(Clone, Default)]
pub struct MarketData {
    fixings: HashMap<MarketIndex, BTreeMap<Date, f64>>,
    fx_store: Option<FxStore>,
    constructed_elements: ConstructedElementStore,
    pricer_configurations: HashMap<InstrumentType, PricerConfiguration>,
    calibration_quotes: Option<QuoteStore>,
}

impl MarketData {
    /// Creates market data with the supplied fixings and constructed elements.
    #[must_use]
    pub fn new(
        fixings: HashMap<MarketIndex, BTreeMap<Date, f64>>,
        constructed_elements: ConstructedElementStore,
    ) -> Self {
        Self {
            fixings,
            constructed_elements,
            fx_store: None,
            pricer_configurations: HashMap::new(),
            calibration_quotes: None,
        }
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
}

/// Provider interface for market-data requests.
pub trait MarketDataProvider {
    /// Handles a market-data request.
    ///
    /// ## Errors
    /// Returns an error if the market data request cannot be fulfilled.
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData>;

    /// Returns provider evaluation date.
    fn evaluation_date(&self) -> Date;
}

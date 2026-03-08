use std::collections::{BTreeMap, HashMap};

use crate::{
    core::marketdatahandling::{
        constructedelementrequest::ConstructedElementRequest,
        constructedelementstore::ConstructedElementStore, fixingrequest::FixingRequest,
    },
    currencies::exchangeratestore::ExchangeRateStore,
    indices::marketindex::MarketIndex,
    models::ModelParameters,
    time::date::Date,
    utils::errors::Result,
};

/// Request for market data, including constructed elements, fixings, and an optional list
/// of model parameter sets that the provider may inspect when fulfilling the request.
#[derive(Default)]
pub struct MarketDataRequest {
    constructed_elements_request: Option<Vec<ConstructedElementRequest>>,
    fixings_request: Option<Vec<FixingRequest>>,
    models: Vec<ModelParameters>,
    /// Whether the pricer needs an exchange-rate store for FX conversions.
    needs_exchange_rates: bool,
}

impl MarketDataRequest {
    /// Creates a new `MarketDataRequest` with the specified constructed elements and fixings requests.
    #[must_use]
    pub const fn new(
        constructed_elements_request: Option<Vec<ConstructedElementRequest>>,
        fixings_request: Option<Vec<FixingRequest>>,
    ) -> Self {
        Self {
            constructed_elements_request,
            fixings_request,
            models: Vec::new(),
            needs_exchange_rates: false,
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

    /// Returns the list of model parameters attached to this request.
    #[must_use]
    pub fn models(&self) -> &[ModelParameters] {
        &self.models
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

    /// Returns whether the pricer needs an exchange-rate store.
    #[must_use]
    pub const fn needs_exchange_rates(&self) -> bool {
        self.needs_exchange_rates
    }

    /// Builder method to indicate that an exchange-rate store is required.
    #[must_use]
    pub const fn with_exchange_rates(mut self) -> Self {
        self.needs_exchange_rates = true;
        self
    }

    /// Builder method to set the model parameter list, replacing any previously set models.
    #[must_use]
    pub fn with_models(mut self, models: &[ModelParameters]) -> Self {
        models.clone_into(&mut self.models);
        self
    }
}

/// Struct representing market data, including fixings, constructed elements, a list of
/// model parameter sets, and an optional exchange-rate store for FX conversions.
#[derive(Clone, Default)]
pub struct MarketData {
    fixings: HashMap<MarketIndex, BTreeMap<Date, f64>>,
    constructed_elements: ConstructedElementStore,
    models: Vec<ModelParameters>,
    exchange_rate_store: Option<ExchangeRateStore>,
}

impl MarketData {
    /// Creates a new `MarketData` with the specified fixings, constructed elements, and model
    /// parameter sets.
    #[must_use]
    pub fn new(
        fixings: HashMap<MarketIndex, BTreeMap<Date, f64>>,
        constructed_elements: ConstructedElementStore,
        models: &[ModelParameters],
    ) -> Self {
        Self {
            fixings,
            constructed_elements,
            models: models.to_owned(),
            exchange_rate_store: None,
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

    /// Returns the list of model parameters attached to this market data.
    #[must_use]
    pub fn models(&self) -> &[ModelParameters] {
        &self.models
    }

    /// Builder method to set the model parameter list, replacing any previously set models.
    #[must_use]
    pub fn with_models(mut self, models: &[ModelParameters]) -> Self {
        models.clone_into(&mut self.models);
        self
    }

    /// Returns the exchange-rate store, if any.
    #[must_use]
    pub const fn exchange_rate_store(&self) -> Option<&ExchangeRateStore> {
        self.exchange_rate_store.as_ref()
    }

    /// Returns a mutable reference to the exchange-rate store, if any.
    #[must_use]
    pub const fn exchange_rate_store_mut(&mut self) -> Option<&mut ExchangeRateStore> {
        self.exchange_rate_store.as_mut()
    }

    /// Builder method to set the exchange-rate store.
    #[must_use]
    pub fn with_exchange_rate_store(mut self, store: ExchangeRateStore) -> Self {
        self.exchange_rate_store = Some(store);
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

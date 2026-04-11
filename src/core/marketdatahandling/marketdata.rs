use std::collections::{BTreeMap, HashMap};

use crate::{
    core::marketdatahandling::{
        constructedelementrequest::ConstructedElementRequest,
        constructedelementstore::ConstructedElementStore, fixingrequest::FixingRequest,
        fxrequest::FxRequest,
    },
    indices::marketindex::MarketIndex,
    quotes::fxstore::FxStore,
    time::date::Date,
    utils::errors::Result,
};

/// Request for market data, including constructed elements, fixings, and an optional list
/// of model parameter sets that the provider may inspect when fulfilling the request.
#[derive(Default)]
#[allow(clippy::struct_field_names)]
pub struct MarketDataRequest {
    constructed_elements_request: Option<Vec<ConstructedElementRequest>>,
    fixings_request: Option<Vec<FixingRequest>>,
    fx_request: Option<Vec<FxRequest>>,
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
}

/// Struct representing market data, including fixings, constructed elements, a list of
/// model parameter sets, and an optional exchange-rate store for FX conversions.
#[derive(Clone, Default)]
pub struct MarketData {
    fixings: HashMap<MarketIndex, BTreeMap<Date, f64>>,
    fx_store: Option<FxStore>,
    constructed_elements: ConstructedElementStore,
}

impl MarketData {
    /// Creates a new [`MarketData`] with the specified fixings, constructed elements, and model
    /// parameter sets.
    #[must_use]
    pub const fn new(
        fixings: HashMap<MarketIndex, BTreeMap<Date, f64>>,
        constructed_elements: ConstructedElementStore,
    ) -> Self {
        Self {
            fixings,
            constructed_elements,
            fx_store: None,
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

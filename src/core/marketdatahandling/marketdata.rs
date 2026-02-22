use std::collections::{BTreeMap, HashMap};

use crate::{
    core::marketdatahandling::{
        constructedelementrequest::ConstructedElementRequest,
        constructedelementstore::ConstructedElementStore, fixingrequest::FixingRequest,
    },
    indices::marketindex::MarketIndex,
    models::GbmModelParameters,
    time::date::Date,
    utils::errors::Result,
};

/// # `MarketDataRequest`
///
/// Request for market data, including constructed elements and fixings.
#[derive(Default)]
pub struct MarketDataRequest {
    constructed_elements_request: Option<Vec<ConstructedElementRequest>>,
    fixings_request: Option<Vec<FixingRequest>>,
    model_parameters: Option<GbmModelParameters>,
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
            model_parameters: None,
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

    /// Returns the model parameters included in this request, if any.
    #[must_use]
    pub const fn model_parameters(&self) -> Option<&GbmModelParameters> {
        self.model_parameters.as_ref()
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

    /// Builder method to set the model parameters.
    #[must_use]
    pub const fn with_model_parameters(mut self, params: GbmModelParameters) -> Self {
        self.model_parameters = Some(params);
        self
    }
}

/// # `MarketData`
///
/// Struct representing market data, including fixings, constructed elements, and optional model parameters.
pub struct MarketData {
    fixings: HashMap<MarketIndex, BTreeMap<Date, f64>>,
    constructed_elements: ConstructedElementStore,
    model_parameters: Option<GbmModelParameters>,
}

impl MarketData {
    /// Creates a new `MarketData` with the specified fixings and constructed elements.
    #[must_use]
    pub const fn new(
        fixings: HashMap<MarketIndex, BTreeMap<Date, f64>>,
        constructed_elements: ConstructedElementStore,
    ) -> Self {
        Self {
            fixings,
            constructed_elements,
            model_parameters: None,
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

    /// Returns the model parameters, if any.
    #[must_use]
    pub const fn model_parameters(&self) -> Option<&GbmModelParameters> {
        self.model_parameters.as_ref()
    }

    /// Builder method to attach model parameters.
    #[must_use]
    pub const fn with_model_parameters(mut self, params: GbmModelParameters) -> Self {
        self.model_parameters = Some(params);
        self
    }
}

/// # `MarketDataProvider`
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

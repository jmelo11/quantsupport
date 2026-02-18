use std::collections::HashMap;

use crate::{
    core::marketdatarequest::{
        curveelement::{DiscountCurveElement, DividendCurveElement},
        fixingrequest::FixingRequest,
        simulationelement::SimulationElement,
        volatilityelements::{VolatilityAxis, VolatilityCubeElement, VolatilitySurfaceElement},
    },
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::Result,
};

/// `DerivedElementRequest`
///
/// Enum representing different types of market data elements that can be
/// requested from a [`MarketDataProvider`].
pub enum DerivedElementRequest {
    /// Request for a discount curve associated with a specific market index.
    DiscountCurve {
        /// The market index for which the discount curve is requested.
        market_index: MarketIndex,
    },
    /// Request for a dividend curve associated with a specific market index.
    DividendCurve {
        /// The market index for which the dividend curve is requested.
        market_index: MarketIndex,
    },
    /// Request for a volatility surface associated with a specific market index.
    VolatilitySurface {
        /// The market index for which the volatility surface is requested.
        market_index: MarketIndex,
    },
    /// Request for a specific node on a volatility surface, identified by market index, date, and axis.
    VolatilityCube {
        /// The market index for which the volatility cube is requested.
        market_index: MarketIndex,
    },
    /// Request for a specific node on a volatility surface, identified by market index, date, and axis.
    VolNode {
        /// The market index for which the volatility node is requested.
        market_index: MarketIndex,
        /// The date for which the volatility node is requested.
        date: Date,
        /// The axis value for which the volatility node is requested (e.g., strike or tenor).
        axis: VolatilityAxis,
    },
    /// Request for a simulation element associated with a specific market index.
    Simulation {
        /// The market index for which the simulation element is requested.
        market_index: MarketIndex,
    },
}

/// `MarketDataRequest`
///
/// Struct representing a request for market data, which includes
/// lists of derived element requests and fixing requests.
#[derive(Default)]
pub struct MarketDataRequest {
    element_requests: Vec<DerivedElementRequest>,
    fixing_requests: Vec<FixingRequest>,
}

impl MarketDataRequest {
    /// Creates a new `MarketDataRequest` with the specified element requests and fixing requests.
    #[must_use]
    pub fn with_element_requests(mut self, element_requests: Vec<DerivedElementRequest>) -> Self {
        self.element_requests = element_requests;
        self
    }

    /// Creates a new `MarketDataRequest` with the specified element requests and fixing requests.
    #[must_use]
    pub fn with_fixing_requests(mut self, fixing_requests: Vec<FixingRequest>) -> Self {
        self.fixing_requests = fixing_requests;
        self
    }

    /// Returns a reference to the list of derived element requests in the market data request.
    #[must_use]
    pub fn element_requests(&self) -> &[DerivedElementRequest] {
        &self.element_requests
    }

    /// Returns a reference to the list of fixing requests in the market data request.
    #[must_use]
    pub fn fixing_requests(&self) -> &[FixingRequest] {
        &self.fixing_requests
    }
}

/// `MarketDataResponse`
///
/// Trait representing a response to a market data request, which includes
/// the requested market data elements such as discount curves, dividend curves, fixings, volatility nodes, and simulations.
/// This trait is designed to be easily extendable to accommodate additional types of market data
pub trait MarketDataResponse {
    fn discount_curves(&self) -> &HashMap<MarketIndex, DiscountCurveElement>;
    fn dividend_curves(&self) -> &HashMap<MarketIndex, DividendCurveElement>;
    fn fixings(&self) -> &HashMap<(MarketIndex, Date), f64>;
    fn volatility_surfaces(&self) -> &HashMap<MarketIndex, VolatilitySurfaceElement>;
    fn volatility_cubes(&self) -> &HashMap<MarketIndex, VolatilityCubeElement>;
    fn simulations(&self) -> &HashMap<MarketIndex, SimulationElement>;
}

/// `MarketDataProvider`
///
/// Trait representing a provider of market data, which can handle requests for various types of market data elements and
pub trait MarketDataProvider {
    /// Handles a market data request and returns a response containing the requested market data elements.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the market data request cannot be fulfilled or if there is an issue with the provided request parameters.
    fn handle_request(&self, request: &MarketDataRequest) -> Result<impl MarketDataResponse>;

    /// Returns the evaluation date for which the market data is relevant.
    fn evaluation_date(&self) -> Date;
}

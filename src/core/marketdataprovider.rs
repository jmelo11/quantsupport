use std::collections::HashMap;

use crate::{
    ad::adreal::ADReal, core::pillars::Pillars, currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    time::date::Date, utils::errors::Result,
};

/// `ADCurveElementClone`
///
/// Trait to enable cloning of boxed [`ADCurveElement`] objects.
pub trait ADCurveElementClone {
    /// Clones the boxed [`ADCurveElement`].
    fn clone_box(&self) -> Box<dyn ADCurveElement>;
}

impl<T> ADCurveElementClone for T
where
    T: 'static + ADCurveElement + Clone,
{
    fn clone_box(&self) -> Box<dyn ADCurveElement> {
        Box::new(self.clone())
    }
}

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
        axis: f64,
    },
    /// Request for a simulation element associated with a specific market index.
    Simulation {
        /// The market index for which the simulation element is requested.
        market_index: MarketIndex,
    },
}

/// `ADCurveElement`
///
/// Trait representing a curve element that can be used in automatic
/// differentiation contexts. It combines the properties of an interest rates
/// term structure and pillars, and allows for cloning.
pub trait ADCurveElement:
    InterestRatesTermStructure<ADReal> + Pillars<ADReal> + Send + Sync + ADCurveElementClone
{
}

impl Clone for Box<dyn ADCurveElement> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// `DiscountCurveElement`
///
/// Struct representing a discount curve element, which includes
/// the associated market index, currency, and the curve itself.
#[derive(Clone)]
pub struct DiscountCurveElement {
    market_index: MarketIndex,
    currency: Currency,
    curve: Box<dyn ADCurveElement>,
}

impl DiscountCurveElement {
    /// Creates a new [`DiscountCurveElement`] with the specified market index, currency, and curve.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        currency: Currency,
        curve: Box<dyn ADCurveElement>,
    ) -> Self {
        Self {
            market_index,
            currency,
            curve,
        }
    }

    /// Returns the market index associated with the discount curve element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the currency associated with the discount curve element.
    #[must_use]
    pub const fn currency(&self) -> &Currency {
        &self.currency
    }

    /// Returns a reference to the curve associated with the discount curve element.
    #[must_use]
    pub fn curve(&self) -> &dyn ADCurveElement {
        self.curve.as_ref()
    }

    /// Returns a mutable reference to the curve associated with the discount curve element.
    #[must_use]
    pub fn curve_mut(&mut self) -> &mut dyn ADCurveElement {
        self.curve.as_mut()
    }
}

/// `DividendCurveElement`
///
/// Struct representing a dividend curve element, which includes
/// the associated market index, currency, and the curve itself.
#[derive(Clone)]
pub struct DividendCurveElement {
    market_index: MarketIndex,
    currency: Currency,
    curve: Box<dyn ADCurveElement>,
}

impl DividendCurveElement {
    /// Creates a new [`DividendCurveElement`] with the specified market index, currency, and curve.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        currency: Currency,
        curve: Box<dyn ADCurveElement>,
    ) -> Self {
        Self {
            market_index,
            currency,
            curve,
        }
    }

    /// Returns the market index associated with the dividend curve element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the currency associated with the dividend curve element.
    #[must_use]
    pub const fn currency(&self) -> &Currency {
        &self.currency
    }

    /// Returns a reference to the curve associated with the dividend curve element.
    #[must_use]
    pub fn curve(&self) -> &dyn ADCurveElement {
        self.curve.as_ref()
    }

    /// Returns a mutable reference to the curve associated with the dividend curve element.
    #[must_use]
    pub fn curve_mut(&mut self) -> &mut dyn ADCurveElement {
        self.curve.as_mut()
    }
}

/// `SimulationElement`
///
/// Struct representing a simulation element, which includes the associated market
/// index and the simulation draws.
#[derive(Clone)]
pub struct SimulationElement {
    market_index: MarketIndex,
    draws: Vec<f64>,
}

impl SimulationElement {
    /// Creates a new [`SimulationElement`] with the specified market index and simulation draws.
    #[must_use]
    pub const fn new(market_index: MarketIndex, draws: Vec<f64>) -> Self {
        Self {
            market_index,
            draws,
        }
    }

    /// Returns the market index associated with the simulation element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }
    /// Returns a reference to the simulation draws associated with the simulation element.
    #[must_use]
    pub fn draws(&self) -> &[f64] {
        &self.draws
    }
}

/// `FixingRequest`
///
/// Struct representing a request for a fixing, which includes the market index and date for which the fixing is requested.
pub struct FixingRequest {
    market_index: MarketIndex,
    date: Date,
}

impl FixingRequest {
    /// Creates a new `FixingRequest` with the specified market index and date.
    #[must_use]
    pub const fn new(market_index: MarketIndex, date: Date) -> Self {
        Self { market_index, date }
    }

    /// Returns the market index associated with the fixing request.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the date associated with the fixing request.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }
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

/// `VolNodeKey`
///
/// Struct representing a key for identifying a specific node on a volatility surface,
/// based on market index, date, and axis value.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct VolatilityNode {
    market_index: MarketIndex,
    date: Date,
    axis_bits: u64,
}

impl VolatilityNode {
    /// Creates a new `VolNodeKey` with the specified market index, date, and axis value.
    #[must_use]
    pub const fn new(market_index: MarketIndex, date: Date, axis: f64) -> Self {
        Self {
            market_index,
            date,
            axis_bits: axis.to_bits(),
        }
    }
}

/// `MarketDataResponse`
///
/// Struct representing a response to a market data request, which includes
/// the requested market data elements such as discount curves, dividend curves, fixings, volatility nodes, and simulations.
/// This struct is designed to be easily extendable to accommodate additional types of market data in the future.
#[derive(Default)]
pub struct MarketDataResponse {
    discount_curves: HashMap<MarketIndex, DiscountCurveElement>,
    dividend_curves: HashMap<MarketIndex, DividendCurveElement>,
    fixings: HashMap<(MarketIndex, Date), f64>,
    volatility_nodes: HashMap<VolatilityNode, ADReal>,
    simulations: HashMap<MarketIndex, SimulationElement>,
}

impl MarketDataResponse {
    /// Returns a mutable reference to the discount curves included in the market data response.
    #[must_use]
    pub const fn discount_curves_mut(&mut self) -> &mut HashMap<MarketIndex, DiscountCurveElement> {
        &mut self.discount_curves
    }

    /// Returns a mutable reference to the dividend curves included in the market data response.
    #[must_use]
    pub const fn dividend_curves_mut(&mut self) -> &mut HashMap<MarketIndex, DividendCurveElement> {
        &mut self.dividend_curves
    }

    /// Returns a mutable reference to the fixings included in the market data response.
    #[must_use]
    pub const fn fixings_mut(&mut self) -> &mut HashMap<(MarketIndex, Date), f64> {
        &mut self.fixings
    }

    /// Returns a mutable reference to the volatility nodes included in the market data response.
    #[must_use]
    pub const fn vol_nodes_mut(&mut self) -> &mut HashMap<VolatilityNode, ADReal> {
        &mut self.volatility_nodes
    }

    /// Returns a mutable reference to the simulations included in the market data response.
    #[must_use]
    pub const fn simulations_mut(&mut self) -> &mut HashMap<MarketIndex, SimulationElement> {
        &mut self.simulations
    }

    /// Returns a reference to the discount curves included in the market data response.
    #[must_use]
    pub const fn discount_curves(&self) -> &HashMap<MarketIndex, DiscountCurveElement> {
        &self.discount_curves
    }

    /// Returns a reference to the dividend curves included in the market data response.
    #[must_use]
    pub const fn dividend_curves(&self) -> &HashMap<MarketIndex, DividendCurveElement> {
        &self.dividend_curves
    }

    /// Returns a reference to the fixings included in the market data response.
    #[must_use]
    pub const fn fixings(&self) -> &HashMap<(MarketIndex, Date), f64> {
        &self.fixings
    }

    /// Returns a reference to the volatility nodes included in the market data response.
    #[must_use]
    pub const fn vol_nodes(&self) -> &HashMap<VolatilityNode, ADReal> {
        &self.volatility_nodes
    }

    /// Returns a reference to the simulations included in the market data response.
    #[must_use]
    pub const fn simulations(&self) -> &HashMap<MarketIndex, SimulationElement> {
        &self.simulations
    }
}

/// `MarketDataProvider`
///
/// Trait representing a provider of market data, which can handle requests for various types of market data elements and
pub trait MarketDataProvider {
    /// Handles a market data request and returns a response containing the requested market data elements.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the market data request cannot be fulfilled or if there is an issue with the provided request parameters.
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketDataResponse>;

    /// Returns the evaluation date for which the market data is relevant.
    fn evaluation_date(&self) -> Date;
}

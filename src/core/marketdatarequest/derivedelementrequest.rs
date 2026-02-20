use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

use crate::{
    core::marketdatarequest::{
        curveelement::{DiscountCurveElement, DividendCurveElement},
        fixingrequest::FixingRequest,
        simulationelement::SimulationElement,
        volatilityelements::{
            VolatilityAxis, VolatilityCubeElement, VolatilityNode, VolatilityNodeKey,
            VolatilitySurfaceElement,
        },
    },
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::Result,
};

/// # `DerivedElementRequest`
/// Request for a specific derived market-data element.
pub enum DerivedElementRequest {
    /// Request for discount curve of a market index.
    DiscountCurve {
        /// Requested market index.
        market_index: MarketIndex,
    },
    /// Request for dividend curve of a market index.
    DividendCurve {
        /// Requested market index.
        market_index: MarketIndex,
    },
    /// Request for volatility surface of a market index.
    VolatilitySurface {
        /// Requested market index.
        market_index: MarketIndex,
    },
    /// Request for volatility cube of a market index.
    VolatilityCube {
        /// Requested market index.
        market_index: MarketIndex,
    },
    /// Request for a volatility node.
    VolNode {
        /// Requested market index.
        market_index: MarketIndex,
        /// Expiry date coordinate.
        date: Date,
        /// Smile axis coordinate.
        axis: VolatilityAxis,
    },
    /// Request for simulation data of a market index.
    Simulation {
        /// Requested market index.
        market_index: MarketIndex,
    },
}

/// # `MarketDataRequest`
/// Batch request sent to a market-data provider.
#[derive(Default)]
pub struct MarketDataRequest {
    element_requests: Vec<DerivedElementRequest>,
    fixing_requests: Vec<FixingRequest>,
}

impl MarketDataRequest {
    /// Sets element requests.
    #[must_use]
    pub fn with_element_requests(mut self, element_requests: Vec<DerivedElementRequest>) -> Self {
        self.element_requests = element_requests;
        self
    }

    /// Sets fixing requests.
    #[must_use]
    pub fn with_fixing_requests(mut self, fixing_requests: Vec<FixingRequest>) -> Self {
        self.fixing_requests = fixing_requests;
        self
    }

    /// Returns requested derived elements.
    #[must_use]
    pub fn element_requests(&self) -> &[DerivedElementRequest] {
        &self.element_requests
    }

    /// Returns requested fixings.
    #[must_use]
    pub fn fixing_requests(&self) -> &[FixingRequest] {
        &self.fixing_requests
    }
}

/// Shared pointer to a mutable market-data element.
pub type SharedElement<T> = Rc<RefCell<T>>;

/// # `MarketDataResponse`
/// Concrete market-data response with read/write accessors.
#[derive(Clone, Default)]
pub struct MarketDataResponse {
    discount_curves: HashMap<MarketIndex, SharedElement<DiscountCurveElement>>,
    dividend_curves: HashMap<MarketIndex, SharedElement<DividendCurveElement>>,
    fixings: HashMap<(MarketIndex, Date), f64>,
    volatility_nodes: HashMap<VolatilityNodeKey, VolatilityNode>,
    volatility_surfaces: HashMap<MarketIndex, SharedElement<VolatilitySurfaceElement>>,
    volatility_cubes: HashMap<MarketIndex, SharedElement<VolatilityCubeElement>>,
    simulations: HashMap<MarketIndex, SharedElement<SimulationElement>>,
}

impl MarketDataResponse {
    /// Returns discount curves.
    #[must_use]
    pub const fn discount_curves(
        &self,
    ) -> &HashMap<MarketIndex, SharedElement<DiscountCurveElement>> {
        &self.discount_curves
    }

    /// Returns mutable discount curves map.
    #[must_use]
    pub fn discount_curves_mut(
        &mut self,
    ) -> &mut HashMap<MarketIndex, SharedElement<DiscountCurveElement>> {
        &mut self.discount_curves
    }

    /// Returns dividend curves.
    #[must_use]
    pub const fn dividend_curves(
        &self,
    ) -> &HashMap<MarketIndex, SharedElement<DividendCurveElement>> {
        &self.dividend_curves
    }

    /// Returns mutable dividend curves map.
    #[must_use]
    pub fn dividend_curves_mut(
        &mut self,
    ) -> &mut HashMap<MarketIndex, SharedElement<DividendCurveElement>> {
        &mut self.dividend_curves
    }

    /// Returns fixings.
    #[must_use]
    pub const fn fixings(&self) -> &HashMap<(MarketIndex, Date), f64> {
        &self.fixings
    }

    /// Returns mutable fixings map.
    #[must_use]
    pub fn fixings_mut(&mut self) -> &mut HashMap<(MarketIndex, Date), f64> {
        &mut self.fixings
    }

    /// Returns resolved volatility nodes.
    #[must_use]
    pub const fn volatility_nodes(&self) -> &HashMap<VolatilityNodeKey, VolatilityNode> {
        &self.volatility_nodes
    }

    /// Returns mutable resolved volatility nodes.
    #[must_use]
    pub fn volatility_nodes_mut(&mut self) -> &mut HashMap<VolatilityNodeKey, VolatilityNode> {
        &mut self.volatility_nodes
    }

    /// Returns volatility surfaces.
    #[must_use]
    pub const fn volatility_surfaces(
        &self,
    ) -> &HashMap<MarketIndex, SharedElement<VolatilitySurfaceElement>> {
        &self.volatility_surfaces
    }

    /// Returns mutable volatility surfaces map.
    #[must_use]
    pub fn volatility_surfaces_mut(
        &mut self,
    ) -> &mut HashMap<MarketIndex, SharedElement<VolatilitySurfaceElement>> {
        &mut self.volatility_surfaces
    }

    /// Returns volatility cubes.
    #[must_use]
    pub const fn volatility_cubes(
        &self,
    ) -> &HashMap<MarketIndex, SharedElement<VolatilityCubeElement>> {
        &self.volatility_cubes
    }

    /// Returns mutable volatility cubes map.
    #[must_use]
    pub fn volatility_cubes_mut(
        &mut self,
    ) -> &mut HashMap<MarketIndex, SharedElement<VolatilityCubeElement>> {
        &mut self.volatility_cubes
    }

    /// Returns simulations.
    #[must_use]
    pub const fn simulations(
        &self,
    ) -> &HashMap<MarketIndex, SharedElement<SimulationElement>> {
        &self.simulations
    }

    /// Returns mutable simulations map.
    #[must_use]
    pub fn simulations_mut(
        &mut self,
    ) -> &mut HashMap<MarketIndex, SharedElement<SimulationElement>> {
        &mut self.simulations
    }

    /// Gets one discount curve by index.
    #[must_use]
    pub fn discount_curve(&self, index: &MarketIndex) -> Option<&SharedElement<DiscountCurveElement>> {
        self.discount_curves.get(index)
    }

    /// Gets one dividend curve by index.
    #[must_use]
    pub fn dividend_curve(&self, index: &MarketIndex) -> Option<&SharedElement<DividendCurveElement>> {
        self.dividend_curves.get(index)
    }

    /// Gets one volatility surface by index.
    #[must_use]
    pub fn volatility_surface(&self, index: &MarketIndex) -> Option<&SharedElement<VolatilitySurfaceElement>> {
        self.volatility_surfaces.get(index)
    }

    /// Gets one volatility cube by index.
    #[must_use]
    pub fn volatility_cube(&self, index: &MarketIndex) -> Option<&SharedElement<VolatilityCubeElement>> {
        self.volatility_cubes.get(index)
    }
}

/// # `MarketDataProvider`
/// Provider interface for market-data requests.
pub trait MarketDataProvider {
    /// Handles a market-data request.
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketDataResponse>;

    /// Returns provider evaluation date.
    fn evaluation_date(&self) -> Date;
}

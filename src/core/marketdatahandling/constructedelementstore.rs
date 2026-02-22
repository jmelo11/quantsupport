use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    core::elements::{
        curveelement::{DiscountCurveElement, DividendCurveElement},
        simulationelement::SimulationElement,
        volatilitycubelement::VolatilityCubeElement,
        volatilitysurfaceelement::VolatilitySurfaceElement,
    },
    indices::marketindex::MarketIndex,
};

/// Type alias for a shared element using reference counting and interior mutability.
pub type SharedElement<T> = Rc<RefCell<T>>;

/// # `ConstructedElementStore`
///
/// Struct representing a store for constructed market data elements, including discount curves, dividend curves,
/// volatility surfaces, volatility cubes, and simulations.
#[derive(Clone, Default)]
pub struct ConstructedElementStore {
    discount_curves: HashMap<MarketIndex, DiscountCurveElement>,
    dividend_curves: HashMap<MarketIndex, DividendCurveElement>,
    volatility_surfaces: HashMap<MarketIndex, VolatilitySurfaceElement>,
    volatility_cubes: HashMap<MarketIndex, VolatilityCubeElement>,
    simulations: HashMap<MarketIndex, SimulationElement>,
}

impl ConstructedElementStore {
    /// Returns discount curves.
    #[must_use]
    pub const fn discount_curves(&self) -> &HashMap<MarketIndex, DiscountCurveElement> {
        &self.discount_curves
    }

    /// Returns mutable discount curves map.
    #[must_use]
    pub const fn discount_curves_mut(&mut self) -> &mut HashMap<MarketIndex, DiscountCurveElement> {
        &mut self.discount_curves
    }

    /// Returns dividend curves.
    #[must_use]
    pub const fn dividend_curves(&self) -> &HashMap<MarketIndex, DividendCurveElement> {
        &self.dividend_curves
    }

    /// Returns mutable dividend curves map.
    #[must_use]
    pub const fn dividend_curves_mut(&mut self) -> &mut HashMap<MarketIndex, DividendCurveElement> {
        &mut self.dividend_curves
    }

    /// Returns volatility surfaces.
    #[must_use]
    pub const fn volatility_surfaces(&self) -> &HashMap<MarketIndex, VolatilitySurfaceElement> {
        &self.volatility_surfaces
    }

    /// Returns mutable volatility surfaces map.
    #[must_use]
    pub const fn volatility_surfaces_mut(
        &mut self,
    ) -> &mut HashMap<MarketIndex, VolatilitySurfaceElement> {
        &mut self.volatility_surfaces
    }

    /// Returns volatility cubes.
    #[must_use]
    pub const fn volatility_cubes(&self) -> &HashMap<MarketIndex, VolatilityCubeElement> {
        &self.volatility_cubes
    }

    /// Returns mutable volatility cubes map.
    #[must_use]
    pub const fn volatility_cubes_mut(&mut self) -> &mut HashMap<MarketIndex, VolatilityCubeElement> {
        &mut self.volatility_cubes
    }

    /// Returns simulations.
    #[must_use]
    pub const fn simulations(&self) -> &HashMap<MarketIndex, SimulationElement> {
        &self.simulations
    }

    /// Returns mutable simulations map.
    #[must_use]
    pub const fn simulations_mut(&mut self) -> &mut HashMap<MarketIndex, SimulationElement> {
        &mut self.simulations
    }

    /// Gets one discount curve by index.
    #[must_use]
    pub fn discount_curve(&self, index: &MarketIndex) -> Option<&DiscountCurveElement> {
        self.discount_curves.get(index)
    }

    /// Gets one dividend curve by index.
    #[must_use]
    pub fn dividend_curve(&self, index: &MarketIndex) -> Option<&DividendCurveElement> {
        self.dividend_curves.get(index)
    }

    /// Gets one volatility surface by index.
    #[must_use]
    pub fn volatility_surface(&self, index: &MarketIndex) -> Option<&VolatilitySurfaceElement> {
        self.volatility_surfaces.get(index)
    }

    /// Gets one volatility cube by index.
    #[must_use]
    pub fn volatility_cube(&self, index: &MarketIndex) -> Option<&VolatilityCubeElement> {
        self.volatility_cubes.get(index)
    }
}

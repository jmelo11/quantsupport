use std::cell::{Ref, RefMut};

use crate::{
    ad::adreal::ADReal,
    core::{marketdatahandling::constructedelementstore::SharedElement, pillars::Pillars},
    indices::marketindex::MarketIndex,
    volatility::volatilitysurface::VolatilitySurface,
};
/// `ADVolatilitySurfaceElement`
pub trait ADVolatilitySurfaceElement:
    VolatilitySurface<ADReal> + Pillars<ADReal> + Send + Sync
{
}
/// `VolatilitySurfaceElement`
#[derive(Clone)]
pub struct VolatilityCubeElement {
    market_index: MarketIndex,
    cube: SharedElement<dyn ADVolatilitySurfaceElement>,
}

impl VolatilityCubeElement {
    /// Creates a new [`VolatilityCubeElement`] with the specified market index and surface.
    #[must_use]
    pub fn new(
        market_index: MarketIndex,
        cube: SharedElement<dyn ADVolatilitySurfaceElement>,
    ) -> Self {
        Self { market_index, cube }
    }

    /// Returns the market index associated with the volatility surface element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns a reference to the surface associated with the volatility surface element.
    #[must_use]
    pub fn cube(&self) -> Ref<'_, dyn ADVolatilitySurfaceElement> {
        self.cube.borrow()
    }

    /// Returns a mutable reference to the surface associated with the volatility surface element.
    pub fn cube_mut(&mut self) -> RefMut<'_, dyn ADVolatilitySurfaceElement> {
        self.cube.borrow_mut()
    }
}

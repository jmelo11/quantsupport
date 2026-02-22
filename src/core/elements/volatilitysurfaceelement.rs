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
pub struct VolatilitySurfaceElement {
    market_index: MarketIndex,
    surface: SharedElement<dyn ADVolatilitySurfaceElement>,
}

impl VolatilitySurfaceElement {
    /// Creates a new [`VolatilitySurfaceElement`] with the specified market index and surface.
    #[must_use]
    pub fn new(
        market_index: MarketIndex,
        surface: SharedElement<dyn ADVolatilitySurfaceElement>,
    ) -> Self {
        Self {
            market_index,
            surface,
        }
    }
    /// Returns the market index associated with the volatility surface element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns a reference to the surface associated with the volatility surface element.
    #[must_use]
    pub fn surface(&self) -> Ref<'_, dyn ADVolatilitySurfaceElement> {
        self.surface.borrow()
    }

    /// Returns a mutable reference to the surface associated with the volatility surface element.
    #[must_use]
    pub fn surface_mut(&mut self) -> RefMut<'_, dyn ADVolatilitySurfaceElement> {
        self.surface.borrow_mut()
    }
}

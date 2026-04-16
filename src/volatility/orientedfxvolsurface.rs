//! Oriented FX volatility surface adapter.
//!
//! Wraps a [`VolatilitySurfaceElement`] and transparently applies the standard
//! FX parity transform when the requested pair orientation differs from the
//! stored (canonical) one.
//!
//! ## Vol-surface inversion symmetry
//!
//! Under the standard Black-76 / Garman–Kohlhagen framework the implied
//! volatility satisfies:
//!
//! ```text
//! σ_{Y/X}(T, K) = σ_{X/Y}(T, 1/K)
//! ```
//!
//! The adapter applies `K → 1/K` when the pair orientation is inverted with
//! respect to storage. Callers (instruments, pricers) never see the inversion.

use std::cell::Ref;

use crate::{
    ad::dual::DualFwd,
    core::elements::volatilitysurfaceelement::{ADVolatilitySurfaceElement, VolatilitySurfaceElement},
    utils::errors::Result,
};

/// An oriented view of an FX volatility surface.
///
/// When `inverted` is false this behaves as a transparent pass-through.
/// When `inverted` is true, strikes are reciprocated before querying the
/// underlying surface.
pub struct OrientedFxVolSurface<'a> {
    element: &'a VolatilitySurfaceElement,
    inverted: bool,
}

impl<'a> OrientedFxVolSurface<'a> {
    /// Creates a new oriented view.
    ///
    /// * `element` — the stored volatility surface element.
    /// * `inverted` — `true` when the requested pair orientation is the
    ///   inverse of the canonical (stored) one.
    #[must_use]
    pub const fn new(element: &'a VolatilitySurfaceElement, inverted: bool) -> Self {
        Self { element, inverted }
    }

    /// Returns the underlying element (in case callers need raw access).
    #[must_use]
    pub const fn element(&self) -> &'a VolatilitySurfaceElement {
        self.element
    }

    /// Whether the view applies the parity inversion.
    #[must_use]
    pub const fn is_inverted(&self) -> bool {
        self.inverted
    }

    /// Borrow the inner surface.
    #[must_use]
    pub fn surface(&self) -> Ref<'_, dyn ADVolatilitySurfaceElement> {
        self.element.surface()
    }

    /// Returns the volatility for a given expiry date and strike,
    /// applying the parity transform when inverted.
    ///
    /// # Errors
    /// Propagates errors from the underlying surface.
    pub fn volatility_from_date(
        &self,
        expiry: crate::time::date::Date,
        strike: f64,
    ) -> Result<DualFwd> {
        let key = if self.inverted { 1.0 / strike } else { strike };
        self.element.surface().volatility_from_date(expiry, key)
    }
}

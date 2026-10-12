//! Market quote carried by a bond trade.

use crate::utils::errors::{QSError, Result};

/// Market observable used to anchor a bond valuation.
///
/// Clean and dirty prices are quoted per the bond's configured units. A price
/// anchor implies a Z-spread for a straight fixed-rate bond. `OAS` is reserved
/// for issuer-call bonds and is rejected by the deterministic engine.
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BondPricingAnchor {
    /// Clean price, excluding accrued interest.
    CleanPrice(f64),
    /// Dirty price, including accrued interest.
    DirtyPrice(f64),
    /// Continuously compounded spread over the discount curve.
    ZSpread(f64),
    /// Option-adjusted spread for an issuer-call pricing model.
    OAS(f64),
}

impl BondPricingAnchor {
    /// Returns the numeric quote carried by the anchor variant.
    const fn value(self) -> f64 {
        match self {
            Self::CleanPrice(value)
            | Self::DirtyPrice(value)
            | Self::ZSpread(value)
            | Self::OAS(value) => value,
        }
    }

    /// Checks finite spreads and strictly positive quoted prices.
    ///
    /// # Errors
    /// Returns an error for nonfinite values or nonpositive price quotes.
    pub fn validate(self) -> Result<()> {
        let value = self.value();
        if !value.is_finite() {
            return Err(QSError::InvalidValueErr(
                "Bond pricing anchor must be finite".into(),
            ));
        }
        if matches!(self, Self::CleanPrice(_) | Self::DirtyPrice(_)) && value <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Clean and dirty bond prices must be positive".into(),
            ));
        }
        Ok(())
    }
}

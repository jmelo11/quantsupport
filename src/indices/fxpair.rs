//! FX currency-pair type with canonical-form support.
//!
//! [`FxPair`] encodes a directed foreign-exchange pair (base / quote).
//! The canonical form uses a deterministic lexicographic ordering of the
//! ISO 4217 codes so that *EURUSD* and *USDEUR* map to the same canonical
//! key — enabling parity-agnostic storage while preserving the instrument's
//! natural orientation.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::currencies::currency::Currency;
use crate::utils::errors::{QSError, Result};

/// A directed FX currency pair.
///
/// `base` is the currency being bought (numerator in the FX rate);
/// `quote` is the currency being sold (denominator).
/// For example, `FxPair { base: EUR, quote: USD }` means "EUR/USD".
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FxPair {
    base: Currency,
    quote: Currency,
}

impl FxPair {
    /// Creates a new [`FxPair`].
    ///
    /// # Errors
    /// Returns an error if `base` and `quote` are the same currency.
    pub fn new(base: Currency, quote: Currency) -> Result<Self> {
        if base == quote {
            return Err(QSError::InvalidValueErr(format!(
                "FxPair: base and quote must differ, got {base}/{quote}"
            )));
        }
        Ok(Self { base, quote })
    }

    /// Returns the base currency.
    #[must_use]
    pub const fn base(&self) -> Currency {
        self.base
    }

    /// Returns the quote currency.
    #[must_use]
    pub const fn quote(&self) -> Currency {
        self.quote
    }

    /// Returns the pair with base and quote swapped.
    #[must_use]
    pub const fn inverted(&self) -> Self {
        Self {
            base: self.quote,
            quote: self.base,
        }
    }

    /// Returns the canonical form of this pair and whether the original
    /// orientation is inverted with respect to it.
    ///
    /// Canonical ordering: the currency whose ISO code is lexicographically
    /// smaller becomes the base. If the pair is already in canonical order the
    /// second element is `false`; otherwise it is `true` (meaning the
    /// original pair is the inverse of the canonical pair).
    #[must_use]
    pub fn canonical(&self) -> (Self, bool) {
        if self.base.as_str() <= self.quote.as_str() {
            (*self, false)
        } else {
            (self.inverted(), true)
        }
    }

    /// Returns the canonical form of this pair, discarding the inversion flag.
    ///
    /// Useful as a parity-agnostic `HashMap` key: both EURUSD and USDEUR
    /// yield the same canonical key.
    #[must_use]
    pub fn canonical_key(&self) -> Self {
        self.canonical().0
    }
}

impl fmt::Display for FxPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.base, self.quote)
    }
}

impl std::str::FromStr for FxPair {
    type Err = QSError;

    /// Parses a 6-character ISO pair such as `"EURUSD"`.
    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.len() != 6 {
            return Err(QSError::InvalidValueErr(format!(
                "FxPair: expected 6-character ISO pair, got \"{s}\""
            )));
        }
        let base: Currency = s[..3].parse()?;
        let quote: Currency = s[3..].parse()?;
        Self::new(base, quote)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_equal_currencies() {
        assert!(FxPair::new(Currency::EUR, Currency::EUR).is_err());
    }

    #[test]
    fn inverted_swaps_base_and_quote() {
        let pair = FxPair::new(Currency::EUR, Currency::USD).unwrap();
        let inv = pair.inverted();
        assert_eq!(inv.base(), Currency::USD);
        assert_eq!(inv.quote(), Currency::EUR);
    }

    #[test]
    fn canonical_is_stable() {
        let eurusd = FxPair::new(Currency::EUR, Currency::USD).unwrap();
        let usdeur = FxPair::new(Currency::USD, Currency::EUR).unwrap();

        let (c1, inv1) = eurusd.canonical();
        let (c2, inv2) = usdeur.canonical();

        assert_eq!(
            c1, c2,
            "canonical form must be the same for both orientations"
        );
        assert!(!inv1, "EURUSD should already be canonical (EUR < USD)");
        assert!(inv2, "USDEUR should be flagged as inverted");
    }

    #[test]
    fn canonical_key_matches_for_both_orientations() {
        let eurusd = FxPair::new(Currency::EUR, Currency::USD).unwrap();
        let usdeur = eurusd.inverted();
        assert_eq!(eurusd.canonical_key(), usdeur.canonical_key());
    }

    #[test]
    fn display_and_from_str_round_trip() {
        let pair = FxPair::new(Currency::EUR, Currency::USD).unwrap();
        let s = pair.to_string();
        assert_eq!(s, "EURUSD");
        let parsed: FxPair = s.parse().unwrap();
        assert_eq!(parsed, pair);
    }

    #[test]
    fn from_str_rejects_wrong_length() {
        assert!("EUR".parse::<FxPair>().is_err());
        assert!("EURUSDJPY".parse::<FxPair>().is_err());
    }
}

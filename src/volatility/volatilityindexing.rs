use std::{cmp::Ordering, hash::Hash};

use serde::{Deserialize, Serialize};

use crate::utils::errors::QSError;

/// Strike specification for option instruments and volatility quote keys.
///
/// The [`Strike::resolve`] helper interprets these variants as:
/// - [`Strike::Absolute`] — a fixed absolute rate or price.
/// - [`Strike::Atm`] — the supplied forward rate or price.
/// - [`Strike::Relative`] — an additive offset: `K_eff = F + offset`.
///
/// Consumers need not use this helper. In particular, the Black and Hull-White
/// caplet/cap pricers interpret `Relative(pct)` multiplicatively as
/// `F * (1 + pct)`. Volatility lookup keys also need not equal the payoff strike.
/// Consult the consuming pricer's convention; absolute strikes avoid this
/// relative-strike ambiguity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Strike {
    /// A fixed absolute strike rate or price.
    Absolute(f64),
    /// At-the-money: the strike equals the forward rate at pricing time.
    Atm,
    /// A relative-strike payload: additive for [`Strike::resolve`], but
    /// interpreted multiplicatively by some rate-option pricers.
    Relative(f64),
}

impl Strike {
    /// Resolves a strike using the supplied forward, adding a `Relative`
    /// payload in the same rate/price units.
    ///
    /// This helper does not impose its convention on pricers that resolve
    /// strikes independently.
    #[must_use]
    pub fn resolve(self, forward_rate: f64) -> f64 {
        match self {
            Self::Absolute(k) => k,
            Self::Atm => forward_rate,
            Self::Relative(spread) => forward_rate + spread,
        }
    }
}

impl std::str::FromStr for Strike {
    type Err = QSError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "absolute" => Ok(Self::Absolute(0.0)),
            "atm" => Ok(Self::Atm),
            "relative" => Ok(Self::Relative(0.0)),
            _ => Err(QSError::InvalidValueErr(format!(
                "Unknown strike type: {s}"
            ))),
        }
    }
}

/// Represents if the volatility is quoted as black (log-normal) or normal volatility.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolatilityType {
    /// Annualized decimal lognormal volatility, e.g. `0.20` for 20%.
    Black,
    /// Absolute underlying rate/price units per square-root year.
    Normal,
}

impl std::str::FromStr for VolatilityType {
    type Err = QSError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "black" => Ok(Self::Black),
            "normal" => Ok(Self::Normal),
            _ => Err(QSError::InvalidValueErr(format!(
                "Unknown volatility type: {s}"
            ))),
        }
    }
}

/// Smile axis used in volatility surfaces/cubes.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmileType {
    /// Strike axis point.
    Strike,
    /// Delta axis point.
    Delta,
    /// Log-moneyness axis point.
    LogMoneyness,
}

/// Key wrapper around `f64` for map/set usage with total ordering.
#[derive(Clone, Debug, PartialEq)]
pub struct F64Key(pub f64);

impl F64Key {
    /// Creates a new floating key.
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self(value)
    }

    /// Returns the wrapped value.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.0
    }

    /// Returns the bit representation used for hashing.
    #[must_use]
    pub const fn to_key(&self) -> u64 {
        self.0.to_bits()
    }
}

impl Eq for F64Key {}

impl PartialOrd for F64Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for F64Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Hash for F64Key {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_key().hash(state);
    }
}

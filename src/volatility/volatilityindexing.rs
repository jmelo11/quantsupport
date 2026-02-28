use std::{cmp::Ordering, hash::Hash};

use serde::{Deserialize, Serialize};

use crate::utils::errors::AtlasError;

/// Strike specification for a caplet/floorlet.
///
/// - [`Strike::Absolute`] — a fixed absolute strike rate.
/// - [`Strike::Atm`] — at-the-money: the pricer sets the strike equal to the
///   prevailing forward rate at pricing time.
/// - [`Strike::Relative`] — a spread (positive or negative) added to the
///   forward rate at pricing time: `K_eff = F + spread`.
///
/// For [`Strike::Atm`] and [`Strike::Relative`], the effective absolute strike
/// is computed by the pricer from the forward rate before querying the
/// volatility surface.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Strike {
    /// A fixed absolute strike rate.
    Absolute(f64),
    /// At-the-money: the strike equals the forward rate at pricing time.
    Atm,
    /// A spread over the forward rate: `K_eff = F + spread`.
    Relative(f64),
}

/// Represents if the volatility is quoted as black (log-normal) or normal volatility.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum VolatilityType {
    /// Black (log-normal) volatility.
    Black,
    /// Normal volatility.
    Normal,
}

impl std::str::FromStr for VolatilityType {
    type Err = AtlasError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "Black" => Ok(Self::Black),
            "Normal" => Ok(Self::Normal),
            _ => Err(AtlasError::InvalidValueErr(format!(
                "Unknown volatility type: {s}"
            ))),
        }
    }
}

/// Smile axis used in volatility surfaces/cubes.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
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

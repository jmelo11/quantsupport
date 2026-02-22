use std::{cmp::Ordering, hash::Hash};

use serde::{Deserialize, Serialize};

/// # `VolatilityType`
///
/// Represents if the volatility is quoted as black (log-normal) or normal volatility.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum VolatilityType {
    /// Black (log-normal) volatility.
    Black,
    /// Normal volatility.
    Normal,
}

/// # `SmileAxis`
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
    pub fn new(value: f64) -> Self {
        Self(value)
    }

    /// Returns the wrapped value.
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Returns the bit representation used for hashing.
    pub fn to_key(&self) -> u64 {
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

// /// # `SurfaceKey`
// /// Surface node key made of market index, expiry date, and smile axis.
// #[derive(Clone, Debug, Hash, PartialEq, Eq)]
// pub struct SurfaceKey {
//     date: Date,
//     axis: SmileAxis,
// }

// impl SurfaceKey {
//     /// Creates a new surface node key.
//     #[must_use]
//     pub const fn new(date: Date, axis: SmileAxis) -> Self {
//         Self { date, axis }
//     }

//     /// Returns the expiry date.
//     #[must_use]
//     pub const fn date(&self) -> Date {
//         self.date
//     }

//     /// Returns the smile axis.
//     #[must_use]
//     pub const fn axis(&self) -> SmileAxis {
//         self.axis
//     }
// }

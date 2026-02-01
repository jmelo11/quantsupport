use std::collections::BTreeMap;

use crate::{
    indices::marketindex::MarketIndex,
    time::{date::Date, period::Period},
};

/// # `VolatilityType`
///
/// Represents if the volatility is quoted as black (log-normal) or normal volatility.
pub enum VolatilityType {
    /// Black (log-normal) volatility.
    Black,
    /// Normal volatility.
    Normal,
}

/// # `VolatilitySurface`
///
/// Volatility surface keyed by maturity date and strike.
///
/// Values are stored in a nested map so the surface can be populated from
/// serialized market data in a deterministic order.
#[derive(Clone, Debug, Default)]
pub struct VolatilitySurface {
    market_index: MarketIndex,
    points: BTreeMap<Date, BTreeMap<f64, f64>>,
}

impl VolatilitySurface {
    /// Creates an empty volatility surface for the given instrument.
    #[must_use]
    pub const fn new(market_index: MarketIndex) -> Self {
        Self {
            market_index,
            points: BTreeMap::new(),
        }
    }

    /// Returns the instrument identifier for this surface.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }
}

/// Volatility cube keyed by maturity, tenor, and strike.
///
/// The tenor dimension uses the [`Period`] type so common tenors such as `1Y`
/// or `6M` can be stored consistently.
#[derive(Clone, Debug, Default)]
pub struct VolatilityCube {
    market_index: MarketIndex,
    points: BTreeMap<Date, BTreeMap<Period, BTreeMap<f64, f64>>>,
}

impl VolatilityCube {
    /// Creates an empty volatility cube for the given instrument.
    #[must_use]
    pub const fn new(market_index: MarketIndex) -> Self {
        Self {
            market_index,
            points: BTreeMap::new(),
        }
    }

    /// Returns the instrument identifier for this cube.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }
}

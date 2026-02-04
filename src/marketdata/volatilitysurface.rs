use crate::{indices::marketindex::MarketIndex, marketdata::surface::Surface, time::date::Date};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

/// # `VolatilitySurface`
///
/// Represents if the volatility surface.
///
/// ## Generics
/// - `A`: The type of the axis (e.g., Strike, Moneyness, Delta). It should implement `Ord` trait.
pub struct VolatilitySurface<A: Ord> {
    market_index: MarketIndex,
    points: BTreeMap<Date, BTreeMap<A, f64>>,
}

impl<A: Ord> VolatilitySurface<A> {
    /// Creates a new `VolatilitySurface`.
    #[must_use]
    pub fn new(market_index: MarketIndex, points: BTreeMap<Date, BTreeMap<A, f64>>) -> Self {
        Self {
            market_index,
            points,
        }
    }

    /// Returns the market index associated with the volatility surface.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }
}

impl<A: Ord> Surface<Date, A, f64> for VolatilitySurface<A> {
    fn points(&self) -> &BTreeMap<Date, BTreeMap<A, f64>> {
        &self.points
    }
}

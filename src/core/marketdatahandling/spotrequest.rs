use crate::{indices::marketindex::MarketIndex, time::date::Date};

/// Request for a spot observation at a given date.
#[derive(Clone)]
pub struct SpotRequest {
    market_index: MarketIndex,
    date: Date,
}

impl SpotRequest {
    /// Creates a new spot request for the given market index and date.
    #[must_use]
    pub const fn new(market_index: MarketIndex, date: Date) -> Self {
        Self { market_index, date }
    }

    /// Returns the market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the observation date.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }
}

use crate::{indices::marketindex::MarketIndex, time::date::Date};

/// Struct representing a request for a fixing, which includes the market index and date for which the fixing is requested.
pub struct FixingRequest {
    market_index: MarketIndex,
    date: Date,
}

impl FixingRequest {
    /// Creates a new [`FixingRequest`] with the specified market index and date.
    #[must_use]
    pub const fn new(market_index: MarketIndex, date: Date) -> Self {
        Self { market_index, date }
    }

    /// Returns the market index associated with the fixing request.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the date associated with the fixing request.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }
}

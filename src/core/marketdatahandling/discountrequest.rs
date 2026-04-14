use crate::{indices::marketindex::MarketIndex, time::date::Date};

/// Request for a discount factor for a given market index and date.
#[derive(Clone)]
pub struct DiscountRequest {
    market_index: MarketIndex,
    date: Date,
}

impl DiscountRequest {
    /// Creates a new [`DiscountRequest`] with the specified market index and date.
    #[must_use]
    pub const fn new(market_index: MarketIndex, date: Date) -> Self {
        Self { market_index, date }
    }

    /// Returns the market index associated with this discount request.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the date for which the discount factor is requested.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }
}

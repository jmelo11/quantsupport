use crate::{indices::marketindex::MarketIndex, time::date::Date};

pub struct SpotRequest {
    market_index: MarketIndex,
    date: Date,
}

impl SpotRequest {
    pub const fn new(market_index: MarketIndex, date: Date) -> Self {
        Self { market_index, date }
    }

    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    pub const fn date(&self) -> Date {
        self.date
    }
}

use crate::{indices::marketindex::MarketIndex, time::date::Date};

#[derive(Clone)]
pub struct SpotRequest {
    market_index: MarketIndex,
    date: Date,
}

impl SpotRequest {
    #[must_use] 
    pub const fn new(market_index: MarketIndex, date: Date) -> Self {
        Self { market_index, date }
    }

    #[must_use] 
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    #[must_use] 
    pub const fn date(&self) -> Date {
        self.date
    }
}

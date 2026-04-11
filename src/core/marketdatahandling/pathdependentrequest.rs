use crate::{indices::marketindex::MarketIndex, time::date::Date};

#[derive(Clone)]
pub struct PathDependentRequest {
    observation_dates: Vec<Date>,
    market_index: MarketIndex,
}

impl PathDependentRequest {
    #[must_use] 
    pub const fn new(observation_dates: Vec<Date>, market_index: MarketIndex) -> Self {
        Self {
            observation_dates,
            market_index,
        }
    }

    #[must_use] 
    pub fn observation_dates(&self) -> &[Date] {
        &self.observation_dates
    }

    #[must_use] 
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }
}

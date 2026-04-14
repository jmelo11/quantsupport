use crate::{indices::marketindex::MarketIndex, time::date::Date};

/// Request for path-dependent observations over a series of dates.
#[derive(Clone)]
pub struct PathDependentRequest {
    observation_dates: Vec<Date>,
    market_index: MarketIndex,
}

impl PathDependentRequest {
    /// Creates a new path-dependent request for the given observation dates and market index.
    #[must_use]
    pub const fn new(observation_dates: Vec<Date>, market_index: MarketIndex) -> Self {
        Self {
            observation_dates,
            market_index,
        }
    }

    /// Returns the observation dates.
    #[must_use]
    pub fn observation_dates(&self) -> &[Date] {
        &self.observation_dates
    }

    /// Returns the market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }
}

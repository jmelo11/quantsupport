use crate::{indices::marketindex::MarketIndex, time::date::Date};

/// Request for a forward rate between two dates for a given market index.
pub struct ForwardRateRequest {
    market_index: MarketIndex,
    fixing_date: Date,
    start_date: Option<Date>,
    end_date: Option<Date>,
}

impl ForwardRateRequest {
    /// Creates a new [`ForwardRateRequest`] with the specified market index and fixing date.
    #[must_use]
    pub const fn new(market_index: MarketIndex, fixing_date: Date) -> Self {
        Self {
            market_index,
            fixing_date,
            start_date: None,
            end_date: None,
        }
    }

    /// Sets the start date for the forward rate calculation.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the end date for the forward rate calculation.
    #[must_use]
    pub const fn with_end_date(mut self, end_date: Date) -> Self {
        self.end_date = Some(end_date);
        self
    }

    /// Returns the market index associated with this forward rate request.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the fixing date for which the forward rate is requested.
    #[must_use]
    pub const fn fixing_date(&self) -> Date {
        self.fixing_date
    }

    /// Returns the start date for the forward rate calculation, if set.
    #[must_use]
    pub fn start_date(&self) -> Option<Date> {
        self.start_date
    }

    /// Returns the end date for the forward rate calculation, if set.
    #[must_use]
    pub fn end_date(&self) -> Option<Date> {
        self.end_date
    }
}

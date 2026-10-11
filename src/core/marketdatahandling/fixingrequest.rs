use crate::{indices::marketindex::MarketIndex, time::date::Date};

/// Struct representing a request for a fixing, which includes the market index and date for which the fixing is requested.
pub struct FixingRequest {
    market_index: MarketIndex,
    date: Date,
    missing_fixing_allowed: bool,
}

impl FixingRequest {
    /// Creates a new [`FixingRequest`] with the specified market index and date.
    #[must_use]
    pub const fn new(market_index: MarketIndex, date: Date) -> Self {
        Self {
            market_index,
            date,
            missing_fixing_allowed: false,
        }
    }

    /// Sets whether a missing fixing may be omitted from the market-data response.
    /// Defaults to false: an unavailable fixing returns an error. When true, the
    /// caller must handle the absent observation, for example by forecasting
    /// today's unpublished overnight rate.
    #[must_use]
    pub const fn with_missing_fixing_allowed(mut self, allowed: bool) -> Self {
        self.missing_fixing_allowed = allowed;
        self
    }

    /// Whether an unavailable observation may be omitted from the response.
    #[must_use]
    pub const fn missing_fixing_allowed(&self) -> bool {
        self.missing_fixing_allowed
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fixings_require_explicit_permission() {
        let date = Date::new(2026, 10, 9);
        let request = FixingRequest::new(MarketIndex::SOFR, date);
        assert!(!request.missing_fixing_allowed());
        let request = request.with_missing_fixing_allowed(true);
        assert!(request.missing_fixing_allowed());
        assert_eq!(request.market_index(), &MarketIndex::SOFR);
        assert_eq!(request.date(), date);
        assert!(!request
            .with_missing_fixing_allowed(false)
            .missing_fixing_allowed());
    }
}

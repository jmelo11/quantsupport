use crate::{quotes::quote::Quote, time::date::Date};

/// Selects market quotes by identifier.
pub trait QuoteSelector {
    /// Returns the quote with the given identifier.
    fn select(&self, identifier: &str) -> Option<Quote>;
    /// Returns the reference (valuation) date used for building instruments.
    fn reference_date(&self) -> Date;
}

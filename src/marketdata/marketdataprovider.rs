use crate::prelude::Date;

/// # Quote
pub struct Quote {
    identifier: &'static str,
    mid: f64,
    bid: f64,
    ask: f64,
}

/// # ExpandedQuote
pub struct ExpandedQuote {}

/// # MarketDataProvider
pub struct MarketDataProvider {
    reference_date: Date,
    quotes: Vec<Quote>,
}

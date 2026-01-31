use serde::{Deserialize, Serialize};

use crate::prelude::Date;

/// # Quote
#[derive(Serialize, Deserialize)]
pub struct Quote {
    identifier: String,
    mid: f64,
    bid: f64,
    ask: f64,
}

/// # ExpandedQuote
pub struct ExpandedQuote {}

#[derive(Serialize, Deserialize)]
/// # MarketDataProvider
pub struct MarketDataProvider {
    reference_date: Date,
    quotes: Vec<Quote>,
}

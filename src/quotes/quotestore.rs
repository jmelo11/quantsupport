use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    indices::marketindex::MarketIndex,
    quotes::quote::Quote,
    rates::bootstrapping::curvespec::QuoteSelector,
    time::{date::Date, period::Period},
};

/// Provider of market data loaded from quotes.
#[derive(Debug, Serialize, Deserialize)]
pub struct QuoteStore {
    reference_date: Date,
    quotes: HashMap<MarketIndex, HashMap<String, Quote>>,
}

impl QuoteStore {
    /// Creates an empty market data provider.
    #[must_use]
    pub fn new(reference_date: Date) -> Self {
        Self {
            reference_date,
            quotes: HashMap::new(),
        }
    }
    /// Returns the reference date for the provider.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Adds a market quote to the provider.
    ///
    /// Quotes with a `secondary_market_index` (basis swaps, cross-currency
    /// swaps) are indexed under that secondary index, because they calibrate
    /// the secondary curve.  All other quotes are indexed under their primary
    /// market index.
    pub fn add_quote(&mut self, quote: Quote) {
        let index = quote
            .details()
            .secondary_market_index()
            .or_else(|| quote.details().market_index())
            .cloned()
            .unwrap_or_default();
        self.quotes
            .entry(index)
            .or_default()
            .entry(quote.details().identifier())
            .insert_entry(quote);
    }

    /// Returns the quotes for a given market index.
    #[must_use]
    pub fn quotes_for_index(&self, market_index: &MarketIndex) -> Option<&HashMap<String, Quote>> {
        self.quotes.get(market_index)
    }
}

impl QuoteSelector for QuoteStore {
    fn select(&self, market_index: &MarketIndex, tenor: &Period) -> Option<Quote> {
        let bucket = self.quotes.get(market_index)?;
        bucket
            .values()
            .find(|q| q.details().tenor().as_ref() == Some(tenor))
            .cloned()
    }

    fn reference_date(&self) -> Date {
        self.reference_date
    }
}

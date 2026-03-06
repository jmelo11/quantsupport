use std::collections::HashMap;

use crate::{indices::marketindex::MarketIndex, quotes::quote::Quote, time::date::Date};

/// Provider of market data loaded from quotes.
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
    pub fn add_quote(&mut self, quote: Quote) {
        let index = quote.details().market_index().cloned().unwrap_or_default();
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

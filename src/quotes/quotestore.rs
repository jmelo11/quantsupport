use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    quotes::quote::Quote, rates::bootstrapping::curvespec::QuoteSelector, time::date::Date,
};

/// Provider of market data loaded from quotes.
#[derive(Debug, Serialize, Deserialize)]
pub struct QuoteStore {
    reference_date: Date,
    quotes: HashMap<String, Quote>,
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

    /// Adds a market quote to the provider, indexed by its identifier.
    pub fn add_quote(&mut self, quote: Quote) {
        let id = quote.details().identifier();
        self.quotes.insert(id, quote);
    }

    /// Returns a quote by identifier.
    #[must_use]
    pub fn quote(&self, identifier: &str) -> Option<&Quote> {
        self.quotes.get(identifier)
    }

    /// Returns all stored quotes.
    #[must_use]
    pub const fn quotes(&self) -> &HashMap<String, Quote> {
        &self.quotes
    }
}

impl QuoteSelector for QuoteStore {
    fn select(&self, identifier: &str) -> Option<Quote> {
        self.quotes.get(identifier).cloned()
    }

    fn reference_date(&self) -> Date {
        self.reference_date
    }
}

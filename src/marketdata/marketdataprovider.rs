use crate::{
    indices::marketindex::MarketIndex,
    marketdata::quote::{Level, Quote},
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

/// # `MarketDataProvider`
///
/// Provider of market data loaded from quotes.
pub struct MarketDataProvider {
    reference_date: Date,
    quotes: Vec<Quote>,
    // vol_surfaces: HashMap<MarketIndex, VolatilitySurface>, // this should have a volatility type key
    // vol_cubes: HashMap<MarketIndex, VolatilityCube>,
}

impl MarketDataProvider {
    /// Creates an empty market data provider.
    #[must_use]
    pub fn new(reference_date: Date) -> Self {
        Self {
            reference_date,
            quotes: Vec::new(),
            // vol_surfaces: HashMap::new(),
            // vol_cubes: HashMap::new(),
        }
    }

    /// Returns the reference date for the provider.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Returns the raw quotes.
    #[must_use]
    pub fn quotes(&self) -> &[Quote] {
        &self.quotes
    }

    /// Adds a market quote to the provider.
    pub fn add_quote(&mut self, quote: Quote) {
        self.quotes.push(quote);
    }

    /// Returns the quote for a given market index.
    ///
    /// ## Errors
    /// Returns an error if no quote matches the given index.
    pub fn quote(&self, market_index: &MarketIndex) -> Result<&Quote> {
        self.quotes
            .iter()
            .find(|quote| quote.quote_details().market_index() == market_index)
            .ok_or(AtlasError::NotFoundErr(format!(
                "Market quote not found for index {market_index}."
            )))
    }

    /// Returns the quote value for a given market index and level.
    ///
    /// ## Errors
    /// Returns an error if the quote or level is unavailable.
    pub fn quote_value(&self, market_index: &MarketIndex, level: &Level) -> Result<f64> {
        let quote = self.quote(market_index)?;
        quote.quote_levels().value(level)
    }

    // Returns a volatility surface by instrument identifier.
    // #[must_use]
    // pub fn volatility_surface(&self, instrument: &MarketIndex) -> Option<&VolatilitySurface> {
    //     self.vol_surfaces.get(instrument)
    // }

    // /// Returns a volatility cube by instrument identifier.
    // #[must_use]
    // pub fn volatility_cube(&self, instrument: &MarketIndex) -> Option<&VolatilityCube> {
    //     self.vol_cubes.get(instrument)
    // }
}

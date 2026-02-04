use crate::{marketdata::quote::Quote, time::date::Date};

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

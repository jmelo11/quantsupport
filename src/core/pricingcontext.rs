use crate::{marketdata::{fixingprovider::FixingProvider, marketdataprovider::MarketDataProvider}, time::date::Date};

/// # `PricingContext`
pub struct PricingContext {
    market_data_provider: MarketDataProvider,
    fixings_provider: FixingProvider,
    model_configuration: usize, // Placeholder for model configuration identifier, WIP
}

impl PricingContext {
    /// Creates a new pricing context.
    #[must_use]
    pub const fn new(
        market_data_provider: MarketDataProvider,
        fixings_provider: FixingProvider,
        model_configuration: usize,
    ) -> Self {
        Self {
            market_data_provider,
            fixings_provider,
            model_configuration,
        }
    }

    /// Returns the market data provider.
    #[must_use]
    pub const fn market_data_provider(&self) -> &MarketDataProvider {
        &self.market_data_provider
    }

    /// Returns the fixings provider.
    #[must_use]
    pub const fn fixings_provider(&self) -> &FixingProvider {
        &self.fixings_provider
    }

    /// Returns the model configuration identifier.
    #[must_use]
    pub const fn model_configuration(&self) -> usize {
        self.model_configuration
    }

    /// Returns the current reference date.
    #[must_use]
    pub const fn evaluation_date(&self) -> Date {
        self.market_data_provider.reference_date()
    }
}

use crate::{
    core::{assetpresets::AssetPresets, assets::Assets},
    marketdata::{
        fixingprovider::FixingProvider, marketdataprovider::MarketDataProvider, quote::Level,
    },
    time::date::Date,
    utils::errors::Result,
};

/// Placeholder for model configurations
#[derive(Default, Copy, Clone)]
pub struct Model;

/// # `ContextManager`
pub struct ContextManager {
    market_data_provider: MarketDataProvider,
    fixings_provider: FixingProvider,
    quote_level: Level, // Placeholder to select the type of quote we want to use
    asset_presets: AssetPresets, // or AssetPresets?
    assets: Assets,
}

impl ContextManager {
    /// Creates a new pricing data context.
    #[must_use]
    pub fn new(market_data_provider: MarketDataProvider, fixings_provider: FixingProvider) -> Self {
        Self {
            market_data_provider,
            fixings_provider,
            quote_level: Level::Mid,
            asset_presets: AssetPresets::default(),
            assets: Assets::default(),
        }
    }

    /// Sets the quote level used for market value extraction.
    #[must_use]
    pub fn with_quote_level(mut self, quote_level: Level) -> Self {
        self.quote_level = quote_level;
        self
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
    pub const fn asset_presets(&self) -> &AssetPresets {
        &self.asset_presets
    }

    /// Returns the current reference date.
    #[must_use]
    pub const fn evaluation_date(&self) -> Date {
        self.market_data_provider.reference_date()
    }

    /// Generates the assets associated to the given models
    #[must_use]
    pub fn generate_assets(&mut self) -> Result<()> {
        Ok(())
    }
}

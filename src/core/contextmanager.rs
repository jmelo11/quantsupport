use crate::{
    core::{assetpresets::AssetPresets, assets::Assets},
    currencies::currency::Currency,
    marketdata::{
        fixingprovider::FixingProvider, marketdataprovider::MarketDataProvider, quote::Level,
    },
    time::date::Date,
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
    base_currency: Currency,
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
            base_currency: Currency::USD,
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

    /// Returns a mutable reference to the asset presets.
    pub fn asset_presets_mut(&mut self) -> &mut AssetPresets {
        &mut self.asset_presets
    }

    /// Sets the asset presets.
    #[must_use]
    pub fn with_asset_presets(mut self, asset_presets: AssetPresets) -> Self {
        self.asset_presets = asset_presets;
        self
    }

    /// Returns the quote level preference.
    #[must_use]
    pub const fn quote_level(&self) -> Level {
        self.quote_level
    }

    /// Returns the assets registry.
    #[must_use]
    pub const fn assets(&self) -> &Assets {
        &self.assets
    }

    /// Returns the base currency for reporting.
    #[must_use]
    pub const fn base_currency(&self) -> Currency {
        self.base_currency
    }

    /// Sets the base currency.
    #[must_use]
    pub fn with_base_currency(mut self, base_currency: Currency) -> Self {
        self.base_currency = base_currency;
        self
    }

    /// Returns the current reference date.
    #[must_use]
    pub const fn evaluation_date(&self) -> Date {
        self.market_data_provider.reference_date()
    }
}

use crate::marketdata::{fixingprovider::FixingProvider, marketdataprovider::MarketDataProvider};

/// # `PricingContext`
pub struct PricingContext {
    market_data_provider: MarketDataProvider,
    fixings_provider: FixingProvider,
    model_configuration: usize,
}

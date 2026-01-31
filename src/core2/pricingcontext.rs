use serde::{Deserialize, Serialize};

use crate::marketdata::marketdataprovider::MarketDataProvider;

/// # `PricingContext`
#[derive(Serialize, Deserialize)]
pub struct PricingContext {
    market_data_provider: MarketDataProvider,
    model_configuration: usize,
}

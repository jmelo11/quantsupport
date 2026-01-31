use serde::{Deserialize, Serialize};

/// # `PricingContext`
#[derive(Serialize, Deserialize)]
pub struct PricingContext {
    market_data: usize,
    model_configuration: usize,
}

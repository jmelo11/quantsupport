use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Hash)]
/// # `MarketIndexType`
/// Types of values that an index can store.
pub enum QuoteType {
    /// Price or level index.
    Price,
    /// Volatility index.
    Volatility,
    /// Rate index.
    Rate,
    /// Basis point index.
    BasisPoints,
    /// Convexity index.
    Convexity,
}

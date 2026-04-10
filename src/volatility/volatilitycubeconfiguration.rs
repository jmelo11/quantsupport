use serde::{Deserialize, Serialize};

use crate::{
    indices::marketindex::MarketIndex,
    volatility::volatilityindexing::{SmileType, VolatilityType},
};

/// JSON-serializable specification for building a volatility cube
/// (expiry x tenor x strike) from market quotes.  Cubes are typically
/// used for swaption volatilities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VolatilityCubeConfiguration {
    market_index: MarketIndex,
    #[serde(default = "default_volatility_type")]
    volatility_type: VolatilityType,
    #[serde(default = "default_smile_type")]
    smile_type: SmileType,
    quotes: Vec<String>,
}

fn default_volatility_type() -> VolatilityType {
    VolatilityType::Black
}
fn default_smile_type() -> SmileType {
    SmileType::Strike
}

impl VolatilityCubeConfiguration {
    /// Creates a new cube configuration.
    #[must_use]
    pub fn new(
        market_index: MarketIndex,
        volatility_type: VolatilityType,
        smile_type: SmileType,
        quotes: Vec<String>,
    ) -> Self {
        Self {
            market_index,
            volatility_type,
            smile_type,
            quotes,
        }
    }

    /// Returns the market index this cube is associated with.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the volatility type (Black or Normal).
    #[must_use]
    pub const fn volatility_type(&self) -> &VolatilityType {
        &self.volatility_type
    }

    /// Returns the smile axis convention.
    #[must_use]
    pub const fn smile_type(&self) -> SmileType {
        self.smile_type
    }

    /// Returns the list of quote identifiers that populate this cube.
    #[must_use]
    pub fn quotes(&self) -> &[String] {
        &self.quotes
    }
}

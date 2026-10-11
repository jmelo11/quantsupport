use serde::{Deserialize, Serialize};

use crate::{
    indices::marketindex::MarketIndex,
    math::interpolation::extrapolation::ExtrapolationPolicy,
    volatility::volatilityindexing::{SmileType, VolatilityType},
};

/// JSON-serializable specification for building a 2-D volatility surface
/// (expiry × strike) from market quotes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolatilitySurfaceConfiguration {
    market_index: MarketIndex,
    #[serde(default = "default_volatility_type")]
    volatility_type: VolatilityType,
    #[serde(default = "default_smile_type")]
    smile_type: SmileType,
    quotes: Vec<String>,
    #[serde(default)]
    extrapolation_policy: ExtrapolationPolicy,
}

const fn default_volatility_type() -> VolatilityType {
    VolatilityType::Black
}
const fn default_smile_type() -> SmileType {
    SmileType::Strike
}

impl VolatilitySurfaceConfiguration {
    /// Creates a new surface configuration.
    #[must_use]
    pub const fn new(
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
            extrapolation_policy: ExtrapolationPolicy::Reject,
        }
    }

    /// Sets how queries outside the configured grid are handled.
    #[must_use]
    pub const fn with_extrapolation_policy(mut self, policy: ExtrapolationPolicy) -> Self {
        self.extrapolation_policy = policy;
        self
    }

    /// Returns the extrapolation policy; defaults to `Reject`.
    #[must_use]
    pub const fn extrapolation_policy(&self) -> ExtrapolationPolicy {
        self.extrapolation_policy
    }

    /// Returns the market index this surface is associated with.
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

    /// Returns the list of quote identifiers that populate this surface.
    #[must_use]
    pub fn quotes(&self) -> &[String] {
        &self.quotes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_json_defaults_to_reject_and_round_trips_flat() {
        let input = serde_json::json!({"market_index": "SOFR", "quotes": []});
        let configuration: VolatilitySurfaceConfiguration =
            serde_json::from_value(input).expect("legacy configuration");
        assert_eq!(
            configuration.extrapolation_policy(),
            ExtrapolationPolicy::Reject
        );
        let input = serde_json::json!({"market_index": "SOFR", "quotes": [], "extrapolation_policy": "Flat"});
        let configuration: VolatilitySurfaceConfiguration =
            serde_json::from_value(input).expect("flat configuration");
        assert_eq!(
            configuration.extrapolation_policy(),
            ExtrapolationPolicy::Flat
        );
        assert_eq!(
            serde_json::to_value(configuration).expect("configuration serialization")
                ["extrapolation_policy"],
            "Flat"
        );
    }
}

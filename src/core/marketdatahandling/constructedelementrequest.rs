use crate::indices::marketindex::MarketIndex;

/// # `ConstructedElementRequest`
/// Request for a specific derived market-data element.
pub enum ConstructedElementRequest {
    /// Request for discount curve of a market index.
    DiscountCurve {
        /// Requested market index.
        market_index: MarketIndex,
    },
    /// Request for dividend curve of a market index.
    DividendCurve {
        /// Requested market index.
        market_index: MarketIndex,
    },
    /// Request for volatility surface of a market index.
    VolatilitySurface {
        /// Requested market index.
        market_index: MarketIndex,
    },
    /// Request for volatility cube of a market index.
    VolatilityCube {
        /// Requested market index.
        market_index: MarketIndex,
    },
    /// Request for simulation data of a market index.
    Simulation {
        /// Requested market index.
        market_index: MarketIndex,
    },
}

use crate::indices::marketindex::MarketIndex;

/// `SimulationElement`
///
/// Struct representing a simulation element, which includes the associated market
/// index and the simulation draws.
#[derive(Clone)]
pub struct SimulationElement {
    market_index: MarketIndex,
    draws: Vec<f64>,
}

impl SimulationElement {
    /// Creates a new [`SimulationElement`] with the specified market index and simulation draws.
    #[must_use]
    pub const fn new(market_index: MarketIndex, draws: Vec<f64>) -> Self {
        Self {
            market_index,
            draws,
        }
    }

    /// Returns the market index associated with the simulation element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }
    /// Returns a reference to the simulation draws associated with the simulation element.
    #[must_use]
    pub fn draws(&self) -> &[f64] {
        &self.draws
    }
}

use crate::{
    ad::adreal::ADReal,
    core::{marketdatahandling::constructedelementstore::SharedElement, pillars::Pillars},
    indices::marketindex::MarketIndex,
    simulations::simulation::MonteCarloSimulation,
};

/// Simulation element with Monte Carlo draws.
///
/// The [`ADMonteCarloSimulationElement`] trait represents a simulation element that includes both the Monte Carlo
/// simulation draws and the associated market index. This trait is used to define the structure of the simulation
/// elements that will be stored in the `ConstructedElementStore` and used for pricing and risk calculations.
pub trait ADMonteCarloSimulationElement:
    MonteCarloSimulation<ADReal> + Pillars<ADReal> + Send + Sync
{
}

/// Struct representing a simulation element, which includes the associated market
/// index and the simulation draws.
#[derive(Clone)]
pub struct MonteCarloSimulationElement {
    market_index: MarketIndex,
    simulation: SharedElement<dyn ADMonteCarloSimulationElement>,
}

impl MonteCarloSimulationElement {
    /// Creates a new [`MonteCarloSimulationElement`] with the specified market index and simulation draws.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        simulation: SharedElement<dyn ADMonteCarloSimulationElement>,
    ) -> Self {
        Self {
            market_index,
            simulation,
        }
    }

    /// Returns the market index associated with the simulation element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the simulation draws associated with the simulation element.
    #[must_use]
    pub const fn simulation(&self) -> &SharedElement<dyn ADMonteCarloSimulationElement> {
        &self.simulation
    }
}

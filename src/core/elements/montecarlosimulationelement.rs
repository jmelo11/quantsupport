use crate::{
    ad::dual::DualFwd,
    core::{marketdatahandling::constructedelementstore::SharedElement, pillars::Pillars},
    indices::marketindex::MarketIndex,
    models::hullwhite::hullwhitecalibrationquality::HullWhiteCalibrationQuality,
    simulations::simulation::MonteCarloSimulation,
};

/// Simulation element with Monte Carlo draws.
///
/// The [`ADMonteCarloSimulationElement`] trait represents a simulation element that includes both the Monte Carlo
/// simulation draws and the associated market index. This trait is used to define the structure of the simulation
/// elements that will be stored in the `ConstructedElementStore` and used for pricing and risk calculations.
pub trait ADMonteCarloSimulationElement:
    MonteCarloSimulation<DualFwd> + Pillars<DualFwd> + Send + Sync
{
}

/// Struct representing a simulation element, which includes the associated market
/// index and the simulation draws.
#[derive(Clone)]
pub struct MonteCarloSimulationElement {
    market_index: MarketIndex,
    simulation: SharedElement<dyn ADMonteCarloSimulationElement>,
    calibration_quality: Option<HullWhiteCalibrationQuality>,
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
            calibration_quality: None,
        }
    }

    /// Attaches the diagnostics produced when calibrating the simulation model.
    #[must_use]
    pub fn with_calibration_quality(
        mut self,
        quality: Option<HullWhiteCalibrationQuality>,
    ) -> Self {
        self.calibration_quality = quality;
        self
    }
    /// Calibration diagnostics, or `None` for fixed and non-Hull-White models.
    #[must_use]
    pub const fn calibration_quality(&self) -> Option<&HullWhiteCalibrationQuality> {
        self.calibration_quality.as_ref()
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

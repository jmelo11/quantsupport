//! Concrete Monte Carlo simulation produced by the
//! [`SimulationBuilder`](crate::simulations::simulationbuilder::SimulationBuilder).

use crate::{
    ad::dual::DualFwd,
    core::{
        elements::montecarlosimulationelement::ADMonteCarloSimulationElement, pillars::Pillars,
    },
    indices::marketindex::MarketIndex,
    simulations::simulation::MonteCarloSimulation,
    time::date::Date,
};

/// A materialized Monte Carlo simulation: a set of paths simulated on a fixed
/// date grid by a configured model (Hull-White, Brownian motion, ...).
///
/// Paths are stored as [`DualFwd`] constants so they can be combined with
/// AD-enabled market data during pricing; the simulation itself carries no
/// pillars.
pub struct GeneratedMonteCarloSimulation {
    market_index: MarketIndex,
    dates: Vec<Date>,
    paths: Vec<Vec<DualFwd>>,
    n_paths: i64,
    dt: f64,
}

impl GeneratedMonteCarloSimulation {
    /// Creates a new generated simulation from `f64` paths.
    ///
    /// `paths[p][i]` is the simulated value of path `p` at `dates[i]`;
    /// `dt` is the average time step in years.
    #[must_use]
    pub fn new(market_index: MarketIndex, dates: Vec<Date>, paths: Vec<Vec<f64>>, dt: f64) -> Self {
        let n_paths = i64::try_from(paths.len()).unwrap_or(i64::MAX);
        let paths = paths
            .into_iter()
            .map(|path| path.into_iter().map(DualFwd::new).collect())
            .collect();
        Self {
            market_index,
            dates,
            paths,
            n_paths,
            dt,
        }
    }
}

impl MonteCarloSimulation<DualFwd> for GeneratedMonteCarloSimulation {
    fn path(&self) -> &Vec<Vec<DualFwd>> {
        &self.paths
    }

    fn n_paths(&self) -> i64 {
        self.n_paths
    }

    fn dates(&self) -> &[Date] {
        &self.dates
    }

    fn dt(&self) -> f64 {
        self.dt
    }

    fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }
}

impl Pillars<DualFwd> for GeneratedMonteCarloSimulation {
    fn pillars(&self) -> Option<Vec<(String, &DualFwd)>> {
        None
    }

    fn pillar_labels(&self) -> Option<Vec<String>> {
        None
    }

    fn put_pillars_on_tape(&mut self) {}
}

impl ADMonteCarloSimulationElement for GeneratedMonteCarloSimulation {}

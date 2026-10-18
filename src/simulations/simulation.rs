use crate::{ad::scalar::Scalar, indices::marketindex::MarketIndex, time::date::Date};

type Matrix<T> = Vec<Vec<T>>;

/// Describes the attributes that a derived monte-carlo
/// simulation must have.
pub trait MonteCarloSimulation<T>
where
    T: Scalar,
{
    /// Returns the simulated paths of the Monte Carlo simulation, where each path is a vector of
    /// values corresponding to the simulation dates.
    fn path(&self) -> &Matrix<T>;
    /// Returns the number of simulated paths in the Monte Carlo simulation.
    fn n_paths(&self) -> i64;
    /// Returns the simulation date grid.
    fn dates(&self) -> &[Date];
    /// Returns the time step (dt) used in the Monte Carlo simulation, which is typically the
    /// time interval between simulation dates expressed in years.
    /// Materialized simulations built by `SimulationBuilder` return the
    /// average grid step; individual date intervals can differ.
    fn dt(&self) -> f64;
    /// Returns the market index associated with the Monte Carlo simulation, which is used to determine
    /// the underlying asset for the simulation.
    fn market_index(&self) -> MarketIndex;
}

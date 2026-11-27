//! Monte Carlo simulation engine.
//!
//! Path generation and simulation infrastructure for pricing
//! path-dependent instruments.

/// Stores materialized simulation paths and their market conventions.
pub mod generatedsimulation;
/// Provides the common Monte Carlo simulation interface.
pub mod simulation;
/// Builds materialized paths from a simulation configuration and market inputs.
pub mod simulationbuilder;

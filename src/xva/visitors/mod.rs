//! Visitors that implement the XVA simulation pipeline.
//!
//! * [`Inspector`](inspector::Inspector) -- collects market-data requests from claims.
//! * [`MarketModel`](marketmodel::MarketModel) -- trait for Monte Carlo path generation.
//! * [`ExposureEvaluator`](exposureevaluator::ExposureEvaluator) -- computes NPV cubes,
//!   exposure profiles (EPE/ENE/EE), and optionally XVA values with sensitivities.

/// Bulk claim compression — merges compatible cashflows to reduce Monte Carlo cost.
pub mod claimcompressionpreprocessor;
/// The [`ClaimPreprocessor`] trait for preprocessing claims before simulation.
pub mod claimpreprocessor;
/// Monte Carlo exposure evaluator with optional Savine-style parallel AAD.
pub mod exposureevaluator;
/// Resolves realized fixings from a [`FixingStore`].
pub mod fixingpreprocessor;
/// Defines the [`MarketModel`] trait and the [`SimulationResponse`] data container.
pub mod marketmodel;
/// Collects simulation requests from contingent claims and resolves discount policies.
pub mod preprocessorexecutor;

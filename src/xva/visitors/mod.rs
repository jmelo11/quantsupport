//! Visitors that implement the XVA simulation pipeline.
//!
//! * [`Inspector`](inspector::Inspector) -- collects market-data requests from claims.
//! * [`MarketModel`](marketmodel::MarketModel) -- trait for Monte Carlo path generation.
//! * [`ExposureEvaluator`](exposureevaluator::ExposureEvaluator) -- computes NPV cubes,
//!   exposure profiles (EPE/ENE/EE), and optionally XVA values with sensitivities.

/// Collects simulation requests from contingent claims and resolves discount policies.
pub mod inspector;
/// Defines the [`MarketModel`] trait and the [`SimulationResponse`] data container.
pub mod marketmodel;
/// Monte Carlo exposure evaluator with optional Savine-style parallel AAD.
pub mod exposureevaluator;

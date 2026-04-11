//! Visitors that implement the XVA simulation pipeline.
//!
//! * [`Inspector`](inspector::Inspector) — collects market-data requests from claims.
//! * [`MarketModel`](marketgenerator::MarketModel) — trait for Monte Carlo path generation.
//! * [`ExposureEvaluator`](exposureevaluator::ExposureEvaluator) — computes EPE / ENE / EE
//!   exposure profiles over simulated paths.

/// Collects simulation requests from contingent claims and resolves discount policies.
pub mod inspector;
/// Defines the [`MarketModel`] trait and the [`SimulationResponse`] data container.
pub mod marketmodel;
/// Monte Carlo exposure evaluator producing [`ExposureEvaluation`] profiles.
pub mod exposureevaluator;

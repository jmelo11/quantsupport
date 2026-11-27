//! Visitors that implement the XVA simulation pipeline.
//!
//! * [`PreprocessorExecutor`](crate::xva::visitors::preprocessorexecutor::PreprocessorExecutor) -- collects market-data requests from claims.
//! * [`MarketModel`](crate::xva::visitors::marketmodel::MarketModel) -- trait for Monte Carlo path generation.
//! * [`ExposureEvaluator`](crate::xva::visitors::exposureevaluator::ExposureEvaluator) -- computes NPV cubes,
//!   exposure profiles (EPE/ENE/EE), and optionally XVA values with sensitivities.

/// Combines compatible deterministic claims when their original trade attribution is not needed.
pub mod claimcompressionpreprocessor;
/// Collects the simulation requests declared by contingent claims.
pub mod claimpreprocessor;
/// Evaluates claims over simulated paths and constructs exposure cubes and adjustment sensitivities.
pub mod exposureevaluator;
/// Resolves historical fixings needed by contingent claims.
pub mod fixingpreprocessor;
/// Defines the model interface and response layout consumed by exposure calculations.
pub mod marketmodel;
/// Resolves claim conventions, shares identical request blocks and assigns their response indices.
pub mod preprocessorexecutor;

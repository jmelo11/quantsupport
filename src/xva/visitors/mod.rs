//! Visitors that implement the XVA simulation pipeline.
//!
//! * [`PreprocessorExecutor`](preprocessorexecutor::PreprocessorExecutor) -- collects market-data requests from claims.
//! * [`MarketModel`](marketmodel::MarketModel) -- trait for Monte Carlo path generation.
//! * [`ExposureEvaluator`](exposureevaluator::ExposureEvaluator) -- computes NPV cubes,
//!   exposure profiles (EPE/ENE/EE), and optionally XVA values with sensitivities.

pub mod claimcompressionpreprocessor;
/// Claim preprocessing into simulation requests.
pub mod claimpreprocessor;
pub mod exposureevaluator;
/// Historical-fixing preprocessing.
pub mod fixingpreprocessor;
pub mod marketmodel;
pub mod preprocessorexecutor;

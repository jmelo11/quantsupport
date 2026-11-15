//! Exposure simulation and credit, debit, and funding adjustments.
//!
//! This module provides the building blocks for computing exposure profiles
//! (EPE, ENE, EE) and XVA values over Monte Carlo scenarios.  The workflow is:
//!
//! 1. **Decompose** each trade into a flat list of [`ContingentClaim`](contigentclaim::ContingentClaim)s
//!    using [`IntoContingentClaims`](makecontigentclaim::IntoContingentClaims) or
//!    [`MakeContingentClaim`](makecontigentclaim::MakeContingentClaim).
//! 2. **Inspect** the claims with [`PreprocessorExecutor`](visitors::preprocessorexecutor::PreprocessorExecutor) to
//!    collect simulation requests and assign flat-vector indices.
//! 3. **Generate** market scenarios via a [`MarketModel`](visitors::marketmodel::MarketModel)
//!    implementation (e.g. LGM).
//! 4. **Evaluate** with [`ExposureEvaluator`](visitors::exposureevaluator::ExposureEvaluator)
//!    for cubes, or [`evaluate_with_xva`](visitors::exposureevaluator::evaluate_with_xva)
//!    for cubes + XVA values + sensitivities.
//!
//! [`XvaEngine`](engine::XvaEngine) assembles the high-level LGM workflow and
//! calculates CVA and FVA per netting set. DVA is available through the
//! lower-level [`aggregator::DvaFactory`]; capital adjustments and exposure
//! quantiles are not implemented by this module.
//!
//! Exposure profiles are pointwise path means. ENE is signed and nonpositive,
//! while CVA and DVA aggregators report loss/benefit magnitudes. FVA integrates
//! signed exposure. Returned NPV cubes precede variation margin, even when
//! adjustment calculations use collateralized exposures.

/// Binds the numerical exposure transforms and weights used by native adjustment programs.
pub mod aggregationplan;
/// Calculates valuation adjustments from path exposures and their credit or funding weights.
pub mod aggregator;
/// Defines how a contingent claim turns market observations into a payoff.
pub mod claimevaluationstrategy;
/// Represents a dated payment and the observations required to value it.
pub mod contigentclaim;
/// Defines collateral terms and applies variation-margin rules to path exposures.
pub mod csa;
/// Prepares and runs the LGM exposure and valuation-adjustment workflow.
pub mod engine;
/// Executes a prepared native program with the current market binding.
mod execution;
/// Builds contingent claims or decomposes supported instruments into claim collections.
pub mod makecontigentclaim;
/// Groups client claims under a shared collateral agreement for exposure calculations.
pub mod nettingset;
/// Preprocesses claims, simulates market observations and evaluates exposure paths.
pub mod visitors;

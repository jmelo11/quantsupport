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

pub mod aggregator;
pub mod claimevaluationstrategy;
pub mod contigentclaim;
pub mod csa;
/// High-level XVA engine and model setup.
pub mod engine;
pub mod makecontigentclaim;
pub mod nettingset;
pub mod visitors;

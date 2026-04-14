//! XVA (Credit / Funding / Capital Valuation Adjustments) framework.
//!
//! This module provides the building blocks for computing exposure profiles
//! (EPE, ENE, EE) and XVA values over Monte Carlo scenarios.  The workflow is:
//!
//! 1. **Decompose** each trade into a flat list of [`ContingentClaim`](contigentclaim::ContingentClaim)s
//!    using [`IntoContingentClaims`](makecontigentclaim::IntoContingentClaims) or
//!    [`MakeContingentClaim`](makecontigentclaim::MakeContingentClaim).
//! 2. **Inspect** the claims with [`Inspector`](visitors::inspector::Inspector) to
//!    collect simulation requests and assign flat-vector indices.
//! 3. **Generate** market scenarios via a [`MarketModel`](visitors::marketmodel::MarketModel)
//!    implementation (e.g. LGM).
//! 4. **Evaluate** with [`ExposureEvaluator`](visitors::exposureevaluator::ExposureEvaluator)
//!    for cubes, or [`evaluate_with_xva`](visitors::exposureevaluator::evaluate_with_xva)
//!    for cubes + XVA values + sensitivities.

/// Evaluation strategies that define how a single contingent claim is valued.
pub mod claimevaluationstrategy;
/// The atomic unit of exposure: a single contingent cashflow.
pub mod contigentclaim;
/// High-level XVA engine — takes a PricingContext and runs the full pipeline.
pub mod engine;
/// Builder and conversion helpers for creating contingent claims.
pub mod makecontigentclaim;
pub mod va;
/// Visitor pipeline: inspection, market generation, and exposure evaluation.
pub mod visitors;

//! XVA (Credit / Funding / Capital Valuation Adjustments) framework.
//!
//! This module provides the building blocks for computing exposure profiles
//! (EPE, ENE, EE) over Monte Carlo scenarios.  The workflow is:
//!
//! 1. **Decompose** each trade into a flat list of [`ContingentClaim`](contigentclaim::ContingentClaim)s
//!    using [`IntoContingentClaims`](makecontigentclaim::IntoContingentClaims) or
//!    [`MakeContingentClaim`](makecontigentclaim::MakeContingentClaim).
//! 2. **Inspect** the claims with [`Inspector`](visitors::inspector::Inspector) to
//!    collect simulation requests and assign flat-vector indices.
//! 3. **Generate** market scenarios via a [`MarketModel`](visitors::marketgenerator::MarketModel)
//!    implementation (e.g. LGM).
//! 4. **Evaluate** exposure profiles with
//!    [`ExposureEvaluator`](visitors::exposureevaluator::ExposureEvaluator).

/// Evaluation strategies that define how a single contingent claim is valued.
pub mod claimevaluationstrategy;
/// The atomic unit of exposure: a single contingent cashflow.
pub mod contigentclaim;
/// Builder and conversion helpers for creating contingent claims.
pub mod makecontigentclaim;
/// Visitor pipeline: inspection, market generation, and exposure evaluation.
pub mod visitors;
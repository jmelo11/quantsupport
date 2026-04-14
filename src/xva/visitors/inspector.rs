//! Claim inspection and market-data request collection.
//!
//! The [`Inspector`] walks a slice of [`ContingentClaim`]s, collecting each
//! claim's [`SimulationRequest`] into a flat vector and assigning indices
//! so that the evaluator can locate each claim's response within a scenario.

use crate::{
    core::{
        collateral::DiscountPolicy,
        marketdatahandling::{
            discountrequest::DiscountRequest, forwardraterequest::ForwardRateRequest,
            fxrequest::FxRequest, pathdependentrequest::PathDependentRequest,
            spotrequest::SpotRequest,
        },
    },
    xva::contigentclaim::ContingentClaim,
};

/// Declares the market data a [`ContingentClaim`] needs for simulation.
///
/// Each field is optional — `None` means the claim does not require that
/// data category.  The [`Inspector`] collects one of these per claim and
/// passes the full list to the [`MarketModel`](super::marketgenerator::MarketModel).
#[derive(Default, Clone)]
pub struct SimulationRequest {
    /// Discount factor request.
    pub discount_request: Option<DiscountRequest>,
    /// Forward rate request.
    pub forward_rate_request: Option<ForwardRateRequest>,
    /// FX rate request.
    pub fx_request: Option<FxRequest>,
    /// Spot observation request.
    pub spot_request: Option<SpotRequest>,
    /// Path-dependent observation request.
    pub path_dependent_request: Option<PathDependentRequest>,
}

/// Collects [`SimulationRequest`]s from a set of [`ContingentClaim`]s.
///
/// The inspector is the first step of the XVA pipeline.  It:
/// 1. Asks each claim what market data it needs.
/// 2. Optionally resolves a discount curve via a [`DiscountPolicy`].
/// 3. Assigns a flat-vector index to each claim so that the evaluator
///    can locate the corresponding [`SimulationResponse`](super::marketgenerator::SimulationResponse)
///    within each scenario step.
pub struct Inspector {
    requests: Vec<SimulationRequest>,
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl Default for Inspector {
    fn default() -> Self {
        Self::new()
    }
}

impl Inspector {
    /// Creates an inspector without a discount policy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            discount_policy: None,
        }
    }

    /// Creates an inspector with a discount policy that will resolve
    /// the discount curve for each claim during [`visit`].
    #[must_use]
    pub fn with_discount_policy(policy: Box<dyn DiscountPolicy>) -> Self {
        Self {
            requests: Vec::new(),
            discount_policy: Some(policy),
        }
    }

    /// Inspects all claims, collecting their simulation requests
    /// and writing back the flat-vector indices so the evaluator can locate
    /// each claim's data.
    ///
    /// If a [`DiscountPolicy`] was provided, the inspector resolves the
    /// discount index for each claim and attaches a [`DiscountRequest`]
    /// to the corresponding [`SimulationRequest`].
    pub fn visit(&mut self, claims: &mut [ContingentClaim]) {
        self.requests.clear();
        self.requests.reserve(claims.len());
        for (i, claim) in claims.iter_mut().enumerate() {
            let mut request = claim.simulation_request();

            if let Some(policy) = &self.discount_policy {
                if let Ok(discount_index) = policy.accept(claim) {
                    request.discount_request =
                        Some(DiscountRequest::new(discount_index, claim.payment_date()));
                }
            }

            claim.set_idx(i);
            self.requests.push(request);
        }
    }

    /// Returns the collected simulation requests, one per claim, in visit order.
    #[must_use]
    pub fn requests(&self) -> &[SimulationRequest] {
        &self.requests
    }
}

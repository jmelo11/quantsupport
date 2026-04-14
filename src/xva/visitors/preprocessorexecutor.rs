//! Claim inspection and market-data request collection.
//!
//! The [`PreprocessorExecutor`] walks a slice of [`ContingentClaim`]s, running a
//! pipeline of [`ClaimPreprocessor`]s (discount policy, fixing resolution,
//! etc.) and then collecting each claim's [`SimulationRequest`] into a
//! flat vector while assigning indices so the evaluator can locate each
//! claim's response within a scenario.

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

use super::claimpreprocessor::ClaimPreprocessor;

/// Declares the market data a [`ContingentClaim`] needs for simulation.
///
/// Each field is optional — `None` means the claim does not require that
/// data category.  The [`PreprocessorExecutor`] collects one of these per claim and
/// passes the full list to the [`MarketModel`](super::marketmodel::MarketModel).
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
/// The PreprocessorExecutor is the first step of the XVA pipeline.  It runs a
/// pipeline of [`ClaimPreprocessor`]s (discount resolution, fixing
/// resolution, compression, …) and then:
///
/// 1. Asks each claim what market data it needs.
/// 2. Resolves the discount curve via the attached [`DiscountPolicy`].
/// 3. Assigns a flat-vector index to each claim so the evaluator can
///    locate the corresponding [`SimulationResponse`](super::marketmodel::SimulationResponse).
pub struct PreprocessorExecutor {
    requests: Vec<SimulationRequest>,
    discount_policy: Option<Box<dyn DiscountPolicy>>,
    preprocessors: Vec<Box<dyn ClaimPreprocessor>>,
}

impl Default for PreprocessorExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PreprocessorExecutor {
    /// Creates an PreprocessorExecutor without any preprocessors or discount policy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            discount_policy: None,
            preprocessors: Vec::new(),
        }
    }

    /// Creates an PreprocessorExecutor with a discount policy that will resolve
    /// the discount curve for each claim during [`visit`](Self::visit).
    #[must_use]
    pub fn with_discount_policy(policy: Box<dyn DiscountPolicy>) -> Self {
        Self {
            requests: Vec::new(),
            discount_policy: Some(policy),
            preprocessors: Vec::new(),
        }
    }

    /// Appends a preprocessor to the pipeline. Preprocessors run in
    /// the order they are added, before simulation requests are built.
    #[must_use]
    pub fn with_preprocessor(mut self, preprocessor: Box<dyn ClaimPreprocessor>) -> Self {
        self.preprocessors.push(preprocessor);
        self
    }

    /// Like [`visit`](Self::visit), but operates over multiple disjoint
    /// claim slices while maintaining a single global index counter.
    pub fn visit<'a>(&mut self, slices: impl IntoIterator<Item = &'a mut [ContingentClaim]>) {
        self.requests.clear();
        let mut global_idx = 0;
        for claims in slices {
            for claim in claims.iter_mut() {
                for pp in &self.preprocessors {
                    pp.process(claim);
                }
                let mut request = claim.simulation_request();
                if let Some(policy) = &self.discount_policy {
                    if let Ok(discount_index) = policy.accept(claim) {
                        request.discount_request =
                            Some(DiscountRequest::new(discount_index, claim.payment_date()));
                    }
                }
                claim.set_idx(global_idx);
                self.requests.push(request);
                global_idx += 1;
            }
        }
    }

    /// Returns the collected simulation requests, one per claim, in visit order.
    #[must_use]
    pub fn requests(&self) -> &[SimulationRequest] {
        &self.requests
    }
}

//! Claim inspection and market-data request collection.
//!
//! The [`PreprocessorExecutor`] walks a set of [`NettingSet`]s, running a
//! pipeline of [`ClaimPreprocessor`]s (fixing resolution, etc.),
//! optionally compressing claims, and then collecting each claim's
//! [`SimulationRequest`] into a flat vector while assigning indices so
//! the evaluator can locate each claim's response within a scenario.
//!
//! Each [`NettingSet`] carries its own [`DiscountPolicy`] which is used
//! to resolve the discount curve for claims within that set.

use crate::{
    core::marketdatahandling::{
        discountrequest::DiscountRequest, forwardraterequest::ForwardRateRequest,
        fxrequest::FxRequest, pathdependentrequest::PathDependentRequest,
        spotrequest::SpotRequest,
    },
    xva::nettingset::NettingSet,
};

use super::{
    claimcompressionpreprocessor::ClaimCompressionPreprocessor,
    claimpreprocessor::ClaimPreprocessor,
};

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

/// Collects [`SimulationRequest`]s from a set of [`NettingSet`]s.
///
/// The PreprocessorExecutor is the first step of the XVA pipeline.  For
/// each netting set it:
///
/// 1. Runs the per-claim [`ClaimPreprocessor`] pipeline.
/// 2. Optionally compresses compatible claims via
///    [`ClaimCompressionPreprocessor`].
/// 3. Resolves the discount curve using each netting set's own
///    [`DiscountPolicy`](crate::core::collateral::DiscountPolicy).
/// 4. Assigns a flat-vector index to each claim so the evaluator can
///    locate the corresponding [`SimulationResponse`](super::marketmodel::SimulationResponse).
pub struct PreprocessorExecutor {
    requests: Vec<SimulationRequest>,
    preprocessors: Vec<Box<dyn ClaimPreprocessor>>,
    compress: bool,
}

impl Default for PreprocessorExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PreprocessorExecutor {
    /// Creates a PreprocessorExecutor without any preprocessors.
    #[must_use]
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            preprocessors: Vec::new(),
            compress: false,
        }
    }

    /// Appends a preprocessor to the pipeline. Preprocessors run in
    /// the order they are added, before simulation requests are built.
    #[must_use]
    pub fn with_preprocessor(mut self, preprocessor: Box<dyn ClaimPreprocessor>) -> Self {
        self.preprocessors.push(preprocessor);
        self
    }

    /// Enables claim compression. Compatible deterministic cashflows
    /// within each netting set will be merged after per-claim
    /// preprocessing, reducing the number of evaluations in the
    /// Monte Carlo loop.
    #[must_use]
    pub fn with_compression(mut self) -> Self {
        self.compress = true;
        self
    }

    /// Visits all netting sets, preprocessing claims and collecting
    /// simulation requests with a single global index counter.
    ///
    /// For each netting set the pipeline is:
    /// 1. Per-claim preprocessing (fixing resolution, …).
    /// 2. Claim compression (if enabled).
    /// 3. Discount resolution via the netting set's own
    ///    [`DiscountPolicy`](crate::core::collateral::DiscountPolicy).
    /// 4. Global index assignment.
    pub fn visit<'a>(&mut self, netting_sets: impl IntoIterator<Item = &'a mut NettingSet>) {
        self.requests.clear();
        let mut global_idx = 0;

        for ns in netting_sets {
            // Phase 1: per-claim preprocessing.
            for claim in ns.claims_mut() {
                for pp in &self.preprocessors {
                    pp.process(claim);
                }
            }

            // Phase 2: compression.
            if self.compress {
                ClaimCompressionPreprocessor::compress(ns.claims_vec_mut());
            }

            // Phase 3 + 4: discount resolution & index assignment.
            let (policy, claims) = ns.discount_policy_and_claims_mut();
            for claim in claims {
                let mut request = claim.simulation_request();
                if let Ok(discount_index) = policy.accept(claim) {
                    request.discount_request =
                        Some(DiscountRequest::new(discount_index, claim.payment_date()));
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

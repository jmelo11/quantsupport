use crate::{
    core::marketdatahandling::{
        forwardraterequest::ForwardRateRequest, fxrequest::FxRequest,
        pathdependentrequest::PathDependentRequest, spotrequest::SpotRequest,
    },
    xva::contigentclaim::ContingentClaim,
};

/// Declares the market data a [`ContingentClaim`] needs for simulation.
///
/// Discounting is not included here — it is resolved by the context
/// based on the discount policy (CSA, risk-free, etc.).
#[derive(Default)]
pub struct SimulationRequest {
    pub forward_rate_request: Option<ForwardRateRequest>,
    pub fx_request: Option<FxRequest>,
    pub spot_request: Option<SpotRequest>,
    pub path_dependent_request: Option<PathDependentRequest>,
}

#[derive(Default)]
pub struct Inspector {
    requests: Vec<SimulationRequest>,
}

impl Inspector {
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
        }
    }

    /// Inspects all claims, collecting their simulation requests
    /// and writing back the flat-vector indices so the evaluator can locate
    /// each claim's data.
    pub fn visit(&mut self, claims: &mut [ContingentClaim]) {
        self.requests.clear();
        self.requests.reserve(claims.len());
        for (i, claim) in claims.iter_mut().enumerate() {
            let request = claim.simulation_request();
            claim.set_idx(i);
            self.requests.push(request);
        }
    }

    pub fn requests(&self) -> &[SimulationRequest] {
        &self.requests
    }
}

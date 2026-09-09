//! Per-event market-data requests collected from a script.

use crate::core::marketdatahandling::{
    discountrequest::DiscountRequest, forwardraterequest::ForwardRateRequest, fxrequest::FxRequest,
};

/// Market data required to evaluate one scripted event.
///
/// The vectors preserve expression indexing order so generated responses can
/// be read directly by the scripting evaluators.
#[derive(Clone, Default)]
pub struct SimulationDataRequest {
    discounts: Vec<DiscountRequest>,
    forwards: Vec<ForwardRateRequest>,
    fx: Vec<FxRequest>,
    requires_numeraire: bool,
}

impl SimulationDataRequest {
    /// Creates an empty request.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            discounts: Vec::new(),
            forwards: Vec::new(),
            fx: Vec::new(),
            requires_numeraire: false,
        }
    }

    /// Creates an empty request with capacity for each request category.
    #[must_use]
    pub fn with_capacity(discounts: usize, forwards: usize, fx: usize) -> Self {
        Self {
            discounts: Vec::with_capacity(discounts),
            forwards: Vec::with_capacity(forwards),
            fx: Vec::with_capacity(fx),
            requires_numeraire: false,
        }
    }

    /// Adds a discount-factor request.
    pub fn push_df(&mut self, request: DiscountRequest) {
        self.discounts.push(request);
    }

    /// Adds a forward-rate request.
    pub fn push_fwd(&mut self, request: ForwardRateRequest) {
        self.forwards.push(request);
    }

    /// Adds an FX request.
    pub fn push_fx(&mut self, request: FxRequest) {
        self.fx.push(request);
    }

    /// Discount-factor requests in expression-index order.
    #[must_use]
    pub fn dfs(&self) -> &[DiscountRequest] {
        &self.discounts
    }

    /// Forward-rate requests in expression-index order.
    #[must_use]
    pub fn fwds(&self) -> &[ForwardRateRequest] {
        &self.forwards
    }

    /// FX requests in expression-index order.
    #[must_use]
    pub fn fxs(&self) -> &[FxRequest] {
        &self.fx
    }

    /// Marks the event as requiring reference-date discounting.
    pub const fn require_numeraire(&mut self) {
        self.requires_numeraire = true;
    }

    /// Returns whether the event contains a `pays` expression.
    #[must_use]
    pub const fn requires_numeraire(&self) -> bool {
        self.requires_numeraire
    }
}

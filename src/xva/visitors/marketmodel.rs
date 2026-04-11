//! Market model trait and simulation response types.
//!
//! A [`MarketModel`] produces Monte Carlo paths as sequences of
//! [`SimulationResponse`]s.  Each response corresponds to one claim
//! at one simulation date and carries the subset of market data that
//! the claim declared via its [`SimulationRequest`].

use crate::{
    ad::scalar::Scalar,
    core::marketdatahandling::{
        discountrequest::DiscountRequest, forwardraterequest::ForwardRateRequest,
        fxrequest::FxRequest, pathdependentrequest::PathDependentRequest, spotrequest::SpotRequest,
    },
    time::date::Date,
    utils::errors::Result,
    xva::visitors::inspector::SimulationRequest,
};

/// A full Monte Carlo path: one `Vec<SimulationResponse<T>>` per simulation date.
///
/// `scenario[d][i]` is the response for claim `i` at simulation date `d`.
pub type PathScenario<T> = Vec<Vec<SimulationResponse<T>>>;

/// Market data produced by a [`MarketModel`] for a single claim at a single
/// simulation date.
///
/// Each field corresponds to a request category in [`SimulationRequest`].
/// Fields are `None` when the claim did not request that data category.
pub struct SimulationResponse<T: Scalar> {
    pub discounts: Option<T>,
    pub forward_rates: Option<T>,
    pub fx_rates: Option<T>,
    pub spots: Option<T>,
    pub path_dependent_observations: Option<T>,
    pub numeraire: Option<T>,
}

impl<T: Scalar> Default for SimulationResponse<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> SimulationResponse<T>
where
    T: Scalar,
{
    /// Creates an empty response with all fields set to `None`.
    #[must_use] 
    pub const fn new() -> Self {
        Self {
            discounts: None,
            forward_rates: None,
            fx_rates: None,
            spots: None,
            path_dependent_observations: None,
            numeraire: None,
        }
    }
}

/// Trait for Monte Carlo market models that generate simulated paths.
///
/// Implementors (e.g. LGM) must provide:
/// * [`path_iter`](Self::path_iter) — lazily yields one [`PathScenario`] per MC path.
/// * [`set_evaluation_dates`](Self::set_evaluation_dates) — configures the time grid.
/// * Individual `resolve_*` methods for each market data category.
///
/// A default [`resolve_request`](Self::resolve_request) implementation dispatches
/// to the individual resolvers.
pub trait MarketModel<T: Scalar> {
    /// Returns an iterator that lazily generates Monte Carlo paths.
    fn path_iter(&self) -> Box<dyn Iterator<Item = PathScenario<T>> + Send + '_>;

    /// Sets the simulation date grid.
    fn set_evaluation_dates(&mut self, dates: Vec<Date>);

    /// Resolves a full [`SimulationRequest`] into a [`SimulationResponse`]
    /// by dispatching to the individual `resolve_*` methods.
    ///
    /// # Errors
    /// Returns an error if any of the individual resolvers fail.
    fn resolve_request(
        &self,
        eval_date: Date,
        request: &SimulationRequest,
    ) -> Result<SimulationResponse<T>> {
        let mut response = SimulationResponse::new();

        match &request.forward_rate_request {
            Some(req) => {
                let rate = self.resolve_forward_rate_request(eval_date, req)?;
                response.forward_rates = Some(rate);
            }
            None => response.forward_rates = None,
        }

        match &request.fx_request {
            Some(req) => {
                let rate = self.resolve_fx_request(eval_date, req)?;
                response.fx_rates = Some(rate);
            }
            None => response.fx_rates = None,
        }

        match &request.spot_request {
            Some(req) => {
                let spot = self.resolve_spot_request(eval_date, req)?;
                response.spots = Some(spot);
            }
            None => response.spots = None,
        }

        match &request.path_dependent_request {
            Some(req) => {
                let obs = self.resolve_path_dependent_request(eval_date, req)?;
                response.path_dependent_observations = Some(obs);
            }
            None => response.path_dependent_observations = None,
        }

        Ok(response)
    }

    /// Resolves a discount factor `P(eval_date, payment_date)` from the simulated state.
    ///
    /// # Errors
    /// Returns an error if the discount factor cannot be resolved.
    fn resolve_discount_request(&self, eval_date: Date, request: &DiscountRequest) -> Result<T>;
    /// Resolves a forward rate from the simulated state.
    ///
    /// # Errors
    /// Returns an error if the forward rate cannot be resolved.
    fn resolve_forward_rate_request(
        &self,
        eval_date: Date,
        request: &ForwardRateRequest,
    ) -> Result<T>;
    /// Resolves an FX rate from the simulated state.
    ///
    /// # Errors
    /// Returns an error if the FX rate cannot be resolved.
    fn resolve_fx_request(&self, eval_date: Date, request: &FxRequest) -> Result<T>;
    /// Resolves a spot observation from the simulated state.
    ///
    /// # Errors
    /// Returns an error if the spot observation cannot be resolved.
    fn resolve_spot_request(&self, eval_date: Date, request: &SpotRequest) -> Result<T>;
    /// Resolves a path-dependent observation from the simulated state.
    ///
    /// # Errors
    /// Returns an error if the path-dependent observation cannot be resolved.
    fn resolve_path_dependent_request(
        &self,
        eval_date: Date,
        request: &PathDependentRequest,
    ) -> Result<T>;
}

use nalgebra::Scalar;

use crate::{
    core::marketdatahandling::{
        discountrequest::DiscountRequest, forwardraterequest::ForwardRateRequest,
        fxrequest::FxRequest, pathdependentrequest::PathDependentRequest, spotrequest::SpotRequest,
    },
    time::date::Date,
    utils::errors::Result,
    xva::visitors::inspector::SimulationRequest,
};

pub type PathScenario<T> = Vec<Vec<SimulationResponse<T>>>;

pub struct SimulationResponse<T: Scalar> {
    pub discounts: Option<T>,
    pub forward_rates: Option<T>,
    pub fx_rates: Option<T>,
    pub spots: Option<T>,
    pub path_dependent_observations: Option<T>,
}

impl<T> SimulationResponse<T>
where
    T: Scalar,
{
    pub fn new() -> Self {
        Self {
            discounts: None,
            forward_rates: None,
            fx_rates: None,
            spots: None,
            path_dependent_observations: None,
        }
    }
}

pub trait MarketModel<T: Scalar> {
    fn path_iter(&self) -> Box<dyn Iterator<Item = PathScenario<T>> + Send + '_>;

    fn set_evaluation_dates(&mut self, dates: Vec<Date>);

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

    fn resolve_discount_request(&self, eval_date: Date, request: &DiscountRequest) -> Result<T>;
    fn resolve_forward_rate_request(
        &self,
        eval_date: Date,
        request: &ForwardRateRequest,
    ) -> Result<T>;
    fn resolve_fx_request(&self, eval_date: Date, request: &FxRequest) -> Result<T>;
    fn resolve_spot_request(&self, eval_date: Date, request: &SpotRequest) -> Result<T>;
    fn resolve_path_dependent_request(
        &self,
        eval_date: Date,
        request: &PathDependentRequest,
    ) -> Result<T>;
}

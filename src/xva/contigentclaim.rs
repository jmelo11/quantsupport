use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        marketdatahandling::{
            forwardraterequest::ForwardRateRequest, fxrequest::FxRequest,
            pathdependentrequest::PathDependentRequest, spotrequest::SpotRequest,
        },
        trade::Side,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::Result,
    xva::{
        claimevaluationstrategy::ClaimEvaluationStrategy,
        visitors::{inspector::SimulationRequest, marketgenerator::SimulationResponse},
    },
};

pub struct ContingentClaim {
    trade_id: String,
    leg_id: usize,
    idx: Option<usize>,
    payment_date: Date,
    fixing_date: Option<Date>,
    accrual_start: Option<Date>,
    accrual_end: Option<Date>,
    /// The currency in which this claim pays.
    currency: Currency,
    /// If set, the claim involves two currencies (e.g. FX forward, cross-currency swap).
    /// Both `currency` and `foreign_currency` are passed to the FX request so the context
    /// can triangulate and convert to the reporting currency.
    /// If `None`, only `currency` is passed — the context resolves conversion to reporting.
    foreign_currency: Option<Currency>,
    notional: f64,
    side: Side,
    evaluation_strategy: ClaimEvaluationStrategy,
    /// Market index for the underlying (forward rate, spot, etc.).
    /// Discounting is resolved by the context (CSA / discount policy).
    index: Option<MarketIndex>,
}

impl ContingentClaim {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trade_id: String,
        leg_id: usize,
        payment_date: Date,
        fixing_date: Option<Date>,
        accrual_start: Option<Date>,
        accrual_end: Option<Date>,
        currency: Currency,
        foreign_currency: Option<Currency>,
        notional: f64,
        side: Side,
        evaluation_strategy: ClaimEvaluationStrategy,
        index: Option<MarketIndex>,
    ) -> Self {
        Self {
            trade_id,
            leg_id,
            idx: None,
            payment_date,
            fixing_date,
            accrual_start,
            accrual_end,
            currency,
            foreign_currency,
            notional,
            side,
            evaluation_strategy,
            index,
        }
    }

    pub fn trade_id(&self) -> &str {
        &self.trade_id
    }

    pub const fn leg_id(&self) -> usize {
        self.leg_id
    }

    pub const fn payment_date(&self) -> Date {
        self.payment_date
    }

    pub const fn fixing_date(&self) -> Option<Date> {
        self.fixing_date
    }

    pub const fn accrual_start(&self) -> Option<Date> {
        self.accrual_start
    }

    pub const fn accrual_end(&self) -> Option<Date> {
        self.accrual_end
    }

    pub const fn currency(&self) -> Currency {
        self.currency
    }

    pub const fn foreign_currency(&self) -> Option<Currency> {
        self.foreign_currency
    }

    pub const fn notional(&self) -> f64 {
        self.notional
    }

    pub const fn side(&self) -> Side {
        self.side
    }

    pub const fn evaluation_strategy(&self) -> &ClaimEvaluationStrategy {
        &self.evaluation_strategy
    }

    pub fn index(&self) -> Option<&MarketIndex> {
        self.index.as_ref()
    }

    pub fn idx(&self) -> Option<usize> {
        self.idx
    }

    pub fn set_idx(&mut self, idx: usize) {
        self.idx = Some(idx);
    }

    /// Builds the simulation data requests needed to evaluate this claim.
    ///
    /// Discounting and reporting-currency conversion are handled by the context
    /// (discount policy / CSA). The claim only declares what market data it needs:
    /// currencies, forward rates, spot observations, or path-dependent observations.
    pub fn simulation_request(&self) -> SimulationRequest {
        let fx_request = match self.foreign_currency {
            Some(quote) => Some(FxRequest::pair(self.currency, quote)),
            None => Some(FxRequest::single(self.currency)),
        };

        match &self.evaluation_strategy {
            ClaimEvaluationStrategy::Deterministic { .. } => SimulationRequest {
                forward_rate_request: None,
                fx_request,
                spot_request: None,
                path_dependent_request: None,
            },

            ClaimEvaluationStrategy::LinearRate { .. }
            | ClaimEvaluationStrategy::NonLinearRate { .. } => {
                let forward_index = self.index.clone();
                let fixing_date = self.fixing_date.unwrap_or(self.payment_date);
                let forward_request = forward_index.map(|idx| {
                    ForwardRateRequest::new(idx, fixing_date)
                        .with_start_date(self.accrual_start.unwrap_or(fixing_date))
                        .with_end_date(self.accrual_end.unwrap_or(self.payment_date))
                });

                SimulationRequest {
                    forward_rate_request: forward_request,
                    fx_request,
                    spot_request: None,
                    path_dependent_request: None,
                }
            }

            ClaimEvaluationStrategy::SpotPayoff {
                observation_date, ..
            } => {
                let spot_request = self
                    .index
                    .clone()
                    .map(|idx| SpotRequest::new(idx, *observation_date));

                SimulationRequest {
                    forward_rate_request: None,
                    fx_request,
                    spot_request,
                    path_dependent_request: None,
                }
            }

            ClaimEvaluationStrategy::PathDependent {
                observation_dates, ..
            } => {
                let path_request = self
                    .index
                    .clone()
                    .map(|idx| PathDependentRequest::new(observation_dates.clone(), idx));

                SimulationRequest {
                    forward_rate_request: None,
                    fx_request,
                    spot_request: None,
                    path_dependent_request: path_request,
                }
            }

            ClaimEvaluationStrategy::ExerciseContingent { inner, .. } => {
                let inner_request = inner.simulation_request();
                SimulationRequest {
                    forward_rate_request: inner_request.forward_rate_request,
                    fx_request,
                    spot_request: inner_request.spot_request,
                    path_dependent_request: inner_request.path_dependent_request,
                }
            }
        }
    }

    /// Evaluates the value of a single claim at a given evaluation date using
    /// the simulated market data in the [`SimulationResponse`].
    pub fn evaluate_f64(&self, response: &SimulationResponse<f64>) -> Result<f64> {
        let sign = self.side().sign();
        let notional = self.notional();
        let fx = response.fx_rates.unwrap_or(1.0);
        let discount = response.discounts.unwrap_or(1.0);

        let raw = match self.evaluation_strategy() {
            ClaimEvaluationStrategy::Deterministic { amount } => *amount,

            ClaimEvaluationStrategy::LinearRate {
                spread,
                day_counter,
            } => {
                let rate = response.forward_rates.unwrap_or(0.0);
                let start = self.accrual_start().unwrap_or(self.payment_date());
                let end = self.accrual_end().unwrap_or(self.payment_date());
                let tau = day_counter.year_fraction(start, end);
                (rate + spread) * tau
            }

            ClaimEvaluationStrategy::NonLinearRate {
                payoff_ops,
                spread,
                strike: _,
                day_counter,
            } => {
                let rate = response.forward_rates.unwrap_or(0.0);
                let start = self.accrual_start().unwrap_or(self.payment_date());
                let end = self.accrual_end().unwrap_or(self.payment_date());
                let tau = day_counter.year_fraction(start, end);
                let fixing = rate + spread;
                payoff_ops.evaluate_f64(fixing).unwrap_or(0.0) * tau
            }

            ClaimEvaluationStrategy::SpotPayoff {
                payoff_ops,
                strike: _,
                ..
            } => {
                let spot = response.spots.unwrap_or(0.0);
                payoff_ops.evaluate_f64(spot).unwrap_or(0.0)
            }

            ClaimEvaluationStrategy::PathDependent {
                payoff_ops,
                strike: _,
                ..
            } => {
                let obs = response.path_dependent_observations.unwrap_or(0.0);
                payoff_ops.evaluate_f64(obs).unwrap_or(0.0)
            }

            ClaimEvaluationStrategy::ExerciseContingent { inner, .. } => {
                // Delegate to the inner claim; exercise logic to be handled upstream
                inner.evaluate_f64(response)?
            }
        };
        Ok(sign * notional * raw * discount * fx)
    }

    /// Evaluates the value of a single claim at a given evaluation date using
    /// the simulated market data in the [`SimulationResponse`].
    pub fn evaluate(&self, response: &SimulationResponse<DualFwd>) -> Result<DualFwd> {
        let sign = self.side().sign();
        let notional = self.notional();
        let fx = response.fx_rates.unwrap_or(DualFwd::one());
        let discount = response.discounts.unwrap_or(DualFwd::one());

        let raw: DualFwd = match self.evaluation_strategy() {
            ClaimEvaluationStrategy::Deterministic { amount } => DualFwd::scalar(*amount),

            ClaimEvaluationStrategy::LinearRate {
                spread,
                day_counter,
            } => {
                let rate = response.forward_rates.unwrap_or(DualFwd::zero());
                let start = self.accrual_start().unwrap_or(self.payment_date());
                let end = self.accrual_end().unwrap_or(self.payment_date());
                let tau = day_counter.year_fraction(start, end);
                ((rate + *spread) * tau).into()
            }

            ClaimEvaluationStrategy::NonLinearRate {
                payoff_ops,
                spread,
                strike: _,
                day_counter,
            } => {
                let rate = response.forward_rates.unwrap_or(DualFwd::zero());
                let start = self.accrual_start().unwrap_or(self.payment_date());
                let end = self.accrual_end().unwrap_or(self.payment_date());
                let tau = day_counter.year_fraction(start, end);
                let fixing: DualFwd = (rate + *spread).into();
                let payoff = payoff_ops.evaluate(fixing).unwrap_or(DualFwd::zero());
                (payoff * tau).into()
            }

            ClaimEvaluationStrategy::SpotPayoff {
                payoff_ops,
                strike: _,
                ..
            } => {
                let spot = response.spots.unwrap_or(DualFwd::zero());
                payoff_ops.evaluate(spot).unwrap_or(DualFwd::zero())
            }

            ClaimEvaluationStrategy::PathDependent {
                payoff_ops,
                strike: _,
                ..
            } => {
                let obs = response
                    .path_dependent_observations
                    .unwrap_or(DualFwd::zero());
                payoff_ops.evaluate(obs).unwrap_or(DualFwd::zero())
            }

            ClaimEvaluationStrategy::ExerciseContingent { inner, .. } => {
                // Delegate to the inner claim; exercise logic to be handled upstream
                inner.evaluate(response)?
            }
        };

        let result = raw * discount * fx * sign * notional;
        Ok(result.into())
    }
}

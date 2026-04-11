//! The atomic unit of exposure computation.
//!
//! A [`ContingentClaim`] represents a single contingent cashflow that can
//! declare its market-data requirements via [`simulation_request`](ContingentClaim::simulation_request)
//! and be valued on a simulated scenario via [`evaluate`](ContingentClaim::evaluate).
//!
//! Trades are decomposed into one or more `ContingentClaim`s by the
//! [`IntoContingentClaims`](super::makecontigentclaim::IntoContingentClaims) trait
//! or the [`MakeContingentClaim`](super::makecontigentclaim::MakeContingentClaim) builder.

use crate::{
    ad::scalar::Scalar,
    core::{
        collateral::Discountable,
        instrument::AssetClass,
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
        visitors::{inspector::SimulationRequest, marketmodel::SimulationResponse},
    },
};

/// A single contingent cashflow — the atomic building block for XVA exposure.
///
/// Each claim carries:
/// * **Identity** — `trade_id`, `leg_id`, and a flat-vector `idx` assigned by
///   the [`Inspector`](super::visitors::inspector::Inspector).
/// * **Dates** — `payment_date`, optional `fixing_date`, `accrual_start`, `accrual_end`.
/// * **Economics** — `currency`, optional `foreign_currency`, `notional`, `side`.
/// * **Valuation rule** — a [`ClaimEvaluationStrategy`] that defines how the raw
///   payoff is computed from simulated market data.
/// * **Market reference** — an optional [`MarketIndex`] identifying the underlying
///   rate or spot used for simulation.
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
    /// Creates a new `ContingentClaim` with all fields specified explicitly.
    ///
    /// Prefer using [`MakeContingentClaim`](super::makecontigentclaim::MakeContingentClaim)
    /// for a more ergonomic builder interface.
    #[allow(clippy::too_many_arguments)]
    #[must_use] 
    pub const fn new(
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

    /// Returns the trade identifier this claim belongs to.
    #[must_use] 
    pub fn trade_id(&self) -> &str {
        &self.trade_id
    }

    /// Returns the leg identifier within the trade.
    #[must_use] 
    pub const fn leg_id(&self) -> usize {
        self.leg_id
    }

    /// Returns the date on which this cashflow is paid.
    #[must_use] 
    pub const fn payment_date(&self) -> Date {
        self.payment_date
    }

    /// Returns the fixing date, if any.
    #[must_use] 
    pub const fn fixing_date(&self) -> Option<Date> {
        self.fixing_date
    }

    /// Returns the accrual period start date, if any.
    #[must_use] 
    pub const fn accrual_start(&self) -> Option<Date> {
        self.accrual_start
    }

    /// Returns the accrual period end date, if any.
    #[must_use] 
    pub const fn accrual_end(&self) -> Option<Date> {
        self.accrual_end
    }

    /// Returns the payment currency.
    #[must_use] 
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Returns the foreign currency, if this is a multi-currency claim.
    #[must_use] 
    pub const fn foreign_currency(&self) -> Option<Currency> {
        self.foreign_currency
    }

    /// Returns the notional amount.
    #[must_use] 
    pub const fn notional(&self) -> f64 {
        self.notional
    }

    /// Returns the side (long/receive or short/pay).
    #[must_use] 
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the evaluation strategy that defines how the raw payoff is computed.
    #[must_use] 
    pub const fn evaluation_strategy(&self) -> &ClaimEvaluationStrategy {
        &self.evaluation_strategy
    }

    /// Returns the market index for the underlying, if any.
    #[must_use] 
    pub const fn index(&self) -> Option<&MarketIndex> {
        self.index.as_ref()
    }

    /// Returns the flat-vector index assigned by the [`Inspector`](super::visitors::inspector::Inspector).
    #[must_use] 
    pub const fn idx(&self) -> Option<usize> {
        self.idx
    }

    /// Sets the flat-vector index used to locate this claim's
    /// [`SimulationResponse`] within a scenario step.
    pub const fn set_idx(&mut self, idx: usize) {
        self.idx = Some(idx);
    }

    /// Builds the simulation data requests needed to evaluate this claim.
    ///
    /// Discounting and reporting-currency conversion are handled by the context
    /// (discount policy / CSA). The claim only declares what market data it needs:
    /// currencies, forward rates, spot observations, or path-dependent observations.
    #[must_use] 
    pub fn simulation_request(&self) -> SimulationRequest {
        let fx_request = Some(self.foreign_currency.map_or_else(
            || FxRequest::single(self.currency),
            |quote| FxRequest::pair(self.currency, quote),
        ));

        match &self.evaluation_strategy {
            ClaimEvaluationStrategy::Deterministic { .. } => SimulationRequest {
                discount_request: None,
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
                    discount_request: None,
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
                    discount_request: None,
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
                    discount_request: None,
                    forward_rate_request: None,
                    fx_request,
                    spot_request: None,
                    path_dependent_request: path_request,
                }
            }

            ClaimEvaluationStrategy::ExerciseContingent { inner, .. } => {
                let inner_request = inner.simulation_request();
                SimulationRequest {
                    discount_request: None,
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
    ///
    /// # Errors
    /// Returns an error if the evaluation strategy fails to compute a value.
    pub fn evaluate<T: Scalar + 'static>(&self, response: &SimulationResponse<T>) -> Result<T> {
        let sign = self.side().sign();
        let notional = self.notional();
        let fx = response.fx_rates.unwrap_or_else(T::one);
        let discount = response.discounts.unwrap_or_else(T::one);

        let raw: T = match self.evaluation_strategy() {
            ClaimEvaluationStrategy::Deterministic { amount } => T::scalar(*amount),

            ClaimEvaluationStrategy::LinearRate {
                spread,
                day_counter,
            } => {
                let rate = response.forward_rates.unwrap_or_else(T::zero);
                let start = self.accrual_start().unwrap_or_else(|| self.payment_date());
                let end = self.accrual_end().unwrap_or_else(|| self.payment_date());
                let tau = day_counter.year_fraction(start, end);
                rate.add_val(T::scalar(*spread)).mul_val(T::scalar(tau))
            }

            ClaimEvaluationStrategy::NonLinearRate {
                payoff_ops,
                spread,
                strike: _,
                day_counter,
            } => {
                let rate = response.forward_rates.unwrap_or_else(T::zero);
                let start = self.accrual_start().unwrap_or_else(|| self.payment_date());
                let end = self.accrual_end().unwrap_or_else(|| self.payment_date());
                let tau = day_counter.year_fraction(start, end);
                let fixing = rate.add_val(T::scalar(*spread));
                let payoff = payoff_ops.eval(fixing).unwrap_or_else(|_| T::zero());
                payoff.mul_val(T::scalar(tau))
            }

            ClaimEvaluationStrategy::SpotPayoff {
                payoff_ops,
                strike: _,
                ..
            } => {
                let spot = response.spots.unwrap_or_else(T::zero);
                payoff_ops.eval(spot).unwrap_or_else(|_| T::zero())
            }

            ClaimEvaluationStrategy::PathDependent {
                payoff_ops,
                strike: _,
                ..
            } => {
                let obs = response.path_dependent_observations.unwrap_or_else(T::zero);
                payoff_ops.eval(obs).unwrap_or_else(|_| T::zero())
            }

            ClaimEvaluationStrategy::ExerciseContingent { inner, .. } => {
                inner.evaluate(response)?
            }
        };

        Ok(raw
            .mul_val(discount)
            .mul_val(fx)
            .mul_val(T::scalar(sign * notional)))
    }
}

impl Discountable for ContingentClaim {
    fn asset_class(&self) -> AssetClass {
        AssetClass::InterestRate
    }

    fn currency(&self) -> Currency {
        self.currency
    }

    fn discount_index(&self) -> Option<MarketIndex> {
        self.index.clone()
    }
}

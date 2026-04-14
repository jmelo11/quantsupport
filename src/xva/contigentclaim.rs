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
        visitors::{preprocessorexecutor::SimulationRequest, marketmodel::SimulationResponse},
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
/// * **Fixing info** — if the fixing is fully or partially realized at the
///   reference date, the resolved values are stored here so the simulation
///   does not need to recompute them.
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
    /// Fully realized fixing rate (set at indexing time when the fixing is
    /// entirely determined before the reference date).
    realized_fixing: Option<f64>,
    /// Partially realized compound accrual factor and original accrual start
    /// for in-arrears coupons whose accrual period straddles the reference date.
    partial_fixing: Option<PartialFixing>,
}

/// Information for an in-arrears coupon whose accrual period straddles the
/// reference date: the realized portion is locked in, the rest is forecast.
pub struct PartialFixing {
    /// Compound accrual factor for the realized portion: P₀(start)/P₀(ref_date).
    pub realized_accrual_factor: f64,
    /// Original accrual start date (before adjustment to ref_date).
    pub original_accrual_start: Date,
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
            realized_fixing: None,
            partial_fixing: None,
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

    /// Sets a fully realized fixing rate. When set, the claim will use this
    /// value instead of the simulated forward rate.
    pub fn set_realized_fixing(&mut self, rate: f64) {
        self.realized_fixing = Some(rate);
    }

    /// Returns the realized fixing, if set.
    #[must_use]
    pub const fn realized_fixing(&self) -> Option<f64> {
        self.realized_fixing
    }

    /// Sets partial fixing info for an in-arrears coupon whose accrual
    /// period straddles the reference date. Also adjusts `accrual_start`
    /// to `new_start` so the forward rate request covers only the remaining
    /// forecast period.
    pub fn set_partial_fixing(
        &mut self,
        realized_accrual_factor: f64,
        original_accrual_start: Date,
        new_start: Date,
    ) {
        self.partial_fixing = Some(PartialFixing {
            realized_accrual_factor,
            original_accrual_start,
        });
        self.accrual_start = Some(new_start);
    }

    /// Returns the partial fixing info, if set.
    #[must_use]
    pub const fn partial_fixing(&self) -> Option<&PartialFixing> {
        self.partial_fixing.as_ref()
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
                // If the fixing is fully realized, no forward rate simulation needed.
                let forward_request = if self.realized_fixing.is_some() {
                    None
                } else {
                    let forward_index = self.index.clone();
                    let fixing_date = self.fixing_date.unwrap_or(self.payment_date);
                    forward_index.map(|idx| {
                        ForwardRateRequest::new(idx, fixing_date)
                            .with_start_date(self.accrual_start.unwrap_or(fixing_date))
                            .with_end_date(self.accrual_end.unwrap_or(self.payment_date))
                    })
                };

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
                if let Some(rf) = self.realized_fixing {
                    // Fully realized: use the stored rate, no simulation
                    let start = self.accrual_start().unwrap_or_else(|| self.payment_date());
                    let end = self.accrual_end().unwrap_or_else(|| self.payment_date());
                    let tau = day_counter.year_fraction(start, end);
                    T::scalar((rf + spread) * tau)
                } else if let Some(pf) = &self.partial_fixing {
                    // Partially fixed in-arrears: combine realized accrual with forecast
                    let end = self.accrual_end().unwrap_or_else(|| self.payment_date());
                    let tau_full = day_counter.year_fraction(pf.original_accrual_start, end);
                    let start = self.accrual_start().unwrap_or_else(|| self.payment_date());
                    let tau_rem = day_counter.year_fraction(start, end);
                    let fwd = response.forward_rates.unwrap_or_else(T::zero);
                    // compound = raf * (1 + fwd * tau_rem)
                    let compound = T::scalar(pf.realized_accrual_factor)
                        .mul_val(T::one().add_val(fwd.mul_val(T::scalar(tau_rem))));
                    // rate = (compound - 1) / tau_full + spread
                    compound
                        .sub_val(T::one())
                        .div_val(T::scalar(tau_full))
                        .add_val(T::scalar(*spread))
                        .mul_val(T::scalar(tau_full))
                } else {
                    // Standard: use simulated forward rate
                    let rate = response.forward_rates.unwrap_or_else(T::zero);
                    let start = self.accrual_start().unwrap_or_else(|| self.payment_date());
                    let end = self.accrual_end().unwrap_or_else(|| self.payment_date());
                    let tau = day_counter.year_fraction(start, end);
                    rate.add_val(T::scalar(*spread)).mul_val(T::scalar(tau))
                }
            }

            ClaimEvaluationStrategy::NonLinearRate {
                payoff_ops,
                spread,
                strike: _,
                day_counter,
            } => {
                if let Some(rf) = self.realized_fixing {
                    // Fully realized
                    let start = self.accrual_start().unwrap_or_else(|| self.payment_date());
                    let end = self.accrual_end().unwrap_or_else(|| self.payment_date());
                    let tau = day_counter.year_fraction(start, end);
                    let fixing = T::scalar(rf + spread);
                    let payoff = payoff_ops.eval(fixing).unwrap_or_else(|_| T::zero());
                    payoff.mul_val(T::scalar(tau))
                } else if let Some(pf) = &self.partial_fixing {
                    // Partially fixed in-arrears
                    let end = self.accrual_end().unwrap_or_else(|| self.payment_date());
                    let tau_full = day_counter.year_fraction(pf.original_accrual_start, end);
                    let start = self.accrual_start().unwrap_or_else(|| self.payment_date());
                    let tau_rem = day_counter.year_fraction(start, end);
                    let fwd = response.forward_rates.unwrap_or_else(T::zero);
                    let compound = T::scalar(pf.realized_accrual_factor)
                        .mul_val(T::one().add_val(fwd.mul_val(T::scalar(tau_rem))));
                    let rate = compound
                        .sub_val(T::one())
                        .div_val(T::scalar(tau_full));
                    let fixing = rate.add_val(T::scalar(*spread));
                    let payoff = payoff_ops.eval(fixing).unwrap_or_else(|_| T::zero());
                    payoff.mul_val(T::scalar(tau_full))
                } else {
                    // Standard
                    let rate = response.forward_rates.unwrap_or_else(T::zero);
                    let start = self.accrual_start().unwrap_or_else(|| self.payment_date());
                    let end = self.accrual_end().unwrap_or_else(|| self.payment_date());
                    let tau = day_counter.year_fraction(start, end);
                    let fixing = rate.add_val(T::scalar(*spread));
                    let payoff = payoff_ops.eval(fixing).unwrap_or_else(|_| T::zero());
                    payoff.mul_val(T::scalar(tau))
                }
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

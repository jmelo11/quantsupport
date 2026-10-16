//! Loan valuation resolves existing contractual cashflows and projects optional
//! prepayment into a separate pricing state.

use crate::{
    ad::{dual::DualFwd, scalar::Scalar, tape::Tape},
    core::{
        collateral::DiscountPolicy,
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::{AssetClass, Instrument},
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            fixingrequest::FixingRequest,
            forwardraterequest::ForwardRateRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricer::Pricer,
        pricerstate::PricerState,
        request::{
            HandleCashflows, HandleFairRate, HandleSensitivities, HandleValue, LegsProvider,
            Request,
        },
        trade::Trade,
    },
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::{
            cashflow::{Cashflow, SimpleCashflow},
            cashflowtype::CashflowType,
            fixedratecoupon::FixedRateCoupon,
            floatingratecoupon::FloatingRateCoupon,
            leg::Leg,
        },
        fixedincome::{
            loan::{Loan, LoanTrade},
            makeloan::amortization_schedule,
        },
    },
    math::solvers::{
        bisection::Bisection,
        solvertraits::{ContFunc, SolutionStatus},
    },
    models::prepaymentmodelstate::PrepaymentModelState,
    rates::interestrate::InterestRate,
    time::{date::Date, daycounter::DayCounter, period::Period},
    utils::errors::{QSError, Result},
};
use std::collections::{HashMap, HashSet};

/// Prices the existing loan leg.
///
/// Configure a discount policy before valuation; its currency mapping selects
/// the curve independently of the contract.
/// Market changes leave contractual notionals fixed.
/// Prepayment and credit affect projected receipts only. `Request::Cashflows`
/// uses the shared handler; legs 0/1/2 represent scheduled receipts, prepayment,
/// and recovery, all using the library's existing cashflow variants.
#[derive(Default)]
pub struct LoanPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
    refinancing_index: Option<MarketIndex>,
    recompute_schedule: bool,
}
impl LoanPricer {
    /// Creates a pricer that preserves contractual amortization in fair-rate trials.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Configures the market rate observed by refinancing-dependent prepayment.
    #[must_use]
    pub fn with_refinancing_index(mut self, index: MarketIndex) -> Self {
        self.refinancing_index = Some(index);
        self
    }
    /// Controls `FairRate` trials: preserve existing principal, or regenerate
    /// the builder's repayment schedule for each candidate rate/spread.
    #[must_use]
    pub const fn with_recompute_schedule(mut self, enabled: bool) -> Self {
        self.recompute_schedule = enabled;
        self
    }

    fn discount_index(&self, loan: &Loan<DualFwd>) -> Result<MarketIndex> {
        self.discount_policy
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Loan discount policy".into()))?
            .accept(loan)
    }
    fn prepare_state(
        &self,
        trade: &LoanTrade<DualFwd>,
        ctx: &impl MarketDataProvider,
    ) -> Result<LoanState> {
        self.discount_index(trade.instrument())?;
        let request = self
            .market_data_request(trade)
            .ok_or_else(|| QSError::ValueNotSetErr("Loan refinancing benchmark or discount policy".into()))?;
        let needed_observations = trade
            .instrument()
            .leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| match cashflow {
                CashflowType::FixedRateCoupon(coupon) => Some(coupon.payment_date()),
                CashflowType::FloatingRateCoupon(coupon) => Some(coupon.payment_date()),
                _ => None,
            })
            .zip(trade.instrument().observations())
            .filter(|(payment, _)| *payment > ctx.evaluation_date())
            .filter_map(|(_, observation)| observation.as_ref())
            .map(|request| (request.market_index(), request.fixing_date()))
            .collect::<HashSet<_>>();
        let fixings = request
            .fixings_request()
            .into_iter()
            .flatten()
            .filter(|request| {
                request.date() <= ctx.evaluation_date()
                    && needed_observations
                        .contains(&(request.market_index().clone(), request.date()))
            })
            .map(|request| {
                let today = request.date() == ctx.evaluation_date();
                FixingRequest::new(request.market_index().clone(), request.date())
                    .with_missing_fixing_allowed(today)
            })
            .collect();
        let request = request.with_fixings_request(fixings);
        Ok(LoanState {
            market_data: Some(ctx.handle_request(&request)?),
            evaluation_date: ctx.evaluation_date(),
            value: None,
            legs: Vec::new(),
        })
    }
    fn forecast(state: &LoanState, request: &ForwardRateRequest) -> Result<DualFwd> {
        let index = request.market_index();
        if request.fixing_date() < state.evaluation_date {
            return state
                .get_fixing(&index, request.fixing_date())
                .map(DualFwd::scalar);
        }
        if request.fixing_date() == state.evaluation_date {
            if let Ok(value) = state.get_fixing(&index, request.fixing_date()) {
                return Ok(DualFwd::scalar(value));
            }
        }
        let start = request
            .start_date()
            .ok_or_else(|| QSError::ValueNotSetErr("Index observation start".into()))?;
        let end = request
            .end_date()
            .ok_or_else(|| QSError::ValueNotSetErr("Index observation end".into()))?;
        let definition = index.rate_index_details()?.rate_definition();
        let curve = state.get_discount_curve_element(&index)?.curve();
        let growth = curve
            .discount_factor(start)?
            .div_val(curve.discount_factor(end)?);
        Ok(InterestRate::<DualFwd>::implied_rate(
            growth,
            definition.day_counter(),
            definition.compounding(),
            definition.frequency(),
            definition.day_counter().year_fraction(start, end),
        )?
        .rate())
    }
    fn resolve(loan: &Loan<DualFwd>, state: &LoanState) -> Result<Vec<CashflowType<DualFwd>>> {
        let cashflows = loan.leg().cashflows().to_vec();
        let mut observations = loan.observations().iter();
        let mut resolved = HashMap::new();
        for cashflow in &cashflows {
            match cashflow {
                CashflowType::FixedRateCoupon(_) => {
                    observations.next();
                }
                CashflowType::FloatingRateCoupon(coupon) => {
                    let request = observations
                        .next()
                        .and_then(Option::as_ref)
                        .ok_or_else(|| QSError::ValueNotSetErr("Loan coupon observation".into()))?;
                    if coupon.payment_date() <= state.evaluation_date {
                        continue;
                    }
                    let key = (
                        request.market_index(),
                        request.fixing_date(),
                        request.start_date(),
                        request.end_date(),
                    );
                    let rate = if let Some(rate) = resolved.get(&key) {
                        *rate
                    } else {
                        let rate = Self::forecast(state, request)?;
                        resolved.insert(key, rate);
                        rate
                    };
                    coupon.set_fixing(rate);
                }
                _ => {}
            }
        }
        Ok(cashflows)
    }
    fn refinancing_rate(&self, state: &LoanState, date: Date) -> Result<DualFwd> {
        let index = self
            .refinancing_index
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Loan refinancing index".into()))?;
        let tenor = Period::from_frequency(index.rate_index_details()?.frequency())
            .ok_or_else(|| QSError::InvalidValueErr("Refinancing index frequency".into()))?;
        let request = ForwardRateRequest::new(index.clone(), date)
            .with_start_date(date)
            .with_end_date(date.advance(tenor.length(), tenor.units()));
        Self::forecast(state, &request)
    }

    /// Resolves coupons and projects expected receipts on cloned legs.
    #[allow(clippy::too_many_lines)]
    fn project(
        &self,
        trade: &LoanTrade<DualFwd>,
        state: &LoanState,
        cashflows: &[CashflowType<DualFwd>],
    ) -> Result<(DualFwd, Vec<Leg<DualFwd>>)> {
        let loan = trade.instrument();
        let curve = state.get_discount_curve_element(&self.discount_index(loan)?)?;
        let anchor = curve.curve().discount_factor(state.evaluation_date)?;
        let credit = loan
            .credit_index()
            .map(|index| state.get_credit_curve_element(index))
            .transpose()?;
        let credit_anchor = credit
            .map(|element| element.curve().discount_factor(state.evaluation_date))
            .transpose()?
            .unwrap_or_else(DualFwd::one);
        if credit_anchor.value() <= 0.0 || !credit_anchor.value().is_finite() {
            return Err(QSError::InvalidValueErr(
                "Invalid loan survival anchor".into(),
            ));
        }
        let mut previous_survival = credit
            .map(|element| {
                element
                    .curve()
                    .discount_factor(loan.start_date().max(state.evaluation_date))
            })
            .transpose()?
            .unwrap_or_else(DualFwd::one)
            .div_val(credit_anchor);
        let mut scheduled = Vec::new();
        let mut prepaid = Vec::new();
        let mut recovered = Vec::new();
        let mut factor = DualFwd::one();
        let mut value = DualFwd::zero();
        let principal_by_date = cashflows
            .iter()
            .filter_map(|cashflow| {
                if let CashflowType::Redemption(principal) = cashflow {
                    Some((principal.payment_date(), principal.amount()))
                } else {
                    None
                }
            })
            .try_fold(
                HashMap::<Date, f64>::new(),
                |mut amounts, (date, amount)| {
                    *amounts.entry(date).or_default() += amount?;
                    Ok::<_, QSError>(amounts)
                },
            )?;
        for cashflow in cashflows {
            if factor.value() == 0.0 {
                break;
            }
            let past = match cashflow {
                CashflowType::FixedRateCoupon(coupon) => {
                    coupon.payment_date() <= state.evaluation_date
                }
                CashflowType::FloatingRateCoupon(coupon) => {
                    coupon.payment_date() <= state.evaluation_date
                }
                _ => true,
            };
            if past {
                continue;
            }
            let (start, end, payment, opening, day_counter, rate, amount) = match cashflow {
                CashflowType::FixedRateCoupon(coupon) => (
                    coupon.accrual_start_date(),
                    coupon.accrual_end_date(),
                    coupon.payment_date(),
                    coupon.notional(),
                    coupon.rate().day_counter(),
                    coupon.rate().rate(),
                    coupon.amount()?,
                ),
                CashflowType::FloatingRateCoupon(coupon) => (
                    coupon.accrual_start_date(),
                    coupon.accrual_end_date(),
                    coupon.payment_date(),
                    coupon.notional(),
                    coupon.day_counter(),
                    coupon
                        .fixing()
                        .ok_or_else(|| QSError::ValueNotSetErr("Loan fixing".into()))?
                        .add_val(coupon.spread()),
                    coupon.amount()?,
                ),
                _ => continue,
            };
            let principal = principal_by_date.get(&payment).copied().unwrap_or(0.0);
            if payment <= state.evaluation_date {
                continue;
            }
            let remaining = (opening - principal).max(0.0);
            let survival = credit
                .map(|element| element.curve().discount_factor(payment))
                .transpose()?
                .unwrap_or_else(DualFwd::one)
                .div_val(credit_anchor);
            if !survival.value().is_finite()
                || survival.value() < 0.0
                || survival.value() > previous_survival.value() + 1e-10
            {
                return Err(QSError::InvalidValueErr(
                    "Loan survival must be nonnegative and nonincreasing".into(),
                ));
            }
            let probability = if remaining == 0.0 || end <= state.evaluation_date {
                DualFwd::zero()
            } else {
                let refinance = if loan.prepayment().requires_refinancing() {
                    self.refinancing_rate(state, start.max(state.evaluation_date))?
                } else {
                    DualFwd::zero()
                };
                let prepayment_start = start.max(state.evaluation_date);
                let period_fraction =
                    if prepayment_start.advance(1, crate::time::enums::TimeUnit::Months) == end {
                        1.0 / 12.0
                    } else {
                        DayCounter::Actual365.year_fraction(prepayment_start, end)
                    };
                loan.prepayment().probability(&PrepaymentModelState::new(
                    prepayment_start,
                    end,
                    period_fraction,
                    DualFwd::scalar(remaining).mul_val(factor),
                    rate,
                    refinance,
                ))?
            };
            if !probability.value().is_finite() || !(0.0..=1.0).contains(&probability.value()) {
                return Err(QSError::InvalidValueErr(
                    "Loan prepayment function must return a probability in [0, 1]".into(),
                ));
            }
            let weighted = factor.mul_val(survival);
            let interest = amount.mul_val(weighted);
            let redemption = DualFwd::scalar(principal).mul_val(weighted);
            let prepayment = DualFwd::scalar(remaining)
                .mul_val(probability)
                .mul_val(weighted);
            let recovery = previous_survival
                .sub_val(survival)
                .mul_val(DualFwd::scalar(opening * loan.recovery()))
                .mul_val(factor);
            let discount = curve.curve().discount_factor(payment)?.div_val(anchor);
            value = value.add_val(
                interest
                    .add_val(redemption)
                    .add_val(prepayment)
                    .add_val(recovery)
                    .mul_val(discount),
            );
            // Reporting cashflows retain existing variants and f64 notionals.
            // PV uses the AD weights above, including prepayment and credit risk.
            let reporting_notional = opening * weighted.value() * trade.side().sign();
            match cashflow {
                CashflowType::FixedRateCoupon(coupon) => {
                    scheduled.push(CashflowType::FixedRateCoupon(FixedRateCoupon::new(
                        reporting_notional,
                        Box::new(*coupon.rate()),
                        start,
                        end,
                        payment,
                    )));
                }
                CashflowType::FloatingRateCoupon(coupon) => {
                    let projected = FloatingRateCoupon::new(
                        reporting_notional,
                        coupon.spread(),
                        coupon.market_index().clone(),
                        start,
                        end,
                        payment,
                    )
                    .with_day_counter(day_counter);
                    projected.set_fixing(
                        coupon
                            .fixing()
                            .ok_or_else(|| QSError::ValueNotSetErr("Loan fixing".into()))?,
                    );
                    scheduled.push(CashflowType::FloatingRateCoupon(projected));
                }
                _ => {}
            }
            if redemption.value() != 0.0 {
                scheduled.push(CashflowType::Redemption(SimpleCashflow::new(
                    redemption.value() * trade.side().sign(),
                    payment,
                )));
            }
            if prepayment.value() != 0.0 {
                prepaid.push(CashflowType::Redemption(SimpleCashflow::new(
                    prepayment.value() * trade.side().sign(),
                    payment,
                )));
            }
            if recovery.value() != 0.0 {
                recovered.push(CashflowType::ConstantAmount(SimpleCashflow::new(
                    recovery.value() * trade.side().sign(),
                    payment,
                )));
            }
            factor = factor.mul_val(DualFwd::one().sub_val(probability));
            previous_survival = survival;
        }
        if !value.value().is_finite() {
            return Err(QSError::InvalidValueErr("Nonfinite loan value".into()));
        }
        let legs = [scheduled, prepaid, recovered]
            .into_iter()
            .enumerate()
            .map(|(id, cashflows)| {
                Leg::new(
                    id,
                    cashflows,
                    loan.currency(),
                    None,
                    None,
                    None,
                    None,
                    trade.side(),
                    true,
                    AssetClass::FixedIncome,
                    loan.start_date(),
                    loan.maturity_date(),
                )
            })
            .collect();
        Ok((value.mul_val(DualFwd::scalar(trade.side().sign())), legs))
    }
}

struct LoanState {
    market_data: Option<MarketData>,
    evaluation_date: Date,
    value: Option<DualFwd>,
    legs: Vec<Leg<DualFwd>>,
}
impl PricerState for LoanState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }
    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}
impl LegsProvider<DualFwd> for LoanState {
    fn legs(&self) -> &[Leg<DualFwd>] {
        &self.legs
    }
}
impl HandleCashflows<LoanTrade<DualFwd>, LoanState> for LoanPricer {}
impl HandleValue<LoanTrade<DualFwd>, LoanState> for LoanPricer {
    fn handle_value(&self, trade: &LoanTrade<DualFwd>, state: &mut LoanState) -> Result<f64> {
        if let Some(value) = state.value {
            return Ok(value.value());
        }
        Tape::start_recording_fwd();
        Tape::set_mark_fwd();
        let result = (|| {
            state.put_pillars_on_tape()?;
            let cashflows = Self::resolve(trade.instrument(), state)?;
            let (value, legs) = self.project(trade, state, &cashflows)?;
            state.value = Some(value);
            state.legs = legs;
            Ok(value.value())
        })();
        Tape::stop_recording_fwd();
        result
    }
}
impl HandleSensitivities<LoanTrade<DualFwd>, LoanState> for LoanPricer {
    /// Sums distinct pillar adjoints by quote label, counting shared nodes once.
    fn handle_sensitivities(
        &self,
        trade: &LoanTrade<DualFwd>,
        state: &mut LoanState,
    ) -> Result<SensitivityMap> {
        self.handle_value(trade, state)?;
        state
            .value
            .ok_or_else(|| QSError::ValueNotSetErr("Loan value".into()))?
            .backward_to_mark()?;
        let market_data = state
            .get_market_data_reponse()
            .ok_or_else(|| QSError::ValueNotSetErr("Loan market data".into()))?;
        let mut keys = Vec::new();
        let mut exposures = Vec::new();
        let mut positions = HashMap::new();
        let mut seen = HashSet::new();
        for (key, node, pillar) in market_data
            .constructed_elements()
            .discount_curves()
            .values()
            .flat_map(|element| {
                element
                    .curve()
                    .pillars()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(label, pillar)| {
                        (
                            label,
                            pillar.node_ptr(),
                            pillar.adjoint().map(|value| value.value()),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .chain(
                market_data
                    .constructed_elements()
                    .credit_curves()
                    .values()
                    .flat_map(|element| {
                        element
                            .curve()
                            .pillars()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(label, pillar)| {
                                (
                                    label,
                                    pillar.node_ptr(),
                                    pillar.adjoint().map(|value| value.value()),
                                )
                            })
                            .collect::<Vec<_>>()
                    }),
            )
        {
            if seen.insert((key.clone(), node)) {
                let exposure = pillar?;
                if let Some(&position) = positions.get(&key) {
                    exposures[position] += exposure;
                } else {
                    positions.insert(key.clone(), keys.len());
                    keys.push(key);
                    exposures.push(exposure);
                }
            }
        }
        Ok(SensitivityMap::default()
            .with_instrument_keys(&keys)
            .with_exposure(&exposures))
    }
}

struct LoanFairRate<'a> {
    pricer: &'a LoanPricer,
    trade: &'a LoanTrade<DualFwd>,
    state: &'a LoanState,
    cashflows: Vec<CashflowType<DualFwd>>,
    target: f64,
}
impl LoanFairRate<'_> {
    #[allow(clippy::too_many_lines)]
    fn trial_cashflows(&self, candidate: f64) -> Result<Vec<CashflowType<DualFwd>>> {
        let mut coupons = Vec::new();
        for cashflow in &self.cashflows {
            match cashflow {
                CashflowType::FixedRateCoupon(coupon) => {
                    coupons.push(CashflowType::FixedRateCoupon(FixedRateCoupon::new(
                        coupon.notional(),
                        Box::new(InterestRate::from_rate_definition(
                            DualFwd::scalar(candidate),
                            coupon.rate().rate_definition(),
                        )),
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        coupon.payment_date(),
                    )));
                }
                CashflowType::FloatingRateCoupon(coupon) => {
                    let trial = FloatingRateCoupon::new(
                        coupon.notional(),
                        DualFwd::scalar(candidate),
                        coupon.market_index().clone(),
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        coupon.payment_date(),
                    )
                    .with_day_counter(coupon.day_counter());
                    trial.set_fixing(
                        coupon.fixing().ok_or_else(|| {
                            QSError::ValueNotSetErr("Loan fair-rate fixing".into())
                        })?,
                    );
                    coupons.push(CashflowType::FloatingRateCoupon(trial));
                }
                _ => {}
            }
        }
        if !self.pricer.recompute_schedule {
            let mut coupons = coupons.into_iter();
            return self
                .cashflows
                .iter()
                .map(|cashflow| match cashflow {
                    CashflowType::FixedRateCoupon(_) | CashflowType::FloatingRateCoupon(_) => {
                        coupons
                            .next()
                            .ok_or_else(|| QSError::ValueNotSetErr("Loan trial coupon".into()))
                    }
                    _ => Ok(cashflow.clone()),
                })
                .collect();
        }
        let repayment = self.trade.instrument().repayment().ok_or_else(|| {
            QSError::InvalidValueErr(
                "Custom loan schedules require repayment metadata to recompute amortization".into(),
            )
        })?;
        let initial_rate = match coupons.first() {
            Some(CashflowType::FixedRateCoupon(coupon)) => coupon.rate().rate().value(),
            Some(CashflowType::FloatingRateCoupon(coupon)) => {
                coupon
                    .fixing()
                    .ok_or_else(|| QSError::ValueNotSetErr("Loan initial fixing".into()))?
                    .value()
                    + candidate
            }
            _ => return Err(QSError::ValueNotSetErr("Loan trial coupons".into())),
        };
        let period_rates = coupons
            .iter()
            .map(|cashflow| match cashflow {
                CashflowType::FixedRateCoupon(coupon) => {
                    initial_rate
                        * coupon
                            .rate()
                            .day_counter()
                            .year_fraction(coupon.accrual_start_date(), coupon.accrual_end_date())
                }
                CashflowType::FloatingRateCoupon(coupon) => {
                    initial_rate
                        * coupon
                            .day_counter()
                            .year_fraction(coupon.accrual_start_date(), coupon.accrual_end_date())
                }
                _ => 0.0,
            })
            .collect::<Vec<_>>();
        let notionals =
            amortization_schedule(self.trade.instrument().notional(), &period_rates, repayment)?;
        let mut trial = vec![CashflowType::Disbursement(SimpleCashflow::new(
            self.trade.instrument().notional(),
            self.trade.instrument().start_date(),
        ))];
        for (position, coupon) in coupons.iter().enumerate() {
            let (cashflow, payment) = match coupon {
                CashflowType::FixedRateCoupon(coupon) => (
                    CashflowType::FixedRateCoupon(FixedRateCoupon::new(
                        notionals[position],
                        Box::new(*coupon.rate()),
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        coupon.payment_date(),
                    )),
                    coupon.payment_date(),
                ),
                CashflowType::FloatingRateCoupon(coupon) => {
                    let rebound = FloatingRateCoupon::new(
                        notionals[position],
                        coupon.spread(),
                        coupon.market_index().clone(),
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        coupon.payment_date(),
                    )
                    .with_day_counter(coupon.day_counter());
                    rebound.set_fixing(
                        coupon
                            .fixing()
                            .ok_or_else(|| QSError::ValueNotSetErr("Loan trial fixing".into()))?,
                    );
                    (
                        CashflowType::FloatingRateCoupon(rebound),
                        coupon.payment_date(),
                    )
                }
                _ => return Err(QSError::InvalidValueErr("Expected loan coupon".into())),
            };
            trial.push(cashflow);
            let redemption =
                notionals[position] - notionals.get(position + 1).copied().unwrap_or(0.0);
            if redemption > 0.0 {
                trial.push(CashflowType::Redemption(SimpleCashflow::new(
                    redemption, payment,
                )));
            }
        }
        Ok(trial)
    }
}
impl ContFunc<f64> for LoanFairRate<'_> {
    fn call(&self, candidate: &f64) -> Result<f64> {
        let trial = self.trial_cashflows(*candidate)?;
        let (value, _) = self.pricer.project(self.trade, self.state, &trial)?;
        Ok(value.value() * self.trade.side().sign() / self.target - 1.0)
    }
}
impl HandleFairRate<LoanTrade<DualFwd>, LoanState> for LoanPricer {
    fn handle_fair_rate(&self, trade: &LoanTrade<DualFwd>, state: &mut LoanState) -> Result<f64> {
        if state.evaluation_date > trade.instrument().start_date() {
            return Err(QSError::InvalidValueErr(
                "Loan fair rate requires valuation at or before origination".into(),
            ));
        }
        Tape::stop_recording_fwd();
        let cashflows = Self::resolve(trade.instrument(), state)?;
        let fixed = cashflows
            .iter()
            .any(|cashflow| matches!(cashflow, CashflowType::FixedRateCoupon(_)));
        let floating = cashflows
            .iter()
            .any(|cashflow| matches!(cashflow, CashflowType::FloatingRateCoupon(_)));
        if fixed && floating {
            return Err(QSError::InvalidValueErr(
                "Mixed loans need a specified adjustable coupon segment".into(),
            ));
        }
        let curve = state.get_discount_curve_element(&self.discount_index(trade.instrument())?)?;
        let target = trade.instrument().notional()
            * curve
                .curve()
                .discount_factor(trade.instrument().start_date())?
                .value()
            / curve
                .curve()
                .discount_factor(state.evaluation_date)?
                .value();
        let objective = LoanFairRate {
            pricer: self,
            trade,
            state,
            cashflows,
            target,
        };
        let solution = Bisection::new(-0.05, 0.5, 100)
            .with_ftol(1e-11)
            .solve(&objective)?;
        if !matches!(solution.status, SolutionStatus::Converged) {
            return Err(QSError::SolverErr(
                "Loan fair rate did not converge in [-5%, 50%]".into(),
            ));
        }
        Ok(solution.x)
    }
}
impl Pricer for LoanPricer {
    type Item = LoanTrade<DualFwd>;
    type Policy = dyn DiscountPolicy;
    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let mut state = self.prepare_state(trade, ctx)?;
        let mut results =
            EvaluationResults::new(ctx.evaluation_date(), trade.instrument().identifier());
        for request in requests {
            results = match request {
                Request::Value => results.with_price(self.handle_value(trade, &mut state)?),
                Request::Sensitivities => {
                    results.with_sensitivities(self.handle_sensitivities(trade, &mut state)?)
                }
                Request::FairRate => {
                    results.with_fair_rate(self.handle_fair_rate(trade, &mut state)?)
                }
                Request::Cashflows => {
                    self.handle_value(trade, &mut state)?;
                    results.with_cashflows(self.handle_cashflows(trade, &mut state)?)
                }
                _ => {
                    return Err(QSError::InvalidValueErr(
                        "Unsupported loan result request".into(),
                    ));
                }
            };
        }
        Ok(results)
    }
    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest> {
        let loan = trade.instrument();
        let mut indices = vec![self.discount_index(loan).ok()?];
        indices.extend(
            loan.observations()
                .iter()
                .flatten()
                .map(ForwardRateRequest::market_index),
        );
        if loan.prepayment().requires_refinancing() {
            indices.push(self.refinancing_index.clone()?);
        }
        let mut seen = HashSet::new();
        let mut elements = indices
            .into_iter()
            .filter(|index| seen.insert(index.clone()))
            .map(|market_index| ConstructedElementRequest::DiscountCurve { market_index })
            .collect::<Vec<_>>();
        if let Some(index) = loan.credit_index() {
            elements.push(ConstructedElementRequest::CreditCurve {
                market_index: index.clone(),
            });
        }
        let mut seen_fixings = HashSet::new();
        let fixings = loan
            .observations()
            .iter()
            .flatten()
            .filter(|request| seen_fixings.insert((request.market_index(), request.fixing_date())))
            .map(|request| {
                FixingRequest::new(request.market_index(), request.fixing_date())
                    .with_missing_fixing_allowed(true)
            })
            .collect();
        Some(
            MarketDataRequest::default()
                .with_constructed_elements_request(elements)
                .with_fixings_request(fixings),
        )
    }
    fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
        self.discount_policy = Some(policy);
    }
    fn discount_policy(&self) -> Option<&Self::Policy> {
        self.discount_policy.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::elements::curveelement::{CreditCurveElement, DiscountCurveElement},
        prelude::*,
    };
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

    struct Provider {
        data: MarketData,
        date: Date,
    }
    impl MarketDataProvider for Provider {
        fn evaluation_date(&self) -> Date {
            self.date
        }
        fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData> {
            for fixing in request.fixings_request().into_iter().flatten() {
                assert!(fixing.date() <= self.date);
                assert_eq!(fixing.missing_fixing_allowed(), fixing.date() == self.date);
                if !fixing.missing_fixing_allowed()
                    && self
                        .data
                        .fixings()
                        .get(fixing.market_index())
                        .and_then(|dates| dates.get(&fixing.date()))
                        .is_none()
                {
                    return Err(QSError::NotFoundErr("Required historical fixing".into()));
                }
            }
            Ok(self.data.clone())
        }
    }
    fn market(discount: f64, forward: f64, hazard: f64) -> Provider {
        let date = Date::new(2026, 1, 1);
        let mut elements = ConstructedElementStore::default();
        for (index, rate, label) in [
            (MarketIndex::ESTR, discount, "DISCOUNT"),
            (MarketIndex::EURIBOR12m, forward, "FORWARD"),
        ] {
            let curve = FlatForwardTermStructure::new(
                date,
                DualFwd::scalar(rate),
                RateDefinition::new(
                    DayCounter::Actual365,
                    Compounding::Continuous,
                    Frequency::Annual,
                ),
            )
            .with_pillar_label(label.into());
            elements.discount_curves_mut().insert(
                index.clone(),
                DiscountCurveElement::new(index, Rc::new(RefCell::new(curve))),
            );
        }
        let curve = FlatForwardTermStructure::new(
            date,
            DualFwd::scalar(hazard),
            RateDefinition::new(
                DayCounter::Actual365,
                Compounding::Continuous,
                Frequency::Annual,
            ),
        )
        .with_pillar_label("CREDIT".into());
        let index = MarketIndex::Credit("BORROWER".into());
        elements.credit_curves_mut().insert(
            index.clone(),
            CreditCurveElement::new(index, 0.4, Rc::new(RefCell::new(curve))),
        );
        let fixings = HashMap::from([(
            MarketIndex::EURIBOR12m,
            BTreeMap::from([(Date::new(2025, 12, 30), 0.04)]),
        )]);
        Provider {
            data: MarketData::new(fixings, elements),
            date,
        }
    }
    fn estr_loan_pricer() -> LoanPricer {
        let mut pricer = LoanPricer::new();
        pricer.set_discount_policy(Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_risk_free_index(Currency::EUR, MarketIndex::ESTR),
        ));
        pricer
    }
    #[test]
    fn discounting_requires_a_policy_and_can_change_without_changing_the_contract() -> Result<()> {
        let provider = market(0.03, 0.04, 0.0);
        let loan = builder().build()?;
        assert!(loan.discount_index().is_none());
        assert!(loan.leg().discount_index().is_none());
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let error = LoanPricer::new()
            .evaluate(&trade, &[Request::Value], &provider)
            .err()
            .ok_or_else(|| QSError::ValueNotSetErr("Expected missing policy error".into()))?;
        assert!(error.to_string().contains("Loan discount policy"));
        let mut pricer = estr_loan_pricer();
        let requests = [Request::Value, Request::Cashflows];
        let estr = pricer.evaluate(&trade, &requests, &provider)?;
        pricer.set_discount_policy(Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_risk_free_index(Currency::EUR, MarketIndex::EURIBOR12m),
        ));
        let euribor = pricer.evaluate(&trade, &requests, &provider)?;
        assert!(euribor.price() < estr.price());
        assert_eq!(
            estr.cashflows().map(|table| table.amounts()),
            euribor.cashflows().map(|table| table.amounts())
        );
        assert!(trade
            .instrument()
            .leg()
            .discount_index()
            .is_none());
        Ok(())
    }
    fn builder() -> MakeLoan<DualFwd> {
        MakeLoan::default()
            .with_identifier("LOAN".into())
            .with_start_date(Date::new(2026, 1, 1))
            .with_maturity_date(Date::new(2028, 1, 1))
            .with_notional(100_000.0)
            .with_currency(Currency::EUR)
            .with_rate(LoanRateDefinition::Fixed(0.06))
            .with_day_counter(DayCounter::Actual360)
    }
    fn value(pricer: &LoanPricer, trade: &LoanTrade<DualFwd>, market: &Provider) -> Result<f64> {
        pricer
            .evaluate(trade, &[Request::Value], market)?
            .price()
            .ok_or_else(|| QSError::ValueNotSetErr("Price".into()))
    }
    #[test]
    fn shared_cashflow_request_matches_pv_and_request_order_is_stable() -> Result<()> {
        let provider = market(0.03, 0.04, 0.0);
        let loan = builder()
            .with_prepayment(PrepaymentModelConfiguration::Cpr(0.25))
            .build()?;
        let original = loan.leg().cashflows().len();
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let pricer = estr_loan_pricer();
        let result = pricer.evaluate(
            &trade,
            &[
                Request::Cashflows,
                Request::Cashflows,
                Request::FairRate,
                Request::Sensitivities,
                Request::Value,
            ],
            &provider,
        )?;
        let table = result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Cashflows".into()))?;
        let expected = table
            .payment_dates()
            .iter()
            .zip(table.amounts())
            .map(|(date, amount)| {
                amount * (-0.03 * DayCounter::Actual365.year_fraction(provider.date, *date)).exp()
            })
            .sum::<f64>();
        assert!((expected - result.price().unwrap_or_default()).abs() < 1e-7);
        assert!(table
            .cashflow_types()
            .iter()
            .all(|kind| ["FixedRateCoupon", "Redemption"].contains(&kind.as_str())));
        assert!(table.leg_indices().contains(&1));
        let principal: f64 = table
            .amounts()
            .iter()
            .zip(table.cashflow_types())
            .filter(|(_, kind)| kind.as_str() == "Redemption")
            .map(|(amount, _)| amount)
            .sum();
        assert!((principal - 100_000.0).abs() < 1e-7);
        assert_eq!(trade.instrument().leg().cashflows().len(), original);
        let short = LoanTrade::new(trade.instrument().clone(), provider.date, Side::PayShort);
        assert!((value(&pricer, &short, &provider)? + expected).abs() < 1e-7);
        Ok(())
    }
    #[test]
    fn floating_market_changes_preserve_principal_and_exact_reset_fixings() -> Result<()> {
        let loan = builder()
            .with_rate(LoanRateDefinition::Floating {
                index: MarketIndex::EURIBOR12m,
                spread: 0.01,
            })
            .with_amortization_rate(0.05)
            .with_calendar(Calendar::TARGET(
                crate::time::calendars::target::TARGET::new(),
            ))
            .with_fixing_lag(2)
            .build()?;
        let trade = LoanTrade::new(loan, Date::new(2026, 1, 1), Side::LongReceive);
        let pricer = estr_loan_pricer();
        let mut redemptions = Vec::new();
        for rate in [0.02, 0.06] {
            let result =
                pricer.evaluate(&trade, &[Request::Cashflows], &market(0.03, rate, 0.0))?;
            let table = result
                .cashflows()
                .ok_or_else(|| QSError::ValueNotSetErr("Cashflows".into()))?;
            redemptions.push(
                table
                    .amounts()
                    .iter()
                    .zip(table.cashflow_types())
                    .filter(|(_, kind)| kind.as_str() == "Redemption")
                    .map(|(amount, _)| *amount)
                    .collect::<Vec<_>>(),
            );
            assert!(table
                .fixing()
                .iter()
                .take(12 * 2)
                .flatten()
                .all(|fixing| (*fixing - 0.04).abs() < 1e-14));
        }
        assert_eq!(redemptions[0], redemptions[1]);
        let mut missing = market(0.03, 0.04, 0.0);
        missing.data = MarketData::new(HashMap::new(), missing.data.constructed_elements().clone());
        assert!(pricer
            .evaluate(&trade, &[Request::Value], &missing)
            .is_err());
        Ok(())
    }
    #[test]
    fn seasoned_cashflows_request_only_observations_needed_by_future_coupons() -> Result<()> {
        let loan = builder()
            .with_rate(LoanRateDefinition::Floating {
                index: MarketIndex::EURIBOR12m,
                spread: 0.01,
            })
            .with_amortization_rate(0.05)
            .build()?;
        let mut provider = market(0.03, 0.04, 0.0);
        provider.date = Date::new(2027, 1, 1);
        provider.data =
            MarketData::new(HashMap::new(), provider.data.constructed_elements().clone());
        let trade = LoanTrade::new(loan, Date::new(2026, 1, 1), Side::LongReceive);
        let pricer = estr_loan_pricer();
        // The expired first-year reset is absent; today's unpublished fixing may forecast.
        let result = pricer.evaluate(&trade, &[Request::Value, Request::Cashflows], &provider)?;
        let table = result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Seasoned cashflows".into()))?;
        assert!(table
            .payment_dates()
            .iter()
            .all(|date| *date > provider.date));
        assert_eq!(
            table
                .cashflow_types()
                .iter()
                .filter(|kind| kind.as_str() == "FloatingRateCoupon")
                .count(),
            12
        );
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fair_rate_preserves_or_recomputes_existing_schedule() -> Result<()> {
        let provider = market(0.03, 0.04, 0.0);
        for floating in [false, true] {
            let rate = if floating {
                LoanRateDefinition::Floating {
                    index: MarketIndex::EURIBOR12m,
                    spread: 0.01,
                }
            } else {
                LoanRateDefinition::Fixed(0.09)
            };
            let original_rate = if floating { 0.05 } else { 0.09 };
            let original_builder = builder()
                .with_rate(rate)
                .with_amortization_rate(original_rate);
            let trade = LoanTrade::new(
                original_builder.clone().build()?,
                provider.date,
                Side::LongReceive,
            );
            let snapshot = trade
                .instrument()
                .leg()
                .cashflows()
                .iter()
                .filter_map(|cashflow| {
                    if let CashflowType::Redemption(principal) = cashflow {
                        principal.amount().ok()
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            for rebuild in [false, true] {
                let pricer = estr_loan_pricer().with_recompute_schedule(rebuild);
                let fair = pricer
                    .evaluate(&trade, &[Request::FairRate], &provider)?
                    .fair_rate()
                    .ok_or_else(|| QSError::ValueNotSetErr("Fair rate".into()))?;
                let rate = if floating {
                    LoanRateDefinition::Floating {
                        index: MarketIndex::EURIBOR12m,
                        spread: fair,
                    }
                } else {
                    LoanRateDefinition::Fixed(fair)
                };
                let initial_index = 0.04_f64.exp_m1() / (365.0 / 360.0);
                let amortization_rate = if rebuild {
                    if floating {
                        initial_index + fair
                    } else {
                        fair
                    }
                } else {
                    original_rate
                };
                let candidate = LoanTrade::new(
                    original_builder
                        .clone()
                        .with_rate(rate)
                        .with_amortization_rate(amortization_rate)
                        .build()?,
                    provider.date,
                    Side::LongReceive,
                );
                assert!((value(&pricer, &candidate, &provider)? - 100_000.0).abs() < 1e-5);
                let candidate_schedule = candidate
                    .instrument()
                    .leg()
                    .cashflows()
                    .iter()
                    .filter_map(|cashflow| {
                        if let CashflowType::Redemption(principal) = cashflow {
                            principal.amount().ok()
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if rebuild {
                    assert_ne!(snapshot, candidate_schedule);
                } else {
                    assert_eq!(snapshot, candidate_schedule);
                }
            }
            let after = trade
                .instrument()
                .leg()
                .cashflows()
                .iter()
                .filter_map(|cashflow| {
                    if let CashflowType::Redemption(principal) = cashflow {
                        principal.amount().ok()
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(snapshot, after);
        }
        Ok(())
    }
    #[test]
    fn custom_prepayment_function_is_used_and_invalid_probabilities_fail() -> Result<()> {
        let loan = builder()
            .with_prepayment_function(|_: &PrepaymentModelState<DualFwd>| Ok(DualFwd::one()))
            .build()?;
        let provider = market(0.03, 0.04, 0.0);
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let result = estr_loan_pricer().evaluate(&trade, &[Request::Cashflows], &provider)?;
        let table = result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Cashflows".into()))?;
        assert_eq!(
            table
                .payment_dates()
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len(),
            1
        );
        let invalid = builder()
            .with_prepayment_function(|_: &PrepaymentModelState<DualFwd>| Ok(DualFwd::scalar(1.1)))
            .build()?;
        assert!(estr_loan_pricer()
            .evaluate(
                &LoanTrade::new(invalid, provider.date, Side::LongReceive),
                &[Request::Cashflows],
                &provider
            )
            .is_err());
        Ok(())
    }
    #[test]
    fn discount_forward_credit_and_refinancing_risk_match_bumps() -> Result<()> {
        let loan = builder()
            .with_rate(LoanRateDefinition::Floating {
                index: MarketIndex::EURIBOR12m,
                spread: 0.02,
            })
            .with_amortization_rate(0.06)
            .with_credit_index(MarketIndex::Credit("BORROWER".into()))
            .with_prepayment(PrepaymentModelConfiguration::Refinancing {
                base_cpr: 0.02,
                incentive_multiplier: 4.0,
                maximum_cpr: 0.4,
            })
            .build()?;
        let provider = market(0.03, 0.04, 0.01);
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let pricer = estr_loan_pricer().with_refinancing_index(MarketIndex::EURIBOR12m);
        let result = pricer.evaluate(&trade, &[Request::Sensitivities], &provider)?;
        let risk = result
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Risk".into()))?;
        for (label, derivative) in risk.instrument_keys().iter().zip(risk.exposure()) {
            let bump = 1e-6;
            let up = market(
                0.03 + if label == "DISCOUNT" { bump } else { 0.0 },
                0.04 + if label == "FORWARD" { bump } else { 0.0 },
                0.01 + if label == "CREDIT" { bump } else { 0.0 },
            );
            let down = market(
                0.03 - if label == "DISCOUNT" { bump } else { 0.0 },
                0.04 - if label == "FORWARD" { bump } else { 0.0 },
                0.01 - if label == "CREDIT" { bump } else { 0.0 },
            );
            let fd = (value(&pricer, &trade, &up)? - value(&pricer, &trade, &down)?) / (2.0 * bump);
            assert!(
                (fd - derivative).abs() < derivative.abs().mul_add(1e-5, 0.1),
                "{label}: AD {derivative}, FD {fd}"
            );
        }
        Ok(())
    }
}

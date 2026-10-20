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
            optionembeddedcoupon::OptionEmbeddedCoupon,
        },
        fixedincome::{
            loan::{Loan, LoanTrade},
            makeloan::calculate_contractual_opening_balances,
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
    refinancing_benchmark_index: Option<MarketIndex>,
    rebuild_amortization_for_fair_rate: bool,
}
impl LoanPricer {
    /// Creates a pricer that preserves contractual amortization in fair-rate trials.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Selects the index benchmark used by refinancing-incentive prepayment.
    ///
    /// Required when the loan's prepayment model requests refinancing data.
    /// The index metadata supplies the observation tenor and quote convention;
    /// this index does not replace the contractual coupon's floating index.
    /// The observed benchmark excludes replacement-loan spreads and costs.
    #[must_use]
    pub fn with_refinancing_benchmark_index(mut self, index: MarketIndex) -> Self {
        self.refinancing_benchmark_index = Some(index);
        self
    }

    /// Controls `FairRate` trials: preserve existing principal, or regenerate
    /// the builder's repayment schedule for each candidate rate/spread.
    ///
    /// Defaults to `false`. Rebuilding requires repayment metadata and uses
    /// the first trial coupon's all-in rate as the initial amortization rate.
    /// Once solved, that first rate remains fixed during later segment solves.
    /// Neither mode changes the instrument
    /// or the principal schedule used by `Value` and `Sensitivities`.
    #[must_use]
    pub const fn with_fair_rate_amortization_rebuild(mut self, enabled: bool) -> Self {
        self.rebuild_amortization_for_fair_rate = enabled;
        self
    }

    /// Selects the discount curve exclusively through the configured policy.
    ///
    /// # Errors
    /// Rejects an unconfigured policy and propagates policy resolution errors.
    fn select_discount_curve_index(&self, loan: &Loan<DualFwd>) -> Result<MarketIndex> {
        self.discount_policy
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Loan discount policy".into()))?
            .accept(loan)
    }

    /// Loads curves and historical resets needed by unpaid coupons into a new state.
    ///
    /// Past fixing dates require stored observations. A fixing on the evaluation
    /// date may be absent and forecast instead; already paid coupons do not
    /// require historical fixings. Future rates are read from projection curves.
    ///
    /// # Errors
    /// Rejects missing discount policies/refinancing indices and propagates market-data
    /// errors, including missing historical fixings for unpaid coupons.
    fn load_loan_valuation_state(
        &self,
        trade: &LoanTrade<DualFwd>,
        ctx: &impl MarketDataProvider,
    ) -> Result<LoanValuationState> {
        self.select_discount_curve_index(trade.instrument())?;
        let request = self.market_data_request(trade).ok_or_else(|| {
            QSError::ValueNotSetErr("Loan refinancing benchmark or discount policy".into())
        })?;
        let needed_observations = trade
            .instrument()
            .contractual_leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| match cashflow {
                CashflowType::FixedRateCoupon(coupon) => Some(coupon.payment_date()),
                CashflowType::FloatingRateCoupon(coupon) => Some(coupon.payment_date()),
                CashflowType::OptionEmbeddedCoupon(coupon) => Some(coupon.payment_date()),
                _ => None,
            })
            .zip(trade.instrument().coupon_rate_observations())
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
        Ok(LoanValuationState {
            market_data: Some(ctx.handle_request(&request)?),
            evaluation_date: ctx.evaluation_date(),
            value: None,
            legs: Vec::new(),
        })
    }

    /// Resolves an index observation with its own tenor and quote convention.
    ///
    /// Historical dates use exact fixings. Today's fixing takes precedence over
    /// a forecast; future observations use the index curve's discount-factor
    /// ratio over the requested start/end dates. Stored fixings have no AD risk.
    ///
    /// # Errors
    /// Propagates missing historical fixings, incomplete observation dates,
    /// unavailable index curves/conventions, and implied-rate conversion errors.
    fn resolve_index_rate_observation(
        state: &LoanValuationState,
        request: &ForwardRateRequest,
    ) -> Result<DualFwd> {
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

    /// Clones the contractual cashflows and installs fixings on unpaid coupons.
    ///
    /// Coupons sharing an index, fixing date, and observation interval reuse one
    /// resolved rate. The loan's cashflows, notionals, and observations are kept.
    ///
    /// # Errors
    /// Rejects missing floating observation entries and propagates forecast errors.
    fn clone_cashflows_with_resolved_fixings(
        loan: &Loan<DualFwd>,
        state: &LoanValuationState,
    ) -> Result<Vec<CashflowType<DualFwd>>> {
        let mut cashflows = loan.contractual_leg().cashflows().to_vec();
        let mut observations = loan.coupon_rate_observations().iter();
        let mut resolved = HashMap::new();
        for cashflow in &mut cashflows {
            let payment_date = match cashflow {
                CashflowType::FixedRateCoupon(_) => {
                    observations.next();
                    continue;
                }
                CashflowType::FloatingRateCoupon(coupon) => coupon.payment_date(),
                CashflowType::OptionEmbeddedCoupon(coupon) => coupon.payment_date(),
                _ => continue,
            };
            let request = observations
                .next()
                .and_then(Option::as_ref)
                .ok_or_else(|| QSError::ValueNotSetErr("Loan coupon observation".into()))?;
            if payment_date <= state.evaluation_date {
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
                let rate = Self::resolve_index_rate_observation(state, request)?;
                resolved.insert(key, rate);
                rate
            };
            match cashflow {
                CashflowType::FloatingRateCoupon(coupon) => coupon.set_fixing(rate),
                CashflowType::OptionEmbeddedCoupon(coupon) => coupon.set_fixing(rate),
                _ => {}
            }
        }
        Ok(cashflows)
    }

    /// Resolves the configured refinancing benchmark from `date` over its index tenor.
    ///
    /// This is an index observation used for the prepayment incentive, rather
    /// than an all-in replacement-loan quote; spreads and fees are excluded.
    ///
    /// # Errors
    /// Rejects an unset refinancing index or unsupported index frequency, and
    /// propagates observation-resolution errors.
    fn observe_refinancing_benchmark_rate(
        &self,
        state: &LoanValuationState,
        date: Date,
    ) -> Result<DualFwd> {
        let index = self
            .refinancing_benchmark_index
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Loan refinancing benchmark index".into()))?;
        let tenor =
            Period::from_frequency(index.rate_index_details()?.frequency()).ok_or_else(|| {
                QSError::InvalidValueErr("Refinancing benchmark index frequency".into())
            })?;
        let request = ForwardRateRequest::new(index.clone(), date)
            .with_start_date(date)
            .with_end_date(date.advance(tenor.length(), tenor.units()));
        Self::resolve_index_rate_observation(state, &request)
    }

    /// Projects expected receipts from resolved coupons into separate reporting legs.
    ///
    /// Processes resolved coupons strictly after the evaluation date. Scheduled
    /// principal is paid before conditional prepayment; default recovery uses
    /// opening principal and is paid at the period's payment date. Survival is
    /// conditional on the evaluation date. A full monthly prepayment interval
    /// uses 1/12 year; partial and irregular intervals use Actual/365.
    ///
    /// Returns signed receipt PV, excluding the initial advance, and reporting
    /// legs 0/1/2 for scheduled receipts, prepayments, and recoveries. PV retains
    /// AD dependencies in survival and prepayment weights; reporting notionals
    /// and principal amounts are `f64` values, with direction carried by each leg.
    ///
    /// # Errors
    /// Propagates unavailable rates/curves, rejects invalid survival or
    /// prepayment probabilities, and rejects a nonfinite projected value.
    #[allow(clippy::too_many_lines)]
    fn calculate_expected_receipts_and_present_value(
        &self,
        trade: &LoanTrade<DualFwd>,
        state: &LoanValuationState,
        cashflows: &[CashflowType<DualFwd>],
    ) -> Result<(DualFwd, Vec<Leg<DualFwd>>)> {
        let loan = trade.instrument();
        let curve = state.get_discount_curve_element(&self.select_discount_curve_index(loan)?)?;
        let anchor = curve.curve().discount_factor(state.evaluation_date)?;
        let credit = loan
            .credit_curve_index()
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
                CashflowType::OptionEmbeddedCoupon(coupon) => {
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
                CashflowType::OptionEmbeddedCoupon(coupon) => {
                    let fixing = coupon
                        .fixing()
                        .ok_or_else(|| QSError::ValueNotSetErr("Loan fixing".into()))?;
                    (
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        coupon.payment_date(),
                        coupon.notional(),
                        coupon.day_counter()?,
                        coupon.payoff_ops().eval(fixing)?.add_val(coupon.spread()),
                        coupon.amount()?,
                    )
                }
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
                let refinancing_benchmark_rate =
                    if loan.prepayment_model().requires_refinancing_benchmark() {
                        self.observe_refinancing_benchmark_rate(
                            state,
                            start.max(state.evaluation_date),
                        )?
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
                loan.prepayment_model().conditional_prepayment_probability(
                    &PrepaymentModelState::new(
                        prepayment_start,
                        end,
                        period_fraction,
                        DualFwd::scalar(remaining).mul_val(factor),
                        rate,
                        refinancing_benchmark_rate,
                    ),
                )?
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
                .mul_val(DualFwd::scalar(opening * loan.recovery_fraction()))
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
            // The shared report handler applies the reporting legs' side once.
            // PV uses the AD weights above, including prepayment and credit risk.
            let reporting_notional = opening * weighted.value();
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
                CashflowType::OptionEmbeddedCoupon(coupon) => {
                    scheduled.push(CashflowType::OptionEmbeddedCoupon(
                        OptionEmbeddedCoupon::new(
                            reporting_notional,
                            coupon.market_index().clone(),
                            coupon.spread(),
                            start,
                            end,
                            payment,
                            coupon.payoff_ops().clone(),
                        )
                        .with_day_counter(day_counter)
                        .with_fixing(
                            coupon
                                .fixing()
                                .ok_or_else(|| QSError::ValueNotSetErr("Loan fixing".into()))?,
                        ),
                    ));
                }
                _ => {}
            }
            if redemption.value() != 0.0 {
                scheduled.push(CashflowType::Redemption(SimpleCashflow::new(
                    redemption.value(),
                    payment,
                )));
            }
            if prepayment.value() != 0.0 {
                prepaid.push(CashflowType::Redemption(SimpleCashflow::new(
                    prepayment.value(),
                    payment,
                )));
            }
            if recovery.value() != 0.0 {
                recovered.push(CashflowType::ConstantAmount(SimpleCashflow::new(
                    recovery.value(),
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

/// Market observations and cached expected receipts for one evaluation.
///
/// The value retains the AD graph used by sensitivities; the legs supply the
/// shared cashflow-reporting handler without modifying the contractual loan.
struct LoanValuationState {
    market_data: Option<MarketData>,
    evaluation_date: Date,
    value: Option<DualFwd>,
    legs: Vec<Leg<DualFwd>>,
}

impl PricerState for LoanValuationState {
    /// Returns the market data loaded for this evaluation, if available.
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }
    /// Allows shared state handlers to update the loaded market data.
    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl LegsProvider<DualFwd> for LoanValuationState {
    /// Returns projected scheduled, prepayment, and recovery reporting legs.
    fn legs(&self) -> &[Leg<DualFwd>] {
        &self.legs
    }
}

impl HandleCashflows<LoanTrade<DualFwd>, LoanValuationState> for LoanPricer {}

impl HandleValue<LoanTrade<DualFwd>, LoanValuationState> for LoanPricer {
    /// Computes signed receipt PV once and caches the value and reporting legs.
    ///
    /// Records market pillars and projection operations on the forward AD tape
    /// so subsequent sensitivity requests can reuse the cached graph. Recording
    /// is stopped after the computation, including when it returns an error.
    ///
    /// # Errors
    /// Propagates missing pillars, observation-resolution, and projection errors.
    fn handle_value(
        &self,
        trade: &LoanTrade<DualFwd>,
        state: &mut LoanValuationState,
    ) -> Result<f64> {
        if let Some(value) = state.value {
            return Ok(value.value());
        }
        Tape::start_recording_fwd();
        Tape::set_mark_fwd();
        let result = (|| {
            state.put_pillars_on_tape()?;
            let cashflows = Self::clone_cashflows_with_resolved_fixings(trade.instrument(), state)?;
            let (value, legs) =
                self.calculate_expected_receipts_and_present_value(trade, state, &cashflows)?;
            state.value = Some(value);
            state.legs = legs;
            Ok(value.value())
        })();
        Tape::stop_recording_fwd();
        result
    }
}

impl HandleSensitivities<LoanTrade<DualFwd>, LoanValuationState> for LoanPricer {
    /// Differentiates cached receipt PV with respect to labelled market pillars.
    ///
    /// Shared pillar nodes are counted once; distinct nodes with the same quote
    /// label contribute to one exposure. Exposures
    /// are raw derivatives per unit pillar change, including differentiable
    /// prepayment and survival weights; contractual amortization stays fixed.
    ///
    /// # Errors
    /// Propagates valuation, AD backpropagation, and missing pillar-adjoint errors.
    fn handle_sensitivities(
        &self,
        trade: &LoanTrade<DualFwd>,
        state: &mut LoanValuationState,
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

/// Contiguous contractual coupon level and its hypothetical redemption date.
struct LoanCouponSegment {
    coupons: std::ops::Range<usize>,
    redemption_date: Date,
}

/// Fair-rate objective holding resolved coupons and the discounted advance target.
struct LoanFairRateObjective<'a> {
    pricer: &'a LoanPricer,
    trade: &'a LoanTrade<DualFwd>,
    state: &'a LoanValuationState,
    cashflows: &'a [CashflowType<DualFwd>],
    adjustable_segment: std::ops::Range<usize>,
    redemption_date: Date,
    target: f64,
}

impl LoanFairRateObjective<'_> {
    /// Limits the upper trial rate to schedules with admissible amortization.
    ///
    /// Long loans with unequal accrual periods can amortize negatively at high
    /// trial rates even though their par rate has a valid principal schedule.
    ///
    /// # Errors
    /// Propagates trial construction errors at the lower search bound.
    fn admissible_upper_bound(&self, lower: f64, upper: f64) -> Result<f64> {
        if !self.pricer.rebuild_amortization_for_fair_rate {
            return Ok(upper);
        }
        self.build_cashflows_for_trial_coupon_or_spread(lower)?;
        if self
            .build_cashflows_for_trial_coupon_or_spread(upper)
            .is_ok()
        {
            return Ok(upper);
        }
        let mut valid = lower;
        let mut invalid = upper;
        for _ in 0..48 {
            let candidate = f64::midpoint(valid, invalid);
            if self
                .build_cashflows_for_trial_coupon_or_spread(candidate)
                .is_ok()
            {
                valid = candidate;
            } else {
                invalid = candidate;
            }
        }
        Ok(valid)
    }

    /// Creates trial coupons for a candidate fixed annual rate or floating spread.
    ///
    /// Only coupons in the selected segment change; other coupon rates and all
    /// index fixings are retained. By default, principal cashflows and
    /// coupon notionals are preserved; recompute mode regenerates amortization
    /// from repayment metadata and the first trial coupon's all-in rate.
    ///
    /// # Errors
    /// Rejects missing fixings/coupons, missing repayment metadata in recompute
    /// mode, and invalid trial amortization rates or principal schedules.
    #[allow(clippy::too_many_lines)]
    fn build_cashflows_for_trial_coupon_or_spread(
        &self,
        candidate: f64,
    ) -> Result<Vec<CashflowType<DualFwd>>> {
        let mut coupons = Vec::new();
        let mut position = 0;
        for cashflow in self.cashflows {
            let selected = self.adjustable_segment.contains(&position);
            if matches!(
                cashflow,
                CashflowType::FixedRateCoupon(_) | CashflowType::FloatingRateCoupon(_)
            ) {
                position += 1;
            }
            if !selected {
                if matches!(
                    cashflow,
                    CashflowType::FixedRateCoupon(_) | CashflowType::FloatingRateCoupon(_)
                ) {
                    coupons.push(cashflow.clone());
                }
                continue;
            }
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
        if !self.pricer.rebuild_amortization_for_fair_rate {
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
        let repayment =
            self.trade
                .instrument()
                .repayment_structure()
                .ok_or_else(|| {
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
                    + coupon.spread().value()
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
        let notionals = calculate_contractual_opening_balances(
            self.trade.instrument().notional(),
            &period_rates,
            repayment,
        )?;
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

impl ContFunc<f64> for LoanFairRateObjective<'_> {
    /// Returns unsigned trial receipt PV divided by advance PV, minus one.
    ///
    /// A zero residual represents par funding, independently of the trade side.
    ///
    /// # Errors
    /// Propagates trial-cashflow construction and receipt-projection errors.
    fn call(&self, candidate: &f64) -> Result<f64> {
        let trial = self.build_cashflows_for_trial_coupon_or_spread(*candidate)?;
        let mut repaid = 0.0;
        let mut prefix = Vec::new();
        for cashflow in trial {
            let payment = match &cashflow {
                CashflowType::FixedRateCoupon(coupon) => coupon.payment_date(),
                CashflowType::FloatingRateCoupon(coupon) => coupon.payment_date(),
                CashflowType::Redemption(principal) | CashflowType::Disbursement(principal) => {
                    principal.payment_date()
                }
                _ => continue,
            };
            if payment <= self.redemption_date {
                if let CashflowType::Redemption(principal) = &cashflow {
                    repaid += principal.amount()?;
                }
                prefix.push(cashflow);
            }
        }
        let remaining = (self.trade.instrument().notional() - repaid).max(0.0);
        if remaining > 0.0 {
            prefix.push(CashflowType::Redemption(SimpleCashflow::new(
                remaining,
                self.redemption_date,
            )));
        }
        let (value, _) = self
            .pricer
            .calculate_expected_receipts_and_present_value(self.trade, self.state, &prefix)?;
        Ok(value.value() * self.trade.side().sign() / self.target - 1.0)
    }
}

impl LoanPricer {
    /// Bootstraps contiguous coupon/spread segments to par successively.
    ///
    /// Each prefix repays its remaining capital at its final payment date.
    /// Solved coupon/spread levels stay fixed when extending to the next prefix.
    ///
    /// Matches receipt PV to original principal discounted from origination to
    /// the evaluation date. Bisection searches within [-0.05, 0.50], narrowing
    /// the upper bound when rebuilding would produce invalid amortization.
    /// It uses at most 100 iterations and a funding-residual tolerance of 1e-11.
    ///
    /// # Errors
    /// Rejects valuation after origination, missing coupons, and bounded coupons.
    /// Propagates market/trial errors and reports unbracketed or unconverged solves.
    fn handle_segment_fair_rate(
        &self,
        trade: &LoanTrade<DualFwd>,
        state: &LoanValuationState,
        target_segment: usize,
    ) -> Result<f64> {
        if state.evaluation_date > trade.instrument().start_date() {
            return Err(QSError::InvalidValueErr(
                "Loan fair rate requires valuation at or before origination".into(),
            ));
        }
        Tape::stop_recording_fwd();
        let mut cashflows = Self::clone_cashflows_with_resolved_fixings(trade.instrument(), state)?;
        if cashflows
            .iter()
            .any(|coupon| matches!(coupon, CashflowType::OptionEmbeddedCoupon(_)))
        {
            return Err(QSError::InvalidValueErr(
                "Loan fair rate does not support bounded coupons".into(),
            ));
        }
        let segments = Self::coupon_segments(&cashflows);
        if segments.is_empty() {
            return Err(QSError::ValueNotSetErr("Loan fair-rate coupons".into()));
        }
        let curve = state
            .get_discount_curve_element(&self.select_discount_curve_index(trade.instrument())?)?;
        let target = trade.instrument().notional()
            * curve
                .curve()
                .discount_factor(trade.instrument().start_date())?
                .value()
            / curve
                .curve()
                .discount_factor(state.evaluation_date)?
                .value();
        if target_segment >= segments.len() {
            return Err(QSError::InvalidValueErr(format!(
                "Loan fair-rate target {target_segment} is out of range for {} segments (zero-based)",
                segments.len()
            )));
        }
        let mut selected_rate = 0.0;
        for (position, segment) in segments.into_iter().take(target_segment + 1).enumerate() {
            let objective = LoanFairRateObjective {
                pricer: self,
                trade,
                state,
                cashflows: &cashflows,
                adjustable_segment: segment.coupons,
                redemption_date: segment.redemption_date,
                target,
            };
            let solve = || -> Result<f64> {
                let upper = objective.admissible_upper_bound(-0.05, 0.5)?;
                if (objective.call(&upper)? - objective.call(&-0.05)?).abs() < 1e-14 {
                    return Err(QSError::SolverErr(
                        "Loan segment has no identifiable rate exposure".into(),
                    ));
                }
                let solution = Bisection::new(-0.05, upper, 100)
                    .with_ftol(1e-11)
                    .solve(&objective)?;
                if !matches!(solution.status, SolutionStatus::Converged) {
                    return Err(QSError::SolverErr(
                        "Loan fair rate did not converge in [-5%, 50%]".into(),
                    ));
                }
                Ok(solution.x)
            };
            let rate = solve().map_err(|error| {
                QSError::SolverErr(format!("Loan fair-rate segment {}: {error}", position + 1))
            })?;
            cashflows = objective.build_cashflows_for_trial_coupon_or_spread(rate)?;
            selected_rate = rate;
        }
        Ok(selected_rate)
    }

    /// Identifies contiguous coupon levels, ignoring intervening principal flows.
    fn coupon_segments(cashflows: &[CashflowType<DualFwd>]) -> Vec<LoanCouponSegment> {
        let mut previous = None;
        let mut segments: Vec<LoanCouponSegment> = Vec::new();
        let mut position = 0;
        for cashflow in cashflows {
            let (key, payment) = match cashflow {
                CashflowType::FixedRateCoupon(coupon) => (
                    (None, coupon.rate().rate().value().to_bits()),
                    coupon.payment_date(),
                ),
                CashflowType::FloatingRateCoupon(coupon) => (
                    (
                        Some(coupon.market_index().clone()),
                        coupon.spread().value().to_bits(),
                    ),
                    coupon.payment_date(),
                ),
                _ => continue,
            };
            if previous.as_ref() == Some(&key) {
                if let Some(segment) = segments.last_mut() {
                    segment.coupons.end = position + 1;
                    segment.redemption_date = payment;
                }
            } else {
                segments.push(LoanCouponSegment {
                    coupons: position..position + 1,
                    redemption_date: payment,
                });
            }
            previous = Some(key);
            position += 1;
        }
        segments
    }
}

impl HandleFairRate<LoanTrade<DualFwd>, LoanValuationState> for LoanPricer {
    /// Returns the selected segment's coupon or spread after solving earlier segments.
    fn handle_fair_rate(
        &self,
        trade: &LoanTrade<DualFwd>,
        state: &mut LoanValuationState,
        target: usize,
    ) -> Result<f64> {
        self.handle_segment_fair_rate(trade, state, target)
    }
}

impl Pricer for LoanPricer {
    type Item = LoanTrade<DualFwd>;
    type Policy = dyn DiscountPolicy;
    /// Evaluates requested results against a fresh market-data state.
    ///
    /// Supports `Value`, `Cashflows`, `Sensitivities`, and `FairRate` in the
    /// caller's order. Repeated value/cashflow requests share a cached projection.
    ///
    /// # Errors
    /// Rejects a missing discount policy or unsupported result requests, and
    /// propagates data/handler errors.
    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        Request::validate(requests)?;
        let mut state = self.load_loan_valuation_state(trade, ctx)?;
        let mut results =
            EvaluationResults::new(ctx.evaluation_date(), trade.instrument().identifier());
        for request in requests {
            results = match request {
                Request::Value => results.with_price(self.handle_value(trade, &mut state)?),
                Request::Sensitivities => {
                    results.with_sensitivities(self.handle_sensitivities(trade, &mut state)?)
                }
                Request::FairRate(target) => {
                    results.with_fair_rate(self.handle_fair_rate(trade, &mut state, *target)?)
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
    /// Collects deduplicated discount, coupon-index, refinancing, and credit curves.
    ///
    /// Coupon fixing requests are initially optional; `load_loan_valuation_state` requires
    /// historical observations only for unpaid coupons. Returns `None` if the
    /// discount index cannot be selected or required refinancing is unconfigured.
    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest> {
        let loan = trade.instrument();
        let mut indices = vec![self.select_discount_curve_index(loan).ok()?];
        indices.extend(
            loan.coupon_rate_observations()
                .iter()
                .flatten()
                .map(ForwardRateRequest::market_index),
        );
        if loan.prepayment_model().requires_refinancing_benchmark() {
            indices.push(self.refinancing_benchmark_index.clone()?);
        }
        let mut seen = HashSet::new();
        let mut elements = indices
            .into_iter()
            .filter(|index| seen.insert(index.clone()))
            .map(|market_index| ConstructedElementRequest::DiscountCurve { market_index })
            .collect::<Vec<_>>();
        if let Some(index) = loan.credit_curve_index() {
            elements.push(ConstructedElementRequest::CreditCurve {
                market_index: index.clone(),
            });
        }
        let mut seen_fixings = HashSet::new();
        let fixings = loan
            .coupon_rate_observations()
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
    /// Installs a policy used to select the loan's discount curve.
    fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
        self.discount_policy = Some(policy);
    }
    /// Returns the configured policy; pricing requires a policy to be present.
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
    fn build_test_market(discount: f64, forward: f64, hazard: f64) -> Provider {
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
        let provider = build_test_market(0.03, 0.04, 0.0);
        let loan = default_loan_builder().build()?;
        assert!(loan.discount_index().is_none());
        assert!(loan.contractual_leg().discount_index().is_none());
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
            .contractual_leg()
            .discount_index()
            .is_none());
        Ok(())
    }
    fn default_loan_builder() -> MakeLoan<DualFwd> {
        MakeLoan::default()
            .with_identifier("LOAN".into())
            .with_start_date(Date::new(2026, 1, 1))
            .with_maturity_date(Date::new(2028, 1, 1))
            .with_notional(100_000.0)
            .with_currency(Currency::EUR)
            .with_coupons(vec![CashflowType::fixed(0.06)])
            .with_day_counter(DayCounter::Actual360)
    }
    fn evaluate_loan_receipt_present_value(
        pricer: &LoanPricer,
        trade: &LoanTrade<DualFwd>,
        market: &Provider,
    ) -> Result<f64> {
        pricer
            .evaluate(trade, &[Request::Value], market)?
            .price()
            .ok_or_else(|| QSError::ValueNotSetErr("Price".into()))
    }
    #[test]
    fn shared_cashflow_request_matches_pv_and_request_order_is_stable() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let loan = default_loan_builder()
            .with_prepayment_configuration(PrepaymentModelConfiguration::Cpr(0.25))
            .build()?;
        let original = loan.contractual_leg().cashflows().len();
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let pricer = estr_loan_pricer();
        let result = pricer.evaluate(
            &trade,
            &[
                Request::Cashflows,
                Request::Cashflows,
                Request::FairRate(0),
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
        assert_eq!(
            trade.instrument().contractual_leg().cashflows().len(),
            original
        );
        let short = LoanTrade::new(trade.instrument().clone(), provider.date, Side::PayShort);
        let short_result = pricer.evaluate(&short, &[Request::Cashflows], &provider)?;
        let short_table = short_result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Short cashflows".into()))?;
        assert_eq!(table.payment_dates(), short_table.payment_dates());
        assert_eq!(table.amounts().len(), short_table.amounts().len());
        for (long, short) in table.amounts().iter().zip(short_table.amounts()) {
            assert!((long + short).abs() < 1e-10);
        }
        assert!(
            (evaluate_loan_receipt_present_value(&pricer, &short, &provider)? + expected).abs()
                < 1e-7
        );
        Ok(())
    }

    #[test]
    fn floating_market_changes_preserve_principal_and_exact_reset_fixings() -> Result<()> {
        let loan = default_loan_builder()
            .with_coupons(vec![CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.01,
                None,
                None,
            )])
            .with_initial_amortization_rate(0.05)
            .with_calendar(Calendar::TARGET(
                crate::time::calendars::target::TARGET::new(),
            ))
            .with_fixing_lag_business_days(2)
            .build()?;
        let trade = LoanTrade::new(loan, Date::new(2026, 1, 1), Side::LongReceive);
        let pricer = estr_loan_pricer();
        let mut redemptions = Vec::new();
        for rate in [0.02, 0.06] {
            let result = pricer.evaluate(
                &trade,
                &[Request::Cashflows],
                &build_test_market(0.03, rate, 0.0),
            )?;
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
        let mut missing = build_test_market(0.03, 0.04, 0.0);
        missing.data = MarketData::new(HashMap::new(), missing.data.constructed_elements().clone());
        assert!(pricer
            .evaluate(&trade, &[Request::Value], &missing)
            .is_err());
        Ok(())
    }

    #[test]
    fn seasoned_cashflows_request_only_observations_needed_by_future_coupons() -> Result<()> {
        let loan = default_loan_builder()
            .with_coupons(vec![CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.01,
                None,
                None,
            )])
            .with_initial_amortization_rate(0.05)
            .build()?;
        let mut provider = build_test_market(0.03, 0.04, 0.0);
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
    fn fair_rates_bootstrap_mixed_and_stepped_segments_with_early_principal_repayment() -> Result<()>
    {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let coupon_schedule = |rates: &[f64]| {
            rates
                .iter()
                .enumerate()
                .flat_map(|(position, rate)| {
                    let coupon = if position < 2 {
                        CashflowType::fixed(*rate)
                    } else {
                        CashflowType::floating(MarketIndex::EURIBOR12m, *rate, None, None)
                    };
                    vec![coupon; 6]
                })
                .collect()
        };
        for repayment in [
            LoanRepaymentStructure::French,
            LoanRepaymentStructure::Bullet,
            LoanRepaymentStructure::Balloon {
                residual_fraction: 0.25,
            },
        ] {
            let builder = default_loan_builder()
                .with_repayment_structure(repayment)
                .with_coupons(coupon_schedule(&[0.06, 0.04, 0.01, 0.005]))
                .with_initial_amortization_rate(0.05);
            for side in [Side::LongReceive, Side::PayShort] {
                let trade = LoanTrade::new(builder.clone().build()?, provider.date, side);
                let original = estr_loan_pricer().evaluate(
                    &trade,
                    &[Request::Value, Request::Cashflows],
                    &provider,
                )?;
                for rebuild in [false, true] {
                    let pricer = estr_loan_pricer().with_fair_rate_amortization_rebuild(rebuild);
                    let result = pricer.evaluate(
                        &trade,
                        &[Request::FairRate(0), Request::Value, Request::Cashflows],
                        &provider,
                    )?;
                    let rates = (0..4)
                        .map(|target| {
                            pricer
                                .evaluate(&trade, &[Request::FairRate(target)], &provider)?
                                .fair_rate()
                                .ok_or_else(|| QSError::ValueNotSetErr("Segment fair rate".into()))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let candidate = LoanTrade::new(
                        builder
                            .clone()
                            .with_coupons(coupon_schedule(&rates))
                            .with_initial_amortization_rate(if rebuild { rates[0] } else { 0.05 })
                            .build()?,
                        provider.date,
                        side,
                    );
                    let par = pricer.evaluate(
                        &candidate,
                        &[Request::Value, Request::Cashflows],
                        &provider,
                    )?;
                    assert!(
                        (par.price().unwrap_or_default() - side.sign() * 100_000.0).abs() < 1e-5
                    );
                    let table = par
                        .cashflows()
                        .ok_or_else(|| QSError::ValueNotSetErr("Par cashflows".into()))?;
                    for segment in 1..=4 {
                        let end = provider
                            .date
                            .advance(segment * 6, crate::time::enums::TimeUnit::Months);
                        let mut prefix_value = 0.0;
                        let mut repaid = 0.0;
                        for ((date, amount), kind) in table
                            .payment_dates()
                            .iter()
                            .zip(table.amounts())
                            .zip(table.cashflow_types())
                        {
                            if *date <= end {
                                prefix_value += amount
                                    * (-0.03
                                        * DayCounter::Actual365
                                            .year_fraction(provider.date, *date))
                                    .exp();
                                if kind == "Redemption" {
                                    repaid += amount;
                                }
                            }
                        }
                        prefix_value += (side.sign() * 100_000.0 - repaid)
                            * (-0.03 * DayCounter::Actual365.year_fraction(provider.date, end))
                                .exp();
                        assert!(
                            (prefix_value - side.sign() * 100_000.0).abs() < 1e-5,
                            "segment {segment}, rebuild={rebuild}: {prefix_value}"
                        );
                    }
                    assert_eq!(original.price(), result.price());
                    assert_eq!(
                        original.cashflows().map(|table| table.amounts()),
                        result.cashflows().map(|table| table.amounts())
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn coupon_segments_preserve_repeated_levels_and_ignore_principal_and_index_resets() -> Result<()>
    {
        let mut coupons = vec![CashflowType::fixed(0.03); 6];
        coupons.extend(vec![CashflowType::fixed(0.04); 6]);
        coupons.extend(vec![CashflowType::fixed(0.03); 6]);
        coupons.extend(vec![
            CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.03,
                None,
                None
            );
            3
        ]);
        coupons.push(CashflowType::floating(
            MarketIndex::EURIBOR3m,
            0.03,
            None,
            None,
        ));
        let loan = default_loan_builder().with_coupons(coupons).build()?;
        let segments = LoanPricer::coupon_segments(loan.contractual_leg().cashflows());
        let ranges = segments
            .iter()
            .map(|segment| segment.coupons.clone())
            .collect::<Vec<_>>();
        assert_eq!(ranges, vec![0..6, 6..12, 12..18, 18..21, 21..24]);
        let floating = default_loan_builder()
            .with_coupons(vec![CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.01,
                None,
                None,
            )])
            .with_initial_amortization_rate(0.05)
            .build()?;
        assert_eq!(
            LoanPricer::coupon_segments(floating.contractual_leg().cashflows()).len(),
            1
        );
        Ok(())
    }

    #[test]
    fn floating_first_bootstrap_retains_the_solved_initial_amortization_rate() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let coupons = |spread, fixed_rate| {
            let mut coupons =
                vec![CashflowType::floating(MarketIndex::EURIBOR12m, spread, None, None); 12];
            coupons.push(CashflowType::fixed(fixed_rate));
            coupons
        };
        let builder = default_loan_builder()
            .with_coupons(coupons(0.01, 0.06))
            .with_initial_amortization_rate(0.05);
        let trade = LoanTrade::new(builder.clone().build()?, provider.date, Side::LongReceive);
        for rebuild in [false, true] {
            let pricer = estr_loan_pricer().with_fair_rate_amortization_rebuild(rebuild);
            let rates = (0..2)
                .map(|target| {
                    pricer
                        .evaluate(&trade, &[Request::FairRate(target)], &provider)?
                        .fair_rate()
                        .ok_or_else(|| QSError::ValueNotSetErr("Floating-first fair rate".into()))
                })
                .collect::<Result<Vec<_>>>()?;
            let initial_rate = if rebuild {
                0.04_f64.exp_m1() / (365.0 / 360.0) + rates[0]
            } else {
                0.05
            };
            let candidate = LoanTrade::new(
                builder
                    .clone()
                    .with_coupons(coupons(rates[0], rates[1]))
                    .with_initial_amortization_rate(initial_rate)
                    .build()?,
                provider.date,
                Side::LongReceive,
            );
            assert!(
                (evaluate_loan_receipt_present_value(&pricer, &candidate, &provider)? - 100_000.0)
                    .abs()
                    < 1e-5
            );
        }
        Ok(())
    }

    #[test]
    fn fully_prepaid_segments_report_an_unidentifiable_rate() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let mut coupons = vec![CashflowType::fixed(0.04); 12];
        coupons.push(CashflowType::fixed(0.06));
        let trade = LoanTrade::new(
            default_loan_builder()
                .with_coupons(coupons)
                .with_prepayment_configuration(PrepaymentModelConfiguration::Cpr(1.0))
                .build()?,
            provider.date,
            Side::LongReceive,
        );
        // A valid earlier target does not attempt to solve an unidentifiable later segment.
        assert!(estr_loan_pricer()
            .evaluate(&trade, &[Request::FairRate(0)], &provider)?
            .fair_rate()
            .is_some());
        assert!(estr_loan_pricer()
            .evaluate(&trade, &[Request::FairRate(2)], &provider)
            .is_err());
        let error = estr_loan_pricer()
            .evaluate(&trade, &[Request::FairRate(1)], &provider)
            .err()
            .ok_or_else(|| QSError::ValueNotSetErr("Expected unidentified segment error".into()))?;
        assert!(error.to_string().contains("segment 2"));
        assert!(error.to_string().contains("no identifiable rate exposure"));
        Ok(())
    }

    #[test]
    fn fair_rate_rebuilds_thirty_year_mortgages_with_actual_month_lengths() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        for floating in [false, true] {
            let rate = if floating {
                CashflowType::floating(MarketIndex::EURIBOR12m, 0.01, None, None)
            } else {
                CashflowType::fixed(0.06)
            };
            let builder = default_loan_builder()
                .with_maturity_date(Date::new(2056, 1, 1))
                .with_notional(250_000.0)
                .with_coupons(vec![rate])
                .with_initial_amortization_rate(if floating { 0.05 } else { 0.06 });
            let trade = LoanTrade::new(builder.clone().build()?, provider.date, Side::LongReceive);
            let pricer = estr_loan_pricer().with_fair_rate_amortization_rebuild(true);
            let fair = pricer
                .evaluate(&trade, &[Request::FairRate(0)], &provider)?
                .fair_rate()
                .ok_or_else(|| QSError::ValueNotSetErr("Mortgage fair rate".into()))?;
            let rate = if floating {
                CashflowType::floating(MarketIndex::EURIBOR12m, fair, None, None)
            } else {
                CashflowType::fixed(fair)
            };
            let initial_index = 0.04_f64.exp_m1() / (365.0 / 360.0);
            let par = LoanTrade::new(
                builder
                    .with_coupons(vec![rate])
                    .with_initial_amortization_rate(if floating {
                        initial_index + fair
                    } else {
                        fair
                    })
                    .build()?,
                provider.date,
                Side::LongReceive,
            );
            let value = evaluate_loan_receipt_present_value(&pricer, &par, &provider)?;
            assert!((value - 250_000.0).abs() < 1e-5);
        }
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fair_rate_preserves_or_recomputes_existing_schedule() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        for floating in [false, true] {
            let rate = if floating {
                CashflowType::floating(MarketIndex::EURIBOR12m, 0.01, None, None)
            } else {
                CashflowType::fixed(0.09)
            };
            let original_rate = if floating { 0.05 } else { 0.09 };
            let original_builder = default_loan_builder()
                .with_coupons(vec![rate])
                .with_initial_amortization_rate(original_rate);
            let trade = LoanTrade::new(
                original_builder.clone().build()?,
                provider.date,
                Side::LongReceive,
            );
            let snapshot = trade
                .instrument()
                .contractual_leg()
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
                let pricer = estr_loan_pricer().with_fair_rate_amortization_rebuild(rebuild);
                let fair = pricer
                    .evaluate(&trade, &[Request::FairRate(0)], &provider)?
                    .fair_rate()
                    .ok_or_else(|| QSError::ValueNotSetErr("Fair rate".into()))?;
                let rate = if floating {
                    CashflowType::floating(MarketIndex::EURIBOR12m, fair, None, None)
                } else {
                    CashflowType::fixed(fair)
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
                        .with_coupons(vec![rate])
                        .with_initial_amortization_rate(amortization_rate)
                        .build()?,
                    provider.date,
                    Side::LongReceive,
                );
                assert!(
                    (evaluate_loan_receipt_present_value(&pricer, &candidate, &provider)?
                        - 100_000.0)
                        .abs()
                        < 1e-5
                );
                let candidate_schedule = candidate
                    .instrument()
                    .contractual_leg()
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
                .contractual_leg()
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
        let loan = default_loan_builder()
            .with_custom_prepayment_model(|_: &PrepaymentModelState<DualFwd>| Ok(DualFwd::one()))
            .build()?;
        let provider = build_test_market(0.03, 0.04, 0.0);
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
        let invalid = default_loan_builder()
            .with_custom_prepayment_model(|_: &PrepaymentModelState<DualFwd>| {
                Ok(DualFwd::scalar(1.1))
            })
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
        let loan = default_loan_builder()
            .with_coupons(vec![CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.02,
                None,
                None,
            )])
            .with_initial_amortization_rate(0.06)
            .with_credit_curve_index(MarketIndex::Credit("BORROWER".into()))
            .with_prepayment_configuration(PrepaymentModelConfiguration::Refinancing {
                base_cpr: 0.02,
                incentive_multiplier: 4.0,
                maximum_cpr: 0.4,
            })
            .build()?;
        let provider = build_test_market(0.03, 0.04, 0.01);
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let pricer = estr_loan_pricer().with_refinancing_benchmark_index(MarketIndex::EURIBOR12m);
        let result = pricer.evaluate(&trade, &[Request::Sensitivities], &provider)?;
        let risk = result
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Risk".into()))?;
        for (label, derivative) in risk.instrument_keys().iter().zip(risk.exposure()) {
            let bump = 1e-6;
            let up = build_test_market(
                0.03 + if label == "DISCOUNT" { bump } else { 0.0 },
                0.04 + if label == "FORWARD" { bump } else { 0.0 },
                0.01 + if label == "CREDIT" { bump } else { 0.0 },
            );
            let down = build_test_market(
                0.03 - if label == "DISCOUNT" { bump } else { 0.0 },
                0.04 - if label == "FORWARD" { bump } else { 0.0 },
                0.01 - if label == "CREDIT" { bump } else { 0.0 },
            );
            let fd = (evaluate_loan_receipt_present_value(&pricer, &trade, &up)?
                - evaluate_loan_receipt_present_value(&pricer, &trade, &down)?)
                / (2.0 * bump);
            assert!(
                (fd - derivative).abs() < derivative.abs().mul_add(1e-5, 0.1),
                "{label}: AD {derivative}, FD {fd}"
            );
        }
        Ok(())
    }
}

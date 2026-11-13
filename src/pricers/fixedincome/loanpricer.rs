//! Loan valuation resolves existing contractual cashflows and projects optional
//! prepayment into a separate pricing state.

use crate::{
    ad::{
        dual::DualFwd,
        forward::ADForward,
        node::TapeNode,
        scalar::Scalar,
        tape::{Tape, TapeHolder},
    },
    core::{
        collateral::DiscountPolicy,
        elements::curveelement::{CreditCurveElement, DiscountCurveElement},
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::{AssetClass, Instrument},
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            fixingrequest::FixingRequest,
            forwardraterequest::ForwardRateRequest,
            fxrequest::FxRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricer::Pricer,
        pricerconfiguration::{InstrumentType, PricerConfiguration, PricerImplementation},
        pricerstate::PricerState,
        request::{
            accrued_amount, HandleAccruedAmount, HandleCashflows, HandleFairRate,
            HandleSensitivities, HandleValue, LegsProvider, Request,
        },
        trade::Trade,
    },
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::{
            cashflow::{Cashflow, SimpleCashflow},
            cashflowtype::CashflowType,
            coupons::NonLinearCoupon,
            fixedratecoupon::FixedRateCoupon,
            floatingratecoupon::FloatingRateCoupon,
            leg::Leg,
            optionembeddedcoupon::OptionEmbeddedCoupon,
        },
        fixedincome::{
            loan::{Loan, LoanTrade},
            loanrepaymentstructure::{level_installment, LoanRepaymentStructure},
            makeloan::calculate_contractual_opening_balances,
        },
    },
    math::solvers::{
        bisection::Bisection,
        solvertraits::{ContFunc, SolutionStatus},
    },
    models::prepaymentmodelstate::PrepaymentModelState,
    rates::interestrate::InterestRate,
    time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
    utils::errors::{QSError, Result},
};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

/// Prices the existing loan leg.
///
/// Configure a discount policy before valuation; its currency mapping selects
/// the curve independently of the contract.
/// French installments are recalculated at each contractual rate/reset change.
/// Prepayment and credit affect projected receipts only. `Request::Cashflows(true)`
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

    /// Creates the native pricer from the context's typed loan configuration.
    /// Discounting is supplied separately through [`Pricer::set_discount_policy`].
    ///
    /// # Errors
    /// Rejects configurations for another instrument or pricing implementation.
    pub fn from_configuration(configuration: &PricerConfiguration) -> Result<Self> {
        configuration.validate()?;
        if let (
            InstrumentType::Loan,
            PricerImplementation::Loan {
                refinancing_benchmark_index,
                rebuild_amortization_for_fair_rate,
            },
        ) = (configuration.instrument_type(), configuration.pricer())
        {
            Ok(Self {
                discount_policy: None,
                refinancing_benchmark_index: refinancing_benchmark_index.clone(),
                rebuild_amortization_for_fair_rate: *rebuild_amortization_for_fair_rate,
            })
        } else {
            Err(QSError::InvalidValueErr(
                "LoanPricer requires a Loan pricer configuration".into(),
            ))
        }
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
    /// Both modes retain the instrument. Ordinary valuation and sensitivities
    /// apply the contractual French reset rule.
    #[must_use]
    pub const fn with_fair_rate_amortization_rebuild(mut self, enabled: bool) -> Self {
        self.rebuild_amortization_for_fair_rate = enabled;
        self
    }

    /// Selects the discount curve exclusively through the configured policy.
    ///
    /// # Errors
    /// Rejects an unconfigured policy and propagates policy resolution errors.
    fn select_discount_curve_index<N: Scalar>(&self, loan: &Loan<N>) -> Result<MarketIndex> {
        self.discount_policy
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Loan discount policy".into()))?
            .accept(loan)
    }

    /// Loads curves and historical resets needed to replay the loan from origination into a new state.
    ///
    /// Past fixing dates require stored observations. A fixing on the evaluation
    /// date may be absent and forecast instead; settled coupons also require their historical resets
    /// when reconstructing amortization and prepayment. Future rates are read from projection curves.
    ///
    /// # Errors
    /// Rejects missing discount policies/refinancing indices and propagates market-data
    /// errors, including missing historical fixings from origination.
    fn load_loan_valuation_state(
        &self,
        trade: &LoanTrade<DualFwd>,
        ctx: &impl MarketDataProvider,
    ) -> Result<LoanValuationState> {
        self.select_discount_curve_index(trade.instrument())?;
        let request = self.market_data_request(trade).ok_or_else(|| {
            QSError::ValueNotSetErr("Loan refinancing benchmark or discount policy".into())
        })?;
        let fixings = request
            .fixings_request()
            .into_iter()
            .flatten()
            .filter(|request| ctx.anchor_fixings() && request.date() < ctx.evaluation_date())
            .map(|request| FixingRequest::new(request.market_index().clone(), request.date()))
            .collect();
        let request = request.with_fixings_request(fixings);
        let market_data = ctx
            .handle_request(&request)?
            .with_anchor_fixings(ctx.anchor_fixings());
        let state = LoanValuationState {
            market_data: Some(market_data),
            evaluation_date: ctx.evaluation_date(),
            value: None,
            legs: Vec::new(),
            record_sensitivities: true,
            report_cashflows: true,
        };
        Ok(state)
    }

    /// Resolves an index observation with its own tenor and quote convention.
    ///
    /// With anchoring enabled, historical dates require exact fixings. Otherwise
    /// observations use the index curve's discount-factor ratio over the
    /// requested start/end dates. Stored fixings have no AD risk.
    ///
    /// # Errors
    /// Propagates missing historical fixings, incomplete observation dates,
    /// unavailable index curves/conventions, and implied-rate conversion errors.
    fn resolve_index_rate_observation<N: Scalar + From<DualFwd>>(
        state: &LoanValuationState,
        request: &ForwardRateRequest,
    ) -> Result<N> {
        request
            .resolve_index_rate(
                state
                    .get_market_data_reponse()
                    .ok_or_else(|| QSError::ValueNotSetErr("Loan market data".into()))?,
                state.evaluation_date,
                state
                    .get_market_data_reponse()
                    .is_none_or(MarketData::anchor_fixings),
            )
            .map(N::from)
    }

    /// Clones the contractual cashflows and installs every contractual coupon fixing.
    ///
    /// Coupons sharing an index, fixing date, and observation interval reuse one
    /// resolved rate. The loan's cashflows, notionals, and observations are kept.
    ///
    /// # Errors
    /// Rejects missing floating observation entries and propagates forecast errors.
    fn clone_cashflows_with_resolved_fixings<N: Scalar + From<DualFwd>>(
        loan: &Loan<N>,
        state: &LoanValuationState,
    ) -> Result<Vec<CashflowType<N>>> {
        let mut cashflows = loan.contractual_leg().cashflows().to_vec();
        let mut observations = loan.coupon_rate_observations().iter();
        let mut resolved = HashMap::new();
        for cashflow in &mut cashflows {
            match cashflow {
                CashflowType::FixedRateCoupon(_) => {
                    observations.next();
                    continue;
                }
                CashflowType::FloatingRateCoupon(_) | CashflowType::OptionEmbeddedCoupon(_) => {}
                _ => continue,
            }
            let request = observations
                .next()
                .and_then(Option::as_ref)
                .ok_or_else(|| QSError::ValueNotSetErr("Loan coupon observation".into()))?;

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
                CashflowType::FloatingRateCoupon(coupon) => {
                    coupon.set_fixing_date(request.fixing_date());
                    coupon.set_fixing(rate);
                }
                CashflowType::OptionEmbeddedCoupon(coupon) => {
                    coupon.set_fixing_date(request.fixing_date());
                    coupon.set_fixing(rate);
                }
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
    fn observe_refinancing_benchmark_rate<N: Scalar + From<DualFwd>>(
        &self,
        state: &LoanValuationState,
        date: Date,
    ) -> Result<N> {
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
            .with_end_date(date.advance(tenor.length(), tenor.units())?);
        Self::resolve_index_rate_observation(state, &request)
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
    record_sensitivities: bool,
    report_cashflows: bool,
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
    /// Records market pillars and projection operations only when sensitivities
    /// are requested. Reporting legs are built only for cashflows or accrual.
    /// Recording is stopped after the computation, including on errors.
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
        if state.record_sensitivities {
            Tape::start_recording_fwd();
            Tape::set_mark_fwd();
        } else {
            Tape::stop_recording_fwd();
        }
        let result = (|| {
            if state.record_sensitivities {
                state.put_pillars_on_tape()?;
            }
            let cashflows = Self::clone_cashflows_with_resolved_fixings(trade.instrument(), state)?;
            let (value, legs) = self.project_expected_receipts(
                trade,
                state,
                &cashflows,
                true,
                None,
                state.report_cashflows,
            )?;
            let source = trade.instrument().currency();
            let value = if let Some(target) = state
                .get_market_data_reponse()
                .and_then(MarketData::base_currency)
                .filter(|target| *target != source)
            {
                (value * state.get_exchange_rate(source, target)?).into()
            } else {
                value
            };
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
    /// prepayment, survival and rate-dependent amortization.
    ///
    /// # Errors
    /// Propagates valuation, AD backpropagation, and missing pillar-adjoint errors.
    fn handle_sensitivities(
        &self,
        trade: &LoanTrade<DualFwd>,
        state: &mut LoanValuationState,
    ) -> Result<SensitivityMap> {
        self.handle_value(trade, state)?;
        // A repeated request reuses the value graph, but needs a fresh backward pass.
        Tape::reset_adjoints_fwd();
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
        for (label, exposure) in state.get_fx_sensitivities()? {
            keys.push(label);
            exposures.push(exposure);
        }
        Ok(SensitivityMap::default()
            .with_instrument_keys(&keys)
            .with_exposure(&exposures))
    }
}

impl<N: LoanAmortizationScalar> HandleAccruedAmount<LoanTrade<N>, LoanValuationState> for LoanPricer
where
    FixedRateCoupon<N>: Cashflow<N>,
    FloatingRateCoupon<N>: Cashflow<N>,
    OptionEmbeddedCoupon<N>: NonLinearCoupon<N>,
    Leg<f64>: From<Leg<N>>,
{
    fn handle_accrued_amount(
        &self,
        trade: &LoanTrade<N>,
        state: &mut LoanValuationState,
        start: Date,
        end: Date,
    ) -> Result<f64> {
        if !state.legs.is_empty() {
            let legs: Vec<Leg<f64>> = state.legs.iter().cloned().map(Into::into).collect();
            return accrued_amount(&legs, state.get_market_data_reponse(), start, end);
        }
        let cashflows = Self::clone_cashflows_with_resolved_fixings(trade.instrument(), state)?;
        let (_, projected) =
            self.project_expected_receipts(trade, state, &cashflows, false, None, true)?;
        let legs: Vec<Leg<f64>> = projected.into_iter().map(Into::into).collect();
        accrued_amount(&legs, state.get_market_data_reponse(), start, end)
    }
}

impl Pricer for LoanPricer {
    type Item = LoanTrade<DualFwd>;
    type Policy = dyn DiscountPolicy;
    /// Evaluates requested results against a fresh market-data state.
    ///
    /// Supports `Value`, `Cashflows`, `Sensitivities`, `FairRate` and `AccruedAmount` in the
    /// caller's order. Accrual reuses the projection when cashflows, value or
    /// sensitivities are requested; standalone accrual projects numeric receipts.
    ///
    /// # Errors
    /// Rejects a missing discount policy and propagates data/handler errors.
    /// Unsupported measures are left absent.
    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let supported: Vec<Request> = requests
            .iter()
            .copied()
            .filter(|request| {
                matches!(
                    request,
                    Request::Value
                        | Request::Sensitivities
                        | Request::Cashflows(_)
                        | Request::FairRate(_)
                        | Request::AccruedAmount(_, _)
                )
            })
            .collect();
        let requests = supported.as_slice();
        Request::validate(requests)?;
        if requests.is_empty() {
            return Ok(EvaluationResults::new(
                ctx.evaluation_date(),
                trade.instrument().identifier(),
            ));
        }
        let mut state = self.load_loan_valuation_state(trade, ctx)?;
        state.record_sensitivities = requests.contains(&Request::Sensitivities);
        state.report_cashflows = requests.iter().any(|request| {
            matches!(
                request,
                Request::Cashflows(_) | Request::AccruedAmount(_, _)
            )
        });
        if !state.record_sensitivities {
            Tape::stop_recording_fwd();
        }
        if requests
            .iter()
            .any(|request| matches!(request, Request::Value | Request::Sensitivities))
        {
            self.handle_value(trade, &mut state)?;
        } else if requests
            .iter()
            .any(|request| matches!(request, Request::Cashflows(_)))
        {
            let cashflows =
                Self::clone_cashflows_with_resolved_fixings(trade.instrument(), &state)?;
            let (_, legs) =
                self.project_expected_receipts(trade, &state, &cashflows, false, None, true)?;
            state.legs = legs;
        }
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
                Request::AccruedAmount(start, end) => {
                    if *start < ctx.evaluation_date() {
                        return Err(QSError::InvalidValueErr(
                            "Loan accrual windows must start at or after evaluation".into(),
                        ));
                    }
                    results.with_accrued_amount(
                        self.handle_accrued_amount(trade, &mut state, *start, *end)?,
                    )
                }
                Request::Cashflows(from_evaluation_date) => results.with_cashflows(
                    self.handle_cashflows(trade, &mut state)?
                        .with_payment_cutoff(from_evaluation_date.then_some(ctx.evaluation_date())),
                ),
                _ => results,
            };
        }
        Ok(results)
    }
    /// Collects deduplicated discount, coupon-index, refinancing, and credit curves.
    ///
    /// Coupon fixing requests are initially optional; `load_loan_valuation_state` requires
    /// historical observations only from origination. Returns `None` if the
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
        let mut fixings: Vec<_> = loan
            .coupon_rate_observations()
            .iter()
            .flatten()
            .filter(|request| seen_fixings.insert((request.market_index(), request.fixing_date())))
            .map(|request| {
                FixingRequest::new(request.market_index(), request.fixing_date())
                    .with_missing_fixing_allowed(true)
            })
            .collect();
        if let Some(index) = self
            .refinancing_benchmark_index
            .as_ref()
            .filter(|_| loan.prepayment_model().requires_refinancing_benchmark())
        {
            for cashflow in loan.contractual_leg().cashflows() {
                let (date, payment) = match cashflow {
                    CashflowType::FixedRateCoupon(coupon) => {
                        (coupon.accrual_start_date(), coupon.payment_date())
                    }
                    CashflowType::FloatingRateCoupon(coupon) => {
                        (coupon.accrual_start_date(), coupon.payment_date())
                    }
                    CashflowType::OptionEmbeddedCoupon(coupon) => {
                        (coupon.accrual_start_date(), coupon.payment_date())
                    }
                    _ => continue,
                };
                if payment == loan.maturity_date() {
                    continue;
                }
                if seen_fixings.insert((index.clone(), date)) {
                    fixings.push(
                        FixingRequest::new(index.clone(), date).with_missing_fixing_allowed(true),
                    );
                }
            }
        }
        let mut request = MarketDataRequest::default()
            .with_constructed_elements_request(elements)
            .with_fixings_request(fixings);
        let mut fx = request.fx_request().cloned().unwrap_or_default();
        fx.push(FxRequest::single(loan.currency()));
        request = request.with_fx_request(fx);
        Some(request)
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

/// One resolved coupon and its contractual principal fallback for custom schedules.
struct LoanPeriod<'a, N: Scalar> {
    coupon: &'a CashflowType<N>,
    start: Date,
    end: Date,
    payment: Date,
    opening: f64,
    principal: f64,
    rate: N,
    fraction: f64,
    reset: Option<(MarketIndex, Date)>,
}

impl<'a, N: Scalar> LoanPeriod<'a, N> {
    /// Borrows coupon metadata once; no coupon or index observations are resolved here.
    fn from_coupon(coupon: &'a CashflowType<N>) -> Result<Option<Self>> {
        let (start, end, payment, opening, basis, rate, reset) = match coupon {
            CashflowType::FixedRateCoupon(c) => (
                c.accrual_start_date(),
                c.accrual_end_date(),
                c.payment_date(),
                c.notional(),
                c.rate().day_counter(),
                c.rate().rate(),
                None,
            ),
            CashflowType::FloatingRateCoupon(c) => (
                c.accrual_start_date(),
                c.accrual_end_date(),
                c.payment_date(),
                c.notional(),
                c.day_counter(),
                c.fixing()
                    .ok_or_else(|| QSError::ValueNotSetErr("Loan fixing".into()))?
                    .add_val(c.spread()),
                Some((c.market_index().clone(), c.fixing_date())),
            ),
            CashflowType::OptionEmbeddedCoupon(c) => (
                c.accrual_start_date(),
                c.accrual_end_date(),
                c.payment_date(),
                c.notional(),
                c.day_counter()?,
                c.payoff_ops()
                    .eval(
                        c.fixing()
                            .ok_or_else(|| QSError::ValueNotSetErr("Loan fixing".into()))?,
                    )?
                    .add_val(c.spread()),
                Some((c.market_index().clone(), c.fixing_date())),
            ),
            _ => return Ok(None),
        };
        Ok(Some(Self {
            coupon,
            start,
            end,
            payment,
            opening,
            principal: 0.0,
            rate,
            fraction: basis.year_fraction(start, end),
            reset,
        }))
    }

    /// Rebinds a report coupon while preserving its observation date and resolved fixing.
    fn reporting_coupon(&self, notional: f64) -> Result<CashflowType<N>> {
        let coupon = self
            .coupon
            .for_period(notional, self.start, self.end, self.payment, None)?;
        Ok(match (coupon, self.coupon) {
            (CashflowType::FloatingRateCoupon(c), CashflowType::FloatingRateCoupon(source)) => {
                let c = c.with_fixing_date(source.fixing_date());
                c.set_fixing(
                    source
                        .fixing()
                        .ok_or_else(|| QSError::ValueNotSetErr("Loan fixing".into()))?,
                );
                CashflowType::FloatingRateCoupon(c)
            }
            (CashflowType::OptionEmbeddedCoupon(c), CashflowType::OptionEmbeddedCoupon(source)) => {
                CashflowType::OptionEmbeddedCoupon(
                    c.with_fixing_date(source.fixing_date()).with_fixing(
                        source
                            .fixing()
                            .ok_or_else(|| QSError::ValueNotSetErr("Loan fixing".into()))?,
                    ),
                )
            }
            (coupon, _) => coupon,
        })
    }
}

/// Curves and date-normalization factors borrowed once for an entire projection.
struct ProjectionCurves<'a, N: Scalar> {
    discount: Option<&'a DiscountCurveElement>,
    credit: Option<&'a CreditCurveElement>,
    discount_anchor: N,
    credit_anchor: N,
    evaluation: Date,
}

impl<'a, N: Scalar + From<DualFwd>> ProjectionCurves<'a, N> {
    /// Loads optional curves without allocating date maps or copied curve nodes.
    fn new(
        pricer: &LoanPricer,
        loan: &Loan<N>,
        state: &'a LoanValuationState,
        discounted: bool,
    ) -> Result<Self> {
        let discount = discounted
            .then(|| state.get_discount_curve_element(&pricer.select_discount_curve_index(loan)?))
            .transpose()?;
        let credit = loan
            .credit_curve_index()
            .map(|index| state.get_credit_curve_element(index))
            .transpose()?;
        let discount_anchor = discount
            .map(|c| {
                c.curve()
                    .discount_factor(state.evaluation_date)
                    .map(N::from)
            })
            .transpose()?
            .unwrap_or_else(N::one);
        let credit_anchor = credit
            .map(|c| {
                c.curve()
                    .discount_factor(state.evaluation_date)
                    .map(N::from)
            })
            .transpose()?
            .unwrap_or_else(N::one);
        if !credit_anchor.value().is_finite() || credit_anchor.value() <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Invalid loan survival anchor".into(),
            ));
        }
        Ok(Self {
            discount,
            credit,
            discount_anchor,
            credit_anchor,
            evaluation: state.evaluation_date,
        })
    }

    /// Conditions future credit survival on evaluation; settled periods have no new default.
    fn survival(&self, date: Date) -> Result<N> {
        if date <= self.evaluation {
            return Ok(N::one());
        }
        Ok(self
            .credit
            .map(|c| c.curve().discount_factor(date).map(N::from))
            .transpose()?
            .unwrap_or_else(N::one)
            .div_val(self.credit_anchor))
    }

    /// Discounts future receipts only, leaving full-history reporting independent of PV.
    fn present_value(&self, date: Date, amount: N) -> Result<N> {
        if date <= self.evaluation {
            return Ok(N::zero());
        }
        self.discount.map_or_else(
            || Ok(N::zero()),
            |c| {
                Ok(amount.mul_val(
                    N::from(c.curve().discount_factor(date)?).div_val(self.discount_anchor),
                ))
            },
        )
    }
}

/// Scalars supporting reset installments without recording each annuity step.
trait LoanAmortizationScalar: Scalar + From<DualFwd> {
    /// Discounts the actual remaining accrual fractions at the current reset rate.
    fn reset_installment(
        principal: Self,
        residual: Self,
        rate: Self,
        fractions: impl Iterator<Item = f64>,
    ) -> Result<Self>;
}

impl LoanAmortizationScalar for f64 {
    fn reset_installment(
        principal: Self,
        residual: Self,
        rate: Self,
        fractions: impl Iterator<Item = Self>,
    ) -> Result<Self> {
        level_installment(
            principal,
            residual,
            fractions.map(|fraction| rate * fraction),
        )
    }
}

impl LoanAmortizationScalar for DualFwd {
    fn reset_installment(
        principal: Self,
        residual: Self,
        rate: Self,
        fractions: impl Iterator<Item = f64>,
    ) -> Result<Self> {
        if !ADForward::with_tape(|tape| tape.active) {
            return level_installment(
                principal.inner(),
                residual.inner(),
                fractions.map(|fraction| rate.inner() * fraction),
            )
            .map(Self::constant);
        }
        // Evaluate the annuity and its rate partial using the nested forward
        // scalar, then record one reverse node for the complete installment.
        // Forward seeds remain intact, including mixed higher derivatives.
        let mut discount = ADForward::one();
        let mut annuity = ADForward::zero();
        let mut discount_partial = ADForward::zero();
        let mut annuity_partial = ADForward::zero();
        for fraction in fractions {
            let compound = ADForward::one() + rate.inner() * fraction;
            if !compound.value().is_finite() || compound.value() <= 0.0 {
                return Err(QSError::InvalidValueErr(
                    "Invalid loan amortization rate".into(),
                ));
            }
            discount /= compound;
            discount_partial = (discount_partial - discount * fraction) / compound;
            annuity += discount;
            annuity_partial += discount_partial;
        }
        let value = (principal.inner() - residual.inner() * discount) / annuity;
        if !value.value().is_finite() || annuity.value() <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Invalid loan annuity factor".into(),
            ));
        }
        let rate_partial =
            (-residual.inner() * discount_partial - value * annuity_partial) / annuity;
        let mut node = TapeNode::default();
        for (input, partial) in [
            (principal, ADForward::one() / annuity),
            (residual, -discount / annuity),
            (rate, rate_partial),
        ] {
            if let Some(child) = input.node_ptr() {
                node.push_child(child, partial);
            }
        }
        let node = ADForward::with_tape(|tape| tape.record(node));
        Ok(Self::from_raw(value, node))
    }
}

/// Mutable amortization and prepayment state. All rate-dependent balances retain AD.
struct LoanProjection<N: Scalar> {
    balance: N,
    installment: N,
    factor: N,
    survival: N,
    value: N,
    scheduled: Vec<CashflowType<N>>,
    prepaid: Vec<CashflowType<N>>,
    recovered: Vec<CashflowType<N>>,
    report_cashflows: bool,
}

impl<N: Scalar> LoanProjection<N> {
    /// Resolves scheduled principal; a prefix horizon redeems the closing balance.
    fn principal(
        &self,
        period: &LoanPeriod<'_, N>,
        repayment: Option<&LoanRepaymentStructure>,
        final_payment: bool,
    ) -> Result<N> {
        if final_payment {
            return Ok(self.balance);
        }
        match repayment {
            Some(LoanRepaymentStructure::Bullet) => Ok(N::zero()),
            Some(LoanRepaymentStructure::French | LoanRepaymentStructure::Balloon { .. }) => {
                let principal = self.installment.sub_val(
                    self.balance
                        .mul_val(period.rate)
                        .mul_val(N::scalar(period.fraction)),
                );
                if principal.value() < -1e-8 || principal.value() > self.balance.value() + 1e-8 {
                    return Err(QSError::InvalidValueErr(
                        "Loan amortization must retain nonincreasing principal".into(),
                    ));
                }
                Ok(principal.max_val(N::zero()).min_val(self.balance))
            }
            None => Ok(N::scalar(period.principal)),
        }
    }

    /// Records undiscounted receipts in the existing coupon and redemption types.
    fn report(
        &mut self,
        period: &LoanPeriod<'_, N>,
        opening: N,
        principal: N,
        prepayment: N,
        recovery: N,
        weighted: N,
    ) -> Result<()> {
        let notional = opening.mul_val(weighted).value();
        if notional > 0.0 {
            self.scheduled.push(period.reporting_coupon(notional)?);
        }
        if principal.value() != 0.0 {
            self.scheduled
                .push(CashflowType::Redemption(SimpleCashflow::new(
                    principal.value(),
                    period.payment,
                )));
        }
        if prepayment.value() != 0.0 {
            self.prepaid
                .push(CashflowType::Redemption(SimpleCashflow::new(
                    prepayment.value(),
                    period.payment,
                )));
        }
        if recovery.value() != 0.0 {
            self.recovered
                .push(CashflowType::Redemption(SimpleCashflow::new(
                    recovery.value(),
                    period.payment,
                )));
        }
        Ok(())
    }
}

impl LoanPricer {
    /// Projects from origination, recasting French installments at each contractual reset.
    /// A horizon settles the remaining balance for a fair-rate prefix objective.
    fn project_expected_receipts<N: LoanAmortizationScalar>(
        &self,
        trade: &LoanTrade<N>,
        state: &LoanValuationState,
        cashflows: &[CashflowType<N>],
        discounted: bool,
        horizon: Option<Date>,
        report_cashflows: bool,
    ) -> Result<(N, Vec<Leg<N>>)> {
        let loan = trade.instrument();
        let periods = loan_periods(cashflows, loan.repayment_structure().is_none())?;
        let curves = ProjectionCurves::new(self, loan, state, discounted)?;
        let mut projection = LoanProjection {
            balance: N::scalar(loan.notional()),
            installment: N::zero(),
            factor: N::one(),
            survival: curves.survival(loan.start_date())?,
            value: N::zero(),
            scheduled: Vec::with_capacity(if report_cashflows { cashflows.len() } else { 0 }),
            prepaid: Vec::with_capacity(if report_cashflows { periods.len() } else { 0 }),
            recovered: Vec::new(),
            report_cashflows,
        };
        if report_cashflows {
            projection
                .scheduled
                .push(CashflowType::Disbursement(SimpleCashflow::new(
                    loan.notional(),
                    loan.start_date(),
                )));
        }
        for (position, period) in periods.iter().enumerate() {
            if projection.factor.value() == 0.0 || horizon.is_some_and(|date| period.payment > date)
            {
                break;
            }
            let reset = position == 0
                || period.reset != periods[position - 1].reset
                || period.rate.value().to_bits() != periods[position - 1].rate.value().to_bits();
            if reset
                && matches!(
                    loan.repayment_structure(),
                    Some(LoanRepaymentStructure::French | LoanRepaymentStructure::Balloon { .. })
                )
            {
                let residual = match loan.repayment_structure() {
                    Some(LoanRepaymentStructure::Balloon { residual_fraction }) => {
                        loan.notional() * residual_fraction
                    }
                    _ => 0.0,
                };
                projection.installment = N::reset_installment(
                    projection.balance,
                    N::scalar(residual),
                    period.rate,
                    periods[position..].iter().map(|future| future.fraction),
                )?;
            }
            if loan.repayment_structure().is_none() {
                projection.balance = N::scalar(period.opening);
            }
            self.project_period(
                loan,
                state,
                period,
                &curves,
                &mut projection,
                position + 1 == periods.len() || horizon == Some(period.payment),
            )?;
        }
        if !projection.value.value().is_finite() {
            return Err(QSError::InvalidValueErr("Nonfinite loan value".into()));
        }
        let value = projection.value.mul_val(N::scalar(trade.side().sign()));
        if !report_cashflows {
            return Ok((value, Vec::new()));
        }
        let legs = [
            projection.scheduled,
            projection.prepaid,
            projection.recovered,
        ]
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
        Ok((value, legs))
    }

    /// Advances a single interval, including past prepayment but only future PV.
    fn project_period<N: Scalar + From<DualFwd>>(
        &self,
        loan: &Loan<N>,
        state: &LoanValuationState,
        period: &LoanPeriod<'_, N>,
        curves: &ProjectionCurves<'_, N>,
        projection: &mut LoanProjection<N>,
        final_payment: bool,
    ) -> Result<()> {
        let opening = projection.balance;
        let principal = projection.principal(period, loan.repayment_structure(), final_payment)?;
        let remaining = opening.sub_val(principal).max_val(N::zero());
        let survival = curves.survival(period.payment)?;
        if !survival.value().is_finite()
            || survival.value() < 0.0
            || survival.value() > projection.survival.value() + 1e-10
        {
            return Err(QSError::InvalidValueErr(
                "Loan survival must be nonnegative and nonincreasing".into(),
            ));
        }
        let probability =
            self.prepayment_probability(loan, state, period, remaining.mul_val(projection.factor))?;
        let weighted = projection.factor.mul_val(survival);
        let interest = opening
            .mul_val(period.rate)
            .mul_val(N::scalar(period.fraction))
            .mul_val(weighted);
        let redemption = principal.mul_val(weighted);
        let prepayment = remaining.mul_val(probability).mul_val(weighted);
        let recovery = projection
            .survival
            .sub_val(survival)
            .mul_val(opening)
            .mul_val(N::scalar(loan.recovery_fraction()))
            .mul_val(projection.factor);
        projection.value = projection.value.add_val(
            curves.present_value(
                period.payment,
                interest
                    .add_val(redemption)
                    .add_val(prepayment)
                    .add_val(recovery),
            )?,
        );
        if projection.report_cashflows {
            projection.report(period, opening, redemption, prepayment, recovery, weighted)?;
        }
        projection.balance = remaining;
        projection.factor = projection.factor.mul_val(N::one().sub_val(probability));
        projection.survival = survival;
        Ok(())
    }

    /// Converts the model's origination-based interval probability to expected principal.
    fn prepayment_probability<N: Scalar + From<DualFwd>>(
        &self,
        loan: &Loan<N>,
        state: &LoanValuationState,
        period: &LoanPeriod<'_, N>,
        remaining: N,
    ) -> Result<N> {
        if remaining.value() == 0.0 {
            return Ok(N::zero());
        }
        let benchmark = if loan.prepayment_model().requires_refinancing_benchmark() {
            self.observe_refinancing_benchmark_rate(state, period.start)?
        } else {
            N::zero()
        };
        let fraction = if period.start.advance(1, TimeUnit::Months)? == period.end {
            1.0 / 12.0
        } else {
            DayCounter::Actual365.year_fraction(period.start, period.end)
        };
        let probability = loan.prepayment_model().conditional_prepayment_probability(
            &PrepaymentModelState::new(
                period.start,
                period.end,
                fraction,
                remaining,
                period.rate,
                benchmark,
            ),
        )?;
        if !probability.value().is_finite() || !(0.0..=1.0).contains(&probability.value()) {
            return Err(QSError::InvalidValueErr(
                "Loan prepayment function must return a probability in [0, 1]".into(),
            ));
        }
        Ok(probability)
    }
}

/// Extracts ordered coupon periods and attaches any custom scheduled principal.
fn loan_periods<N: Scalar>(
    cashflows: &[CashflowType<N>],
    custom: bool,
) -> Result<Vec<LoanPeriod<'_, N>>> {
    let mut periods = cashflows
        .iter()
        .filter_map(|cashflow| LoanPeriod::from_coupon(cashflow).transpose())
        .collect::<Result<Vec<_>>>()?;
    for cashflow in cashflows.iter().filter(|_| custom) {
        if let CashflowType::Redemption(principal) = cashflow {
            let position = periods
                .binary_search_by_key(&principal.payment_date(), |period| period.payment)
                .map_err(|_| {
                    QSError::InvalidValueErr("Loan redemption has no matching coupon".into())
                })?;
            periods[position].principal += principal.amount()?;
        }
    }
    Ok(periods)
}

/// Contiguous contractual coupon level and its hypothetical redemption date.
struct LoanCouponSegment {
    coupons: Range<usize>,
    redemption_date: Date,
}

/// Fair-rate objective holding resolved coupons and the discounted advance target.
struct LoanFairRateObjective<'a> {
    pricer: &'a LoanPricer,
    trade: &'a LoanTrade<DualFwd>,
    state: &'a LoanValuationState,
    cashflows: &'a [CashflowType<DualFwd>],
    adjustable_segment: Range<usize>,
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
        self.call(&lower)?;
        if self.call(&upper).is_ok() {
            return Ok(upper);
        }
        let mut valid = lower;
        let mut invalid = upper;
        for _ in 0..48 {
            let candidate = f64::midpoint(valid, invalid);
            if self.call(&candidate).is_ok() {
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
                    .with_day_counter(coupon.day_counter())
                    .with_fixing_date(coupon.fixing_date());
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
        self.rebuild_trial_cashflows(&coupons)
    }

    /// Regenerates the contractual trial schedule from the chosen initial rate.
    fn rebuild_trial_cashflows(
        &self,
        coupons: &[CashflowType<DualFwd>],
    ) -> Result<Vec<CashflowType<DualFwd>>> {
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
                    .with_day_counter(coupon.day_counter())
                    .with_fixing_date(coupon.fixing_date());
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
        let (value, _) = self.pricer.project_expected_receipts(
            self.trade,
            self.state,
            &trial,
            true,
            Some(self.redemption_date),
            false,
        )?;
        Ok(value.value() * self.trade.side().sign() / self.target - 1.0)
    }
}

impl LoanPricer {
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
    fn handle_fair_rate(
        &self,
        trade: &LoanTrade<DualFwd>,
        state: &mut LoanValuationState,
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
    /// Builds labelled EUR discount, forward and credit curves for loan valuation tests.
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
    /// Builds a loan pricer that discounts EUR receipts on ESTR.
    fn estr_loan_pricer() -> LoanPricer {
        let mut pricer = LoanPricer::new();
        pricer.set_discount_policy(Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_discount_curve_index(Currency::EUR, MarketIndex::ESTR),
        ));
        pricer
    }

    #[test]
    fn reset_installments_preserve_reverse_and_mixed_derivatives() -> Result<()> {
        for rate in [-0.02, 0.0, 0.06] {
            for residual in [0.0, 25_000.0] {
                let fractions: Vec<_> = (0..360)
                    .map(|month| [31.0, 28.0, 31.0, 30.0][month % 4] / 360.0)
                    .collect();
                let evaluate = |compact: bool| -> Result<([f64; 8], usize)> {
                    Tape::start_recording_fwd();
                    let principal = DualFwd::new(100_000.0);
                    let residual = DualFwd::new(residual);
                    let rate = DualFwd::new_from_inner(ADForward::var(rate));
                    let payment = if compact {
                        DualFwd::reset_installment(
                            principal,
                            residual,
                            rate,
                            fractions.iter().copied(),
                        )?
                    } else {
                        level_installment(
                            principal,
                            residual,
                            fractions
                                .iter()
                                .map(|fraction| rate.mul_val(DualFwd::scalar(*fraction))),
                        )?
                    };
                    Tape::stop_recording_fwd();
                    payment.backward()?;
                    let derivative = rate.adjoint()?;
                    Ok((
                        [
                            payment.value(),
                            payment.inner().first_derivative(),
                            payment.inner().second_derivative(),
                            derivative.value(),
                            derivative.first_derivative(),
                            derivative.second_derivative(),
                            principal.adjoint()?.value(),
                            residual.adjoint()?.value(),
                        ],
                        ADForward::with_tape(|tape| tape.book.len()),
                    ))
                };
                let (expected, _) = evaluate(false)?;
                let (actual, nodes) = evaluate(true)?;
                for (actual, expected) in actual.into_iter().zip(expected) {
                    assert!(
                        (actual - expected).abs() <= 1e-10 * expected.abs().max(1.0),
                        "rate={rate}, residual={residual}: {actual} != {expected}"
                    );
                }
                assert_eq!(nodes, 4, "one installment node plus three inputs");
            }
        }
        Ok(())
    }

    #[test]
    fn value_and_cashflow_requests_do_not_record_a_sensitivity_graph() -> Result<()> {
        Tape::stop_recording_fwd();
        let provider = build_test_market(0.03, 0.04, 0.0);
        let loan = default_loan_builder()
            .with_prepayment_configuration(PrepaymentModelConfiguration::Cpr(0.12))
            .build()?;
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let pricer = estr_loan_pricer();
        let initial_nodes = ADForward::with_tape(|tape| tape.book.len());
        let value = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        assert_eq!(initial_nodes, ADForward::with_tape(|tape| tape.book.len()));
        let requests = [
            Request::Value,
            Request::Cashflows(true),
            Request::Sensitivities,
        ];
        let risk = pricer.evaluate(&trade, &requests, &provider)?;
        let recorded_nodes = ADForward::with_tape(|tape| tape.book.len());
        assert!(recorded_nodes > 0);
        let numeric = pricer.evaluate(&trade, &requests[..2], &provider)?;
        assert_eq!(recorded_nodes, ADForward::with_tape(|tape| tape.book.len()));
        assert_eq!(value.price(), risk.price());
        assert_eq!(numeric.price(), risk.price());
        assert_eq!(
            numeric.cashflows().map(CashflowsTable::amounts),
            risk.cashflows().map(CashflowsTable::amounts)
        );
        Ok(())
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
        let requests = [Request::Value, Request::Cashflows(true)];
        let estr = pricer.evaluate(&trade, &requests, &provider)?;
        pricer.set_discount_policy(Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_discount_curve_index(Currency::EUR, MarketIndex::EURIBOR12m),
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
    /// Builds the standard EUR loan contract used by receipt and fair-rate tests.
    fn default_loan_builder() -> MakeLoan<DualFwd> {
        MakeLoan::default()
            .with_identifier("LOAN".into())
            .with_start_date(Date::new(2026, 1, 1))
            .with_maturity_date(Date::new(2028, 1, 1))
            .with_notional(100_000.0)
            .with_currency(Currency::EUR)
            .with_coupons(vec![CashflowType::FixedRateCoupon(FixedRateCoupon::new(
                1.0,
                Box::new(InterestRate::new(
                    DualFwd::new(0.06),
                    Compounding::Simple,
                    Frequency::Annual,
                    DayCounter::Actual360,
                )),
                Date::new(1970, 1, 1),
                Date::new(1970, 1, 1),
                Date::new(1970, 1, 1),
            ))])
    }
    /// Evaluates the requested loan position and extracts its receipt PV.
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
                Request::Cashflows(true),
                Request::Cashflows(true),
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
        assert!(table.cashflow_types().iter().all(|kind| matches!(
            kind,
            CashflowKind::FixedRateCoupon | CashflowKind::Redemption
        )));
        assert!(table.leg_indices().contains(&1));
        let principal: f64 = table
            .amounts()
            .iter()
            .zip(table.cashflow_types())
            .filter(|(_, kind)| matches!(kind, CashflowKind::Redemption))
            .map(|(amount, _)| amount)
            .sum();
        assert!((principal - 100_000.0).abs() < 1e-7);
        assert_eq!(
            trade.instrument().contractual_leg().cashflows().len(),
            original
        );
        let short = LoanTrade::new(trade.instrument().clone(), provider.date, Side::PayShort);
        let short_result = pricer.evaluate(&short, &[Request::Cashflows(true)], &provider)?;
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
    fn floating_market_changes_recast_future_principal_and_preserve_known_fixings() -> Result<()> {
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
                &[Request::Cashflows(true)],
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
                    .filter(|(_, kind)| matches!(kind, CashflowKind::Redemption))
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
        assert_eq!(&redemptions[0][..12], &redemptions[1][..12]);
        assert_ne!(&redemptions[0][12..], &redemptions[1][12..]);
        let mut missing = build_test_market(0.03, 0.04, 0.0);
        missing.data = MarketData::new(HashMap::new(), missing.data.constructed_elements().clone());
        assert!(pricer
            .evaluate(&trade, &[Request::Value], &missing)
            .is_err());
        Ok(())
    }

    #[test]
    fn seasoned_cashflows_require_the_history_used_to_replay_amortization() -> Result<()> {
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
        // Historical resets determine the closing balance even when past rows are hidden.
        assert!(pricer
            .evaluate(&trade, &[Request::Cashflows(true)], &provider)
            .is_err());
        let history = HashMap::from([(
            MarketIndex::EURIBOR12m,
            BTreeMap::from([(Date::new(2026, 1, 1), 0.04)]),
        )]);
        provider.data = MarketData::new(history, provider.data.constructed_elements().clone());
        // Today's unpublished fixing may still forecast.
        let result = pricer.evaluate(
            &trade,
            &[Request::Value, Request::Cashflows(true)],
            &provider,
        )?;
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
                .filter(|kind| matches!(kind, CashflowKind::FloatingRateCoupon))
                .count(),
            12
        );
        Ok(())
    }

    fn assert_prefixes_at_par(
        table: &CashflowsTable,
        provider: &Provider,
        side: Side,
        rebuild: bool,
    ) -> Result<()> {
        for segment in 1..=4 {
            let end = provider.date.advance(segment * 6, TimeUnit::Months)?;
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
                        * (-0.03 * DayCounter::Actual365.year_fraction(provider.date, *date)).exp();
                    if matches!(kind, CashflowKind::Redemption) {
                        repaid += amount;
                    }
                }
            }
            prefix_value += (side.sign() * 100_000.0 - repaid)
                * (-0.03 * DayCounter::Actual365.year_fraction(provider.date, end)).exp();
            assert!(
                (prefix_value - side.sign() * 100_000.0).abs() < 1e-5,
                "segment {segment}, rebuild={rebuild}: {prefix_value}"
            );
        }
        Ok(())
    }

    #[test]
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
                    &[Request::Value, Request::Cashflows(true)],
                    &provider,
                )?;
                for rebuild in [false, true] {
                    let pricer = estr_loan_pricer().with_fair_rate_amortization_rebuild(rebuild);
                    let result = pricer.evaluate(
                        &trade,
                        &[
                            Request::FairRate(0),
                            Request::Value,
                            Request::Cashflows(true),
                        ],
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
                        &[Request::Value, Request::Cashflows(true)],
                        &provider,
                    )?;
                    assert!(
                        (par.price().unwrap_or_default() - side.sign() * 100_000.0).abs() < 1e-5
                    );
                    let table = par
                        .cashflows()
                        .ok_or_else(|| QSError::ValueNotSetErr("Par cashflows".into()))?;
                    assert_prefixes_at_par(table, &provider, side, rebuild)?;
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

    fn contractual_redemptions(loan: &Loan<DualFwd>) -> Vec<f64> {
        loan.contractual_leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| {
                if let CashflowType::Redemption(principal) = cashflow {
                    principal.amount().ok()
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
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
            let snapshot = contractual_redemptions(trade.instrument());
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
                let candidate_schedule = contractual_redemptions(candidate.instrument());
                if rebuild {
                    assert_ne!(snapshot, candidate_schedule);
                } else {
                    assert_eq!(snapshot, candidate_schedule);
                }
            }
            let after = contractual_redemptions(trade.instrument());
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
        let result = estr_loan_pricer().evaluate(&trade, &[Request::Cashflows(true)], &provider)?;
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
                &[Request::Cashflows(true)],
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
    #[test]
    fn french_loan_sensitivities_match_independent_amortization() -> Result<()> {
        let start = Date::new(2026, 1, 1);
        let principal = 250_000.0;
        let coupon_rate = 0.06;
        let discount_rate = 0.03;
        // Construct monthly accruals independently of the loan's schedule and cashflow reports.
        let periods = (1..=360)
            .map(|month| {
                let previous = start.advance(month - 1, TimeUnit::Months)?;
                let payment = start.advance(month, TimeUnit::Months)?;
                let rate = coupon_rate * DayCounter::Actual360.year_fraction(previous, payment);
                let time = DayCounter::Actual365.year_fraction(start, payment);
                Ok((rate, time))
            })
            .collect::<Result<Vec<_>>>()?;
        // Solve the terminal balance in f64 using future-value accumulation, without
        // calling level_installment or using the pricer's principal schedule.
        let (growth, annuity) = periods
            .iter()
            .fold((1.0, 0.0), |(growth, annuity), (rate, _)| {
                (growth * (1.0 + rate), annuity * (1.0 + rate) + 1.0)
            });
        let installment = principal * growth / annuity;
        let mut balance = principal;
        let mut expected_pv = 0.0;
        let mut expected_sensitivity = 0.0;
        let mut up_pv = 0.0;
        let mut down_pv = 0.0;
        let bump = 1e-6;
        for (position, &(rate, time)) in periods.iter().enumerate() {
            let interest = balance * rate;
            let redemption = if position + 1 == periods.len() {
                balance
            } else {
                installment - interest
            };
            let receipt = interest + redemption;
            balance -= redemption;
            let discounted = receipt * (-discount_rate * time).exp();
            expected_pv += discounted;
            expected_sensitivity -= time * discounted;
            up_pv += receipt * (-(discount_rate + bump) * time).exp();
            down_pv += receipt * (-(discount_rate - bump) * time).exp();
        }
        let provider = build_test_market(discount_rate, 0.04, 0.0);
        let loan = default_loan_builder()
            .with_maturity_date(Date::new(2056, 1, 1))
            .with_notional(principal)
            .with_repayment_structure(LoanRepaymentStructure::French)
            .with_payment_frequency(Frequency::Monthly)
            .build()?;
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let result = estr_loan_pricer().evaluate(
            &trade,
            &[Request::Value, Request::Sensitivities],
            &provider,
        )?;
        let actual_pv = result
            .price()
            .ok_or_else(|| QSError::ValueNotSetErr("Loan PV".into()))?;
        assert!((actual_pv - expected_pv).abs() < 1e-6);
        let risk = loan_sensitivities_by_label(&result)?;
        assert_eq!(
            risk.keys().map(String::as_str).collect::<Vec<_>>(),
            ["CREDIT", "DISCOUNT", "FORWARD"]
        );
        assert!(expected_sensitivity < -1.0);
        assert!((risk["DISCOUNT"] - expected_sensitivity).abs() < 1e-6);
        let finite_difference = (up_pv - down_pv) / (2.0 * bump);
        assert!((risk["DISCOUNT"] - finite_difference).abs() < expected_sensitivity.abs() * 1e-8);
        assert!(risk["FORWARD"].abs() < 1e-12);
        assert!(risk["CREDIT"].abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn thirty_year_mortgage_risk_matches_full_revaluation() -> Result<()> {
        let pricer = estr_loan_pricer().with_refinancing_benchmark_index(MarketIndex::EURIBOR12m);
        for floating in [false, true] {
            for repayment in [
                LoanRepaymentStructure::French,
                LoanRepaymentStructure::Bullet,
                LoanRepaymentStructure::Balloon {
                    residual_fraction: 0.3,
                },
            ] {
                for prepayment in [
                    PrepaymentModelConfiguration::None,
                    PrepaymentModelConfiguration::Cpr(0.1),
                    PrepaymentModelConfiguration::Refinancing {
                        base_cpr: 0.02,
                        incentive_multiplier: 4.0,
                        maximum_cpr: 0.4,
                    },
                ] {
                    let coupon = if floating {
                        CashflowType::floating(MarketIndex::EURIBOR12m, 0.02, None, None)
                    } else {
                        CashflowType::fixed(0.06)
                    };
                    let loan = default_loan_builder()
                        .with_maturity_date(Date::new(2056, 1, 1))
                        .with_notional(250_000.0)
                        .with_coupons(vec![coupon])
                        .with_initial_amortization_rate(0.06)
                        .with_repayment_structure(repayment.clone())
                        .with_prepayment_configuration(prepayment.clone())
                        .with_credit_curve_index(MarketIndex::Credit("BORROWER".into()))
                        .build()?;
                    let provider = build_test_market(0.03, 0.04, 0.01);
                    let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
                    let result = pricer.evaluate(&trade, &[Request::Sensitivities], &provider)?;
                    let risk = result
                        .sensitivities()
                        .ok_or_else(|| QSError::ValueNotSetErr("Mortgage risk".into()))?;
                    for (label, actual) in risk.instrument_keys().iter().zip(risk.exposure()) {
                        let shifted = |bump: f64| {
                            let market = build_test_market(
                                0.03 + if label == "DISCOUNT" { bump } else { 0.0 },
                                0.04 + if label == "FORWARD" { bump } else { 0.0 },
                                0.01 + if label == "CREDIT" { bump } else { 0.0 },
                            );
                            evaluate_loan_receipt_present_value(&pricer, &trade, &market)
                        };
                        let expected = (shifted(1e-6)? - shifted(-1e-6)?) / 2e-6;
                        assert!(
                            (actual - expected).abs() < (expected.abs() * 2e-6).max(0.01),
                            "floating={floating}, {repayment:?}, {prepayment:?}, {label}: AD {actual}, FD {expected}"
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Builds five separately identified positions with identical French loan terms.
    fn five_identical_french_loans() -> Result<Vec<LoanTrade<DualFwd>>> {
        let builder = default_loan_builder()
            .with_maturity_date(Date::new(2056, 1, 1))
            .with_notional(250_000.0)
            .with_repayment_structure(LoanRepaymentStructure::French)
            .with_payment_frequency(Frequency::Monthly);
        (1..=5)
            .map(|position| {
                builder
                    .clone()
                    .with_identifier(format!("FRENCH-{position}"))
                    .build()
                    .map(|loan| LoanTrade::new(loan, Date::new(2026, 1, 1), Side::LongReceive))
            })
            .collect()
    }

    /// Extracts a nonempty map and checks that sensitivity labels are unique and aligned.
    fn loan_sensitivities_by_label(result: &EvaluationResults) -> Result<BTreeMap<String, f64>> {
        let risk = result
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Loan risk".into()))?;
        assert!(!risk.instrument_keys().is_empty());
        assert_eq!(risk.instrument_keys().len(), risk.exposure().len());
        let exposures = risk
            .instrument_keys()
            .iter()
            .cloned()
            .zip(risk.exposure().iter().copied())
            .collect::<BTreeMap<_, _>>();
        assert_eq!(exposures.len(), risk.instrument_keys().len());
        Ok(exposures)
    }

    /// Sums labelled exposures from separate loan evaluations without a portfolio accumulator.
    fn sum_individual_loan_sensitivities(
        loans: &[LoanTrade<DualFwd>],
        pricer: &LoanPricer,
        provider: &Provider,
    ) -> Result<BTreeMap<String, f64>> {
        let mut total = BTreeMap::new();
        for loan in loans {
            let result = pricer.evaluate(loan, &[Request::Sensitivities], provider)?;
            for (label, exposure) in loan_sensitivities_by_label(&result)? {
                *total.entry(label).or_insert(0.0) += exposure;
            }
        }
        Ok(total)
    }

    #[test]
    fn five_french_loans_have_five_times_single_loan_sensitivities() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let pricer = estr_loan_pricer();
        let loans = five_identical_french_loans()?;
        assert_eq!(loans.len(), 5);
        let single = pricer.evaluate(&loans[0], &[Request::Sensitivities], &provider)?;
        let single = loan_sensitivities_by_label(&single)?;
        assert!(single["DISCOUNT"] < -1.0);
        let total = sum_individual_loan_sensitivities(&loans, &pricer, &provider)?;
        assert!(total.keys().eq(single.keys()));
        for (label, exposure) in single {
            let expected = 5.0 * exposure;
            assert!(
                (total[&label] - expected).abs() < 1e-6,
                "{label}: five loans {}, expected {expected}",
                total[&label]
            );
        }
        Ok(())
    }

    #[test]
    fn french_loan_portfolio_sensitivities_match_sum_of_five_individual_loans() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let pricer = estr_loan_pricer();
        let loans = five_identical_french_loans()?;
        assert_eq!(loans.len(), 5);
        let expected = sum_individual_loan_sensitivities(&loans, &pricer, &provider)?;
        let portfolio = Portfolio::new("FIVE-FRENCH-LOANS".into(), loans)?;
        let result = PortfolioPricer::new(pricer).evaluate(
            &portfolio,
            &[Request::Sensitivities],
            &provider,
        )?;
        let actual = loan_sensitivities_by_label(&result)?;
        assert!(actual.keys().eq(expected.keys()));
        for (label, exposure) in expected {
            assert!(
                (actual[&label] - exposure).abs() < 1e-6,
                "{label}: portfolio {}, individual sum {exposure}",
                actual[&label]
            );
        }
        Ok(())
    }

    #[test]
    fn long_and_short_same_french_loan_portfolio_has_zero_sensitivities() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let loan = default_loan_builder()
            .with_maturity_date(Date::new(2056, 1, 1))
            .with_notional(250_000.0)
            .with_repayment_structure(LoanRepaymentStructure::French)
            .build()?;
        let long = LoanTrade::new(loan.clone(), provider.date, Side::LongReceive);
        let short = LoanTrade::new(loan, provider.date, Side::PayShort);
        let requests = [Request::Value, Request::Sensitivities];
        let single = estr_loan_pricer().evaluate(&long, &requests, &provider)?;
        let single_risk = loan_sensitivities_by_label(&single)?;
        assert!(single_risk["DISCOUNT"].abs() > 1.0);
        for trades in [vec![long.clone(), short.clone()], vec![short, long]] {
            let portfolio = Portfolio::new("LONG-SHORT-FRENCH-LOAN".into(), trades)?;
            let result = PortfolioPricer::new(estr_loan_pricer())
                .evaluate(&portfolio, &requests, &provider)?;
            let risk = loan_sensitivities_by_label(&result)?;
            assert!(risk.keys().eq(single_risk.keys()));
            for (label, exposure) in risk {
                assert!(exposure.abs() < 1e-8, "{label}: net sensitivity {exposure}");
            }
            let value = result
                .price()
                .ok_or_else(|| QSError::ValueNotSetErr("Portfolio PV".into()))?;
            assert!(value.abs() < 1e-8, "Net portfolio PV: {value}");
        }
        Ok(())
    }

    #[test]
    fn repeated_mortgage_sensitivity_requests_do_not_accumulate_adjoints() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let loan = default_loan_builder()
            .with_maturity_date(Date::new(2056, 1, 1))
            .with_notional(250_000.0)
            .build()?;
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let pricer = estr_loan_pricer();
        let single = pricer.evaluate(&trade, &[Request::Sensitivities], &provider)?;
        let exposures = |result: &EvaluationResults| -> Result<BTreeMap<String, f64>> {
            let risk = result
                .sensitivities()
                .ok_or_else(|| QSError::ValueNotSetErr("Mortgage risk".into()))?;
            Ok(risk
                .instrument_keys()
                .iter()
                .cloned()
                .zip(risk.exposure().iter().copied())
                .collect())
        };
        for repetitions in [2, 3] {
            let mut requests = vec![Request::Value];
            requests.extend(std::iter::repeat_n(Request::Sensitivities, repetitions));
            let repeated = pricer.evaluate(&trade, &requests, &provider)?;
            assert_eq!(exposures(&single)?, exposures(&repeated)?);
        }
        Ok(())
    }

    #[test]
    fn unsupported_loan_requests_leave_fields_absent_without_loading_market_data() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let trade = LoanTrade::new(
            default_loan_builder().build()?,
            provider.date,
            Side::LongReceive,
        );
        let result = LoanPricer::new().evaluate(
            &trade,
            &[
                Request::YieldToMaturity,
                Request::ModifiedDuration,
                Request::ZSpread,
                Request::OAS,
            ],
            &provider,
        )?;
        assert!(
            result.price().is_none()
                && result.ytm().is_none()
                && result.modified_duration().is_none()
                && result.z_spread().is_none()
                && result.oas().is_none()
        );
        Ok(())
    }

    #[test]
    fn accrued_request_uses_projected_prepayment_notional_once() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let loan = default_loan_builder()
            .with_repayment_structure(LoanRepaymentStructure::Bullet)
            .with_prepayment_configuration(PrepaymentModelConfiguration::Cpr(0.12))
            .build()?;
        let trade = LoanTrade::new(loan, provider.date, Side::PayShort);
        let start = Date::new(2026, 2, 1);
        let end = Date::new(2026, 3, 1);
        let result = estr_loan_pricer().evaluate(
            &trade,
            &[Request::AccruedAmount(start, end)],
            &provider,
        )?;
        let principal = 100_000.0 * 0.88_f64.powf(1.0 / 12.0);
        let expected = -principal * 0.06 * 28.0 / 360.0;
        assert!((result.accrued_amount().unwrap_or_default() - expected).abs() < 1e-8);
        assert!(result.price().is_none() && result.cashflows().is_none());
        Ok(())
    }

    #[test]
    fn accrual_reuses_the_projection_in_any_request_order() -> Result<()> {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        let provider = build_test_market(0.03, 0.04, 0.0);
        let calls = Arc::new(AtomicUsize::new(0));
        let model_calls = Arc::clone(&calls);
        let loan = default_loan_builder()
            .with_custom_prepayment_model(move |_: &PrepaymentModelState<DualFwd>| {
                model_calls.fetch_add(1, Ordering::Relaxed);
                Ok(DualFwd::scalar(0.01))
            })
            .build()?;
        let trade = LoanTrade::new(loan, provider.date, Side::PayShort);
        let pricer = estr_loan_pricer();
        let accrual = Request::AccruedAmount(Date::new(2026, 2, 1), Date::new(2026, 3, 1));
        let standalone = pricer.evaluate(&trade, &[accrual], &provider)?;
        let expected_calls = calls.load(Ordering::Relaxed);
        assert!(expected_calls > 0);
        let expected_amount = standalone
            .accrued_amount()
            .ok_or_else(|| QSError::ValueNotSetErr("Accrued amount".into()))?;
        for requests in [
            vec![accrual, Request::Cashflows(true)],
            vec![Request::Cashflows(true), accrual],
            vec![accrual, Request::Sensitivities, Request::Cashflows(true)],
            vec![Request::Value, accrual, Request::Cashflows(true)],
        ] {
            calls.store(0, Ordering::Relaxed);
            let result = pricer.evaluate(&trade, &requests, &provider)?;
            let amount = result
                .accrued_amount()
                .ok_or_else(|| QSError::ValueNotSetErr("Accrued amount".into()))?;
            assert!((amount - expected_amount).abs() < 1e-8);
            assert_eq!(calls.load(Ordering::Relaxed), expected_calls);
            assert!(result.cashflows().is_some());
            assert_eq!(result.price().is_some(), requests.contains(&Request::Value));
        }
        Ok(())
    }

    #[test]
    fn generic_accrual_accepts_f64_contracts_and_f64_prepayment_models() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let configuration = PrepaymentModelConfiguration::Cpr(0.12);
        let trade = LoanTrade::new(
            default_loan_builder()
                .with_repayment_structure(LoanRepaymentStructure::Bullet)
                .with_prepayment_configuration(configuration.clone())
                .build()?,
            provider.date,
            Side::LongReceive,
        );
        let numeric: LoanTrade<f64> = trade
            .clone()
            .into_scalar(std::sync::Arc::new(configuration));
        let pricer = estr_loan_pricer();
        let mut state = pricer.load_loan_valuation_state(&trade, &provider)?;
        let start = Date::new(2026, 2, 1);
        let end = Date::new(2026, 3, 1);
        let amount = pricer.handle_accrued_amount(&numeric, &mut state, start, end)?;
        let expected = pricer
            .evaluate(&trade, &[Request::AccruedAmount(start, end)], &provider)?
            .accrued_amount()
            .unwrap_or_default();
        assert!((amount - expected).abs() < 1e-10);
        assert!(state.value.is_none());
        Ok(())
    }

    #[test]
    fn numeric_accrual_preserves_the_tape_and_existing_custom_models() -> Result<()> {
        use crate::ad::forward::TAPE_FWD;
        let provider = build_test_market(0.03, 0.04, 0.0);
        let loan = default_loan_builder()
            .with_repayment_structure(LoanRepaymentStructure::Bullet)
            .with_custom_prepayment_model(|_: &PrepaymentModelState<DualFwd>| {
                assert!(!TAPE_FWD.with(|tape| tape.borrow().active));
                Ok(DualFwd::scalar(0.01))
            })
            .build()?;
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        Tape::start_recording_fwd();
        let sentinel = DualFwd::scalar(1.0);
        Tape::set_mark_fwd();
        Tape::stop_recording_fwd();
        let before = TAPE_FWD.with(|tape| {
            let tape = tape.borrow();
            (tape.book.len(), tape.mark, tape.active)
        });
        let result = estr_loan_pricer().evaluate(
            &trade,
            &[Request::AccruedAmount(
                Date::new(2026, 2, 1),
                Date::new(2026, 3, 1),
            )],
            &provider,
        )?;
        assert!(
            (result.accrued_amount().unwrap_or_default() - 100_000.0 * 0.99 * 0.06 * 28.0 / 360.0)
                .abs()
                < 1e-8
        );
        let after = TAPE_FWD.with(|tape| {
            let tape = tape.borrow();
            (tape.book.len(), tape.mark, tape.active)
        });
        assert_eq!(before, after);
        for from_evaluation_date in [false, true] {
            let report = estr_loan_pricer().evaluate(
                &trade,
                &[Request::Cashflows(from_evaluation_date)],
                &provider,
            )?;
            assert!(report.cashflows().is_some() && report.price().is_none());
            let after_report = TAPE_FWD.with(|tape| {
                let tape = tape.borrow();
                (tape.book.len(), tape.mark, tape.active)
            });
            assert_eq!(before, after_report);
        }
        assert_eq!(sentinel.value(), 1.0);
        Ok(())
    }

    #[test]
    fn numeric_accrual_matches_projected_coupon_receipts_and_preserves_risk() -> Result<()> {
        for coupon in [
            CashflowType::fixed(0.06),
            CashflowType::floating(MarketIndex::EURIBOR12m, 0.01, None, None),
            CashflowType::floating(MarketIndex::EURIBOR12m, 0.01, Some(0.02), Some(0.045)),
        ] {
            let provider = build_test_market(0.03, 0.04, 0.01);
            let loan = default_loan_builder()
                .with_coupons(vec![coupon])
                .with_repayment_structure(LoanRepaymentStructure::Bullet)
                .with_credit_curve_index(MarketIndex::Credit("BORROWER".into()))
                .with_prepayment_configuration(PrepaymentModelConfiguration::Refinancing {
                    base_cpr: 0.02,
                    incentive_multiplier: 4.0,
                    maximum_cpr: 0.4,
                })
                .build()?;
            let trade = LoanTrade::new(loan, provider.date, Side::PayShort);
            let pricer =
                estr_loan_pricer().with_refinancing_benchmark_index(MarketIndex::EURIBOR12m);
            let request = Request::AccruedAmount(provider.date, Date::new(2028, 1, 1));
            let numeric = pricer.evaluate(&trade, &[request], &provider)?;
            for requests in [
                [request, Request::Sensitivities, Request::Cashflows(true)],
                [Request::Sensitivities, request, Request::Cashflows(true)],
            ] {
                let result = pricer.evaluate(&trade, &requests, &provider)?;
                assert!(result.sensitivities().is_some());
                let mut state = pricer.load_loan_valuation_state(&trade, &provider)?;
                pricer.handle_value(&trade, &mut state)?;
                let legs = state
                    .legs
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect::<Vec<_>>();
                let expected = accrued_amount(
                    &legs,
                    state.get_market_data_reponse(),
                    provider.date,
                    Date::new(2028, 1, 1),
                )?;
                assert!((numeric.accrued_amount().unwrap_or_default() - expected).abs() < 1e-8);
                assert_eq!(numeric.accrued_amount(), result.accrued_amount());
            }
        }
        Ok(())
    }

    /// Groups scheduled interest and principal by payment date.
    fn scheduled_installments(table: &CashflowsTable) -> BTreeMap<Date, f64> {
        let mut payments = BTreeMap::new();
        for row in table
            .rows()
            .filter(|row| row.leg_index == 0 && !matches!(row.kind, CashflowKind::Disbursement))
        {
            *payments.entry(row.payment_date).or_default() += row.amount;
        }
        payments
    }

    /// Checks a payment block independently of its level.
    fn assert_level(payments: &[f64]) {
        assert!(payments.len() > 1);
        let first = payments[0];
        assert!(
            payments
                .iter()
                .all(|payment| (payment - first).abs() < 1e-7),
            "nonconstant installments: {payments:?}"
        );
    }

    #[test]
    fn french_stub_installments_are_level_on_actual_accrual_fractions() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let cases = [
            default_loan_builder()
                .with_start_date(Date::new(2026, 1, 15))
                .with_first_coupon_date(Date::new(2026, 2, 1)),
            default_loan_builder()
                .with_maturity_date(Date::new(2028, 1, 15))
                .with_next_to_last_coupon_date(Date::new(2028, 1, 1)),
            default_loan_builder()
                .with_start_date(Date::new(2026, 1, 15))
                .with_date_generation_rule(DateGenerationRule::Backward),
        ];
        for maker in cases {
            let trade = LoanTrade::new(maker.build()?, provider.date, Side::LongReceive);
            let result =
                estr_loan_pricer().evaluate(&trade, &[Request::Cashflows(true)], &provider)?;
            let table = result
                .cashflows()
                .ok_or_else(|| QSError::ValueNotSetErr("Cashflows".into()))?;
            assert_level(
                &scheduled_installments(table)
                    .values()
                    .copied()
                    .collect::<Vec<_>>(),
            );
            let principal: f64 = table
                .rows()
                .filter(|row| matches!(row.kind, CashflowKind::Redemption))
                .map(|row| row.amount)
                .sum();
            assert!((principal - 100_000.0).abs() < 1e-7);
        }
        Ok(())
    }

    #[test]
    fn french_installments_reset_once_then_remain_level_for_floating_and_stepped_rates(
    ) -> Result<()> {
        let provider = build_test_market(0.03, 0.08, 0.0);
        let floating = default_loan_builder()
            .with_coupons(vec![CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.01,
                None,
                None,
            )])
            .with_initial_amortization_rate(0.05)
            .with_fixing_lag_business_days(2);
        let mut stepped = vec![CashflowType::fixed(0.04); 12];
        stepped.push(CashflowType::fixed(0.07));
        for maker in [floating, default_loan_builder().with_coupons(stepped)] {
            let trade = LoanTrade::new(maker.build()?, provider.date, Side::LongReceive);
            let result =
                estr_loan_pricer().evaluate(&trade, &[Request::Cashflows(true)], &provider)?;
            let table = result
                .cashflows()
                .ok_or_else(|| QSError::ValueNotSetErr("Cashflows".into()))?;
            let payments: Vec<_> = scheduled_installments(table).values().copied().collect();
            assert_eq!(payments.len(), 24);
            assert_level(&payments[..12]);
            assert_level(&payments[12..]);
            assert!((payments[12] - payments[11]).abs() > 1.0);
        }
        Ok(())
    }

    #[test]
    fn seasoned_prepayment_replays_origination_and_report_window_does_not_change_value(
    ) -> Result<()> {
        let mut provider = build_test_market(0.03, 0.04, 0.0);
        let loan = default_loan_builder()
            .with_prepayment_configuration(PrepaymentModelConfiguration::Cpr(0.12))
            .build()?;
        let later = Date::new(2027, 1, 1);
        let expected = loan.contractual_principal(later)? * 0.88;
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let pricer = estr_loan_pricer();
        let original = pricer.evaluate(&trade, &[Request::Cashflows(false)], &provider)?;
        provider.date = later;
        let full = pricer.evaluate(
            &trade,
            &[Request::Value, Request::Cashflows(false)],
            &provider,
        )?;
        let future = pricer.evaluate(
            &trade,
            &[Request::Cashflows(true), Request::Value],
            &provider,
        )?;
        let original = original
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Cashflows".into()))?;
        let full_table = full
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Cashflows".into()))?;
        let future_table = future
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Cashflows".into()))?;
        assert_eq!(original.amounts(), full_table.amounts());
        assert_eq!(original.payment_dates(), full_table.payment_dates());
        assert_eq!(full.price(), future.price());
        assert!(full_table
            .cashflow_types()
            .contains(&CashflowKind::Disbursement));
        assert!(future_table
            .payment_dates()
            .iter()
            .all(|date| *date > later));
        let principal: f64 = future_table
            .rows()
            .filter(|row| matches!(row.kind, CashflowKind::Redemption))
            .map(|row| row.amount)
            .sum();
        assert!((principal - expected).abs() < 1e-7);
        Ok(())
    }

    #[test]
    fn monthly_coupons_report_their_shared_annual_fixing_date() -> Result<()> {
        let provider = build_test_market(0.03, 0.04, 0.0);
        let loan = default_loan_builder()
            .with_coupons(vec![CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.01,
                None,
                None,
            )])
            .with_initial_amortization_rate(0.05)
            .with_fixing_lag_business_days(2)
            .build()?;
        let trade = LoanTrade::new(loan, provider.date, Side::LongReceive);
        let result =
            estr_loan_pricer().evaluate(&trade, &[Request::Cashflows(false)], &provider)?;
        let table = result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Cashflows".into()))?;
        let dates: Vec<_> = table
            .rows()
            .filter(|row| matches!(row.kind, CashflowKind::FloatingRateCoupon))
            .map(|row| row.fixing_date)
            .collect();
        assert_eq!(&dates[..12], &[Some(Date::new(2025, 12, 30)); 12]);
        assert_eq!(&dates[12..], &[Some(Date::new(2026, 12, 30)); 12]);
        assert!(table
            .rows()
            .filter(|row| matches!(
                row.kind,
                CashflowKind::Redemption | CashflowKind::Disbursement
            ))
            .all(|row| row.fixing_date.is_none()));
        Ok(())
    }
}

#[cfg(test)]
mod resolution_tests {
    use crate::{
        core::makeinstrument::test_support::{assert_legs, cashflows, loan, market, p, reference},
        prelude::*,
    };

    #[test]
    fn loan_resolution_forecasts_initial_coupon_and_matches_explicit_pricing_and_risk() -> Result<()>
    {
        let market = market(reference(), 0.04, FixingStore::default());
        let ctx = &market;
        let maker = loan()?;
        let resolved = maker.build_using(ctx)?;
        let explicit = maker
            .clone()
            .with_start_date(reference())
            .with_maturity_date(reference() + p("2Y")?)
            .with_initial_amortization_rate(0.05)
            .build()?;
        // Check individual principal balances with a tolerance for implied-rate roundoff.
        let balances = |loan: &Loan<DualFwd>| {
            loan.contractual_leg()
                .cashflows()
                .iter()
                .filter_map(|c| match c {
                    CashflowType::FloatingRateCoupon(c) => Some(c.notional()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        for (a, b) in balances(&resolved).iter().zip(balances(&explicit)) {
            assert!((a - b).abs() < 1e-8);
        }
        let mut pricer = LoanPricer::new();
        pricer.set_discount_policy(Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_discount_curve_index(Currency::EUR, MarketIndex::ESTR),
        ));
        let requests = [Request::Value, Request::Sensitivities];
        let auto = pricer.evaluate(
            &LoanTrade::new(resolved, reference(), Side::LongReceive),
            &requests,
            &market,
        )?;
        let manual = pricer.evaluate(
            &LoanTrade::new(explicit, reference(), Side::LongReceive),
            &requests,
            &market,
        )?;
        assert!(
            (auto
                .price()
                .ok_or_else(|| QSError::ValueNotSetErr("PV".into()))?
                - manual
                    .price()
                    .ok_or_else(|| QSError::ValueNotSetErr("PV".into()))?)
            .abs()
                < 1e-8
        );
        let auto_risk = auto
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Risk".into()))?;
        let manual_risk = manual
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Risk".into()))?;
        let auto_risk = auto_risk
            .instrument_keys()
            .iter()
            .zip(auto_risk.exposure())
            .collect::<std::collections::BTreeMap<_, _>>();
        let manual_risk = manual_risk
            .instrument_keys()
            .iter()
            .zip(manual_risk.exposure())
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            auto_risk.keys().collect::<Vec<_>>(),
            manual_risk.keys().collect::<Vec<_>>()
        );
        for (key, value) in auto_risk {
            assert!((value - manual_risk[key]).abs() < 1e-7);
        }
        assert!(maker.build().is_err());
        Ok(())
    }

    #[test]
    fn fixing_policy_controls_historical_resolution_and_pricing() -> Result<()> {
        let start = reference();
        let evaluation = Date::new(2026, 3, 10);
        let maker = loan()?.with_start_date(start);
        let missing = market(start, 0.04, FixingStore::default())
            .with_quote_store(QuoteStore::new(evaluation));
        assert!(maker.build_using(&missing).is_err());
        let projected = missing.with_anchor_fixings(false);
        let auto = maker.build_using(&projected)?;
        let explicit = maker
            .clone()
            .with_maturity_date(start + p("2Y")?)
            .with_initial_amortization_rate(0.05)
            .build()?;
        assert_legs(&auto, &explicit)?;
        let mut fixings = FixingStore::default();
        fixings.add_fixing(&MarketIndex::EURIBOR12m, start, 0.10);
        let stored = market(start, 0.04, fixings).with_quote_store(QuoteStore::new(evaluation));
        let anchored = maker.build_using(&stored)?;
        assert_ne!(
            cashflows(auto.contractual_leg())?,
            cashflows(anchored.contractual_leg())?
        );
        let projection_only = stored.clone().with_anchor_fixings(false);
        let override_anchored = maker.build_using(&projection_only.with_anchor_fixings(true))?;
        assert_eq!(
            cashflows(anchored.contractual_leg())?,
            cashflows(override_anchored.contractual_leg())?
        );
        let ignored = maker.build_using(&stored.clone().with_anchor_fixings(false))?;
        assert_eq!(
            cashflows(auto.contractual_leg())?,
            cashflows(ignored.contractual_leg())?
        );
        let mut pricer = LoanPricer::new();
        pricer.set_discount_policy(Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_discount_curve_index(Currency::EUR, MarketIndex::ESTR),
        ));
        for market in [&stored, &projected] {
            let trade = LoanTrade::new(maker.build_using(market)?, start, Side::LongReceive);
            assert!(pricer.evaluate(&trade, &[Request::Value], market).is_ok());
        }
        let requests = MarketDataRequest::default()
            .with_fixings_request(vec![FixingRequest::new(MarketIndex::EURIBOR12m, start)]);
        assert!(projected.handle_request(&requests).is_err());
        assert!(!projected
            .handle_request(&MarketDataRequest::default())?
            .anchor_fixings());
        Ok(())
    }
}

//! Bond pricing, quote anchoring, settlement and request handling.
use super::bondanalytics::{
    bracketed_root, quoted_dirty_price, solve_yield, yield_price_and_derivative,
};
use crate::rates::yieldtermstructure::primalcurve::PrimalCurve;
use crate::{
    ad::{dual::DualFwd, scalar::Scalar, tape::Tape},
    core::{
        collateral::{DiscountPolicy, Discountable},
        evaluationresults::{EvaluationResults, SensitivityMap},
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            fixingrequest::FixingRequest,
            forwardraterequest::ForwardRateRequest,
            fxrequest::FxRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricer::Pricer,
        pricerconfiguration::InstrumentType,
        pricerstate::PricerState,
        request::{accrued_amount, HandleAccruedAmount, HandleCashflows, LegsProvider, Request},
    },
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::{cashflow::Cashflow, cashflowtype::CashflowType, leg::Leg},
        fixedincome::{
            bondpricinganchor::BondPricingAnchor, bondterms::BondTerms,
            callschedule::CallScheduleEntry, fixedratebond::FixedRateBondTrade,
        },
    },
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};
use std::collections::HashSet;

/// Resolves the bond leg discount index through the required discount policy.
fn discount_index(
    policy: Option<&dyn DiscountPolicy>,
    trade: &impl BondTerms<DualFwd>,
) -> Result<MarketIndex> {
    policy
        .ok_or_else(|| QSError::ValueNotSetErr("Bond discount policy".into()))?
        .accept(trade.bond_leg())
}

#[derive(Clone, Copy)]
pub(super) struct ResolvedCashflow<T> {
    pub(super) payment_date: Date,
    pub(super) amount: T,
}

pub(super) struct BondState {
    value: Option<DualFwd>,
    market_data: Option<MarketData>,
    legs: Vec<Leg<DualFwd>>,
    evaluation_date: Date,
    settlement_date: Date,
    discount_index: MarketIndex,
    spread_day_counter: DayCounter,
    resolved_spread: f64,
    spread_node: Option<DualFwd>,
    spread_label: String,
}

impl PricerState for BondState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl LegsProvider<DualFwd> for BondState {
    fn legs(&self) -> &[Leg<DualFwd>] {
        &self.legs
    }
}

/// Clones a contractual leg and installs historical or projected fixings on
/// every floating coupon.
fn resolve_leg<T: Scalar + From<DualFwd>>(
    leg: &Leg<T>,
    market_data: &MarketData,
    evaluation_date: Date,
    window: Option<(Date, Date)>,
) -> Result<Leg<T>> {
    let mut resolved = leg.clone();
    for cashflow in resolved.cashflows_mut() {
        let (index, start, end) = match cashflow {
            CashflowType::FloatingRateCoupon(coupon) => (
                coupon.market_index().clone(),
                coupon.accrual_start_date(),
                coupon.accrual_end_date(),
            ),
            CashflowType::OptionEmbeddedCoupon(coupon) => (
                coupon.market_index().clone(),
                coupon.accrual_start_date(),
                coupon.accrual_end_date(),
            ),
            _ => continue,
        };
        if window.is_some_and(|(left, right)| left == right || right <= start || left >= end) {
            continue;
        }
        let fixing = match cashflow {
            CashflowType::FloatingRateCoupon(coupon)
                if coupon.overnight_conventions().is_some() =>
            {
                let curve = market_data
                    .constructed_elements()
                    .discount_curve(&index)
                    .ok_or_else(|| {
                        QSError::NotFoundErr(format!("Forward curve not found for {index}"))
                    })?
                    .curve();
                coupon.overnight_rate_with_fixing_policy(
                    evaluation_date,
                    market_data.anchor_fixings(),
                    |date| curve.discount_factor(date).map(T::from),
                    |date| {
                        Ok(market_data
                            .fixings()
                            .get(&index)
                            .and_then(|fixings| fixings.get(&date))
                            .copied())
                    },
                )?
            }
            _ => T::from(
                ForwardRateRequest::new(index.clone(), start)
                    .with_start_date(start)
                    .with_end_date(end)
                    .resolve_index_rate(
                        market_data,
                        evaluation_date,
                        market_data.anchor_fixings(),
                    )?,
            ),
        };
        match cashflow {
            CashflowType::FloatingRateCoupon(coupon) => coupon.set_fixing(fixing),
            CashflowType::OptionEmbeddedCoupon(coupon) => coupon.set_fixing(fixing),
            _ => {}
        }
    }
    Ok(resolved)
}

/// Resolves contractual earned interest without quote solving or a pricing model.
fn bond_accrued_amount(
    trade: &impl BondTerms<DualFwd>,
    market_data: &MarketData,
    evaluation_date: Date,
    start: Date,
    end: Date,
) -> Result<f64> {
    let leg: Leg<f64> = trade
        .bond_leg()
        .clone()
        .with_side(trade.position_side())
        .into();
    let resolved = resolve_leg(&leg, market_data, evaluation_date, Some((start, end)))?;
    accrued_amount(&[resolved], Some(market_data), start, end)
}

impl<E: BondPricingEngine, T: BondTerms<DualFwd>> HandleAccruedAmount<T, BondState> for E {
    fn handle_accrued_amount(
        &self,
        trade: &T,
        state: &mut BondState,
        start: Date,
        end: Date,
    ) -> Result<f64> {
        let market = state
            .market_data
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Bond market data".into()))?;
        bond_accrued_amount(trade, market, state.evaluation_date, start, end)
    }
}

/// Collects resolved coupons and redemptions strictly after settlement.
fn remaining_cashflows(
    leg: &Leg<DualFwd>,
    settlement_date: Date,
) -> Result<Vec<ResolvedCashflow<DualFwd>>> {
    let mut cashflows = Vec::new();
    for cashflow in leg.cashflows() {
        let resolved = match cashflow {
            CashflowType::FixedRateCoupon(coupon) if coupon.payment_date() > settlement_date => {
                Some(ResolvedCashflow {
                    payment_date: coupon.payment_date(),
                    amount: coupon.amount()?,
                })
            }
            CashflowType::FloatingRateCoupon(coupon)
                if coupon.payment_date() > settlement_date =>
            {
                Some(ResolvedCashflow {
                    payment_date: coupon.payment_date(),
                    amount: coupon.amount()?,
                })
            }
            CashflowType::OptionEmbeddedCoupon(coupon)
                if coupon.payment_date() > settlement_date =>
            {
                Some(ResolvedCashflow {
                    payment_date: coupon.payment_date(),
                    amount: coupon.amount()?,
                })
            }
            CashflowType::Redemption(redemption) if redemption.payment_date() > settlement_date => {
                Some(ResolvedCashflow {
                    payment_date: redemption.payment_date(),
                    amount: DualFwd::scalar(redemption.amount()?),
                })
            }
            CashflowType::FixedRateCoupon(_)
            | CashflowType::FloatingRateCoupon(_)
            | CashflowType::OptionEmbeddedCoupon(_)
            | CashflowType::Redemption(_)
            | CashflowType::Disbursement(_) => None,
            _ => {
                return Err(QSError::InvalidValueErr(
                    "Bond pricing engines support fixed, floating, capped/floored coupons and redemptions only".into(),
                ))
            }
        };
        if let Some(resolved) = resolved {
            cashflows.push(resolved);
        }
    }
    if cashflows.is_empty() {
        return Err(QSError::InvalidValueErr(
            "Bond has no cashflows after settlement".into(),
        ));
    }
    cashflows.sort_by_key(|cashflow| cashflow.payment_date);
    Ok(cashflows)
}

/// Sums principal redemptions that remain outstanding after settlement.
fn outstanding_principal(leg: &Leg<DualFwd>, settlement_date: Date) -> Result<f64> {
    let principal = leg
        .cashflows()
        .iter()
        .filter_map(|cashflow| match cashflow {
            CashflowType::Redemption(redemption) if redemption.payment_date() > settlement_date => {
                Some(redemption.amount())
            }
            _ => None,
        })
        .try_fold(0.0, |total, amount| amount.map(|value| total + value))?;
    if principal <= 0.0 {
        return Err(QSError::InvalidValueErr(
            "Bond has no outstanding principal after settlement".into(),
        ));
    }
    Ok(principal)
}

/// Engine-specific preparation and valuation. Analytics do not dispatch by method.
pub(super) trait BondPricingEngine {
    const INSTRUMENT_TYPE: InstrumentType;
    type Model<T: Scalar>;
    /// Returns the discount policy used by this bond pricing engine.
    fn policy(&self) -> Option<&dyn DiscountPolicy>;
    /// Loads the market inputs and scalar model required by this engine.
    fn prepare(
        &self,
        trade: &impl BondTerms<DualFwd>,
        discount_index: &MarketIndex,
        context: &impl MarketDataProvider,
    ) -> Result<(MarketData, Self::Model<f64>)>;
    /// Rebuilds the prepared model with differentiable parameters for valuation risk.
    fn ad_model(
        &self,
        model: &Self::Model<f64>,
        market: &MarketData,
    ) -> Result<Self::Model<DualFwd>>;
    /// Values the resolved bond cashflows and issuer calls with the supplied model and spread.
    fn value<T: Scalar>(
        &self,
        model: &Self::Model<T>,
        valuation: &BondValuation<'_, T>,
        spread: T,
    ) -> Result<T>;
}

pub(super) struct BondValuation<'a, T: Scalar> {
    pub(super) curve: &'a dyn InterestRatesTermStructure<T>,
    pub(super) cashflows: &'a [ResolvedCashflow<T>],
    pub(super) leg: &'a Leg<DualFwd>,
    pub(super) units: f64,
    pub(super) calls: &'a [CallScheduleEntry],
    pub(super) reference_date: Date,
    pub(super) spread_day_counter: DayCounter,
}

/// Discounts contractual cashflows for a bond without issuer exercise.
pub(super) fn discounted_value<T: Scalar>(
    valuation: &BondValuation<'_, T>,
    spread: T,
) -> Result<T> {
    let reference_discount = valuation.curve.discount_factor(valuation.reference_date)?;
    if reference_discount.value() <= 0.0 {
        return Err(QSError::InvalidValueErr(
            "Discount factor at valuation date must be positive".into(),
        ));
    }
    valuation
        .cashflows
        .iter()
        .try_fold(T::zero(), |value, cashflow| {
            let time = valuation
                .spread_day_counter
                .year_fraction(valuation.reference_date, cashflow.payment_date);
            let discount = valuation
                .curve
                .discount_factor(cashflow.payment_date)?
                .div_val(reference_discount);
            let spread_discount = spread.mul_val(T::scalar(-time)).exp();
            let discounted_amount = cashflow.amount.mul_val(discount).mul_val(spread_discount);
            Ok(value.add_val(discounted_amount))
        })
}

/// Requests contractual discount/forward curves and past coupon fixings.
pub(super) fn contractual_market_request(
    trade: &impl BondTerms<DualFwd>,
    discount_index: Option<&MarketIndex>,
    evaluation_date: Date,
    anchor_fixings: bool,
    window: Option<(Date, Date)>,
) -> Result<MarketDataRequest> {
    let mut indices = HashSet::new();
    let mut elements = Vec::new();
    if let Some(index) = discount_index {
        indices.insert(index.clone());
        elements.push(ConstructedElementRequest::DiscountCurve {
            market_index: index.clone(),
        });
    }
    let mut fixings = Vec::new();
    for cashflow in trade.bond_leg().cashflows() {
        let (index, start, end) = match cashflow {
            CashflowType::FloatingRateCoupon(coupon) => (
                coupon.market_index(),
                coupon.accrual_start_date(),
                coupon.accrual_end_date(),
            ),
            CashflowType::OptionEmbeddedCoupon(coupon) => (
                coupon.market_index(),
                coupon.accrual_start_date(),
                coupon.accrual_end_date(),
            ),
            _ => continue,
        };
        if window.is_some_and(|(left, right)| left == right || right <= start || left >= end) {
            continue;
        }
        if indices.insert(index.clone()) {
            elements.push(ConstructedElementRequest::DiscountCurve {
                market_index: index.clone(),
            });
        }
        if anchor_fixings {
            if let CashflowType::FloatingRateCoupon(coupon) = cashflow {
                if coupon.overnight_conventions().is_some() {
                    fixings.extend(
                        coupon
                            .overnight_fixing_dates(evaluation_date)?
                            .into_iter()
                            .filter(|date| *date < evaluation_date)
                            .map(|date| FixingRequest::new(index.clone(), date)),
                    );
                    continue;
                }
            }
            if start < evaluation_date {
                fixings.push(FixingRequest::new(index.clone(), start));
            }
        }
    }
    Ok(MarketDataRequest::default()
        .with_constructed_elements_request(elements)
        .with_fixings_request(fixings)
        .with_fx_request(vec![FxRequest::single(trade.bond_leg().currency())]))
}
/// Records the engine valuation and the trade's spread as an independent risk factor.
fn build_ad_value<E: BondPricingEngine>(
    engine: &E,
    trade: &impl BondTerms<DualFwd>,
    state: &mut BondState,
    model: &E::Model<f64>,
) -> Result<f64> {
    Tape::start_recording_fwd();
    Tape::set_mark_fwd();
    let result = (|| {
        state.put_pillars_on_tape()?;
        let market = state
            .market_data
            .as_mut()
            .ok_or_else(|| QSError::ValueNotSetErr("Bond market data".into()))?;
        for cube in market
            .constructed_elements_mut()
            .volatility_cubes_mut()
            .values_mut()
        {
            cube.cube_mut().put_pillars_on_tape();
        }
        let ad_model = engine.ad_model(model, market)?;
        let spread = DualFwd::new(state.resolved_spread);
        let resolved_leg = resolve_leg(trade.bond_leg(), market, state.evaluation_date, None)?;
        let cashflows = remaining_cashflows(&resolved_leg, state.settlement_date)?;
        let curve = market
            .constructed_elements()
            .discount_curve(&state.discount_index)
            .ok_or_else(|| QSError::NotFoundErr("Bond discount curve".into()))?
            .curve();
        let valuation = BondValuation {
            curve: &*curve,
            cashflows: &cashflows,
            leg: &resolved_leg,
            units: trade.units(),
            calls: trade.call_schedule(),
            reference_date: state.evaluation_date,
            spread_day_counter: state.spread_day_counter,
        };
        let value = engine
            .value(&ad_model, &valuation, spread)?
            .mul_val(DualFwd::scalar(trade.position_side().sign()));
        drop(curve);
        state.legs = vec![resolved_leg.with_side(trade.position_side())];
        let source = trade.bond_leg().currency();
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
        state.spread_node = Some(spread);
        Ok(value.value())
    })();
    Tape::stop_recording_fwd();
    result
}
/// Collects curve-pillar adjoints and the OAS or Z-spread adjoint in one
/// aggregated sensitivity table.
fn sensitivities(state: &BondState) -> Result<SensitivityMap> {
    let value = state
        .value
        .ok_or_else(|| QSError::ValueNotSetErr("Bond PV".into()))?;
    value.backward_to_mark()?;

    let mut labels = Vec::new();
    let mut exposures = Vec::new();
    let elements = state
        .market_data
        .as_ref()
        .ok_or_else(|| QSError::ValueNotSetErr("Bond market data".into()))?
        .constructed_elements();
    for element in elements.discount_curves().values() {
        for (label, pillar) in element.curve().pillars().into_iter().flatten() {
            labels.push(label);
            exposures.push(pillar.adjoint().map_or(0.0, |adjoint| adjoint.value()));
        }
    }
    for element in elements.volatility_surfaces().values() {
        for (label, pillar) in element.surface().pillars().into_iter().flatten() {
            labels.push(label);
            exposures.push(pillar.adjoint()?.value());
        }
    }
    for element in elements.volatility_cubes().values() {
        for (label, pillar) in element.cube().pillars().into_iter().flatten() {
            labels.push(label);
            exposures.push(pillar.adjoint()?.value());
        }
    }
    let spread = state
        .spread_node
        .ok_or_else(|| QSError::ValueNotSetErr("Bond spread risk factor".into()))?;
    labels.push(state.spread_label.clone());
    exposures.push(spread.adjoint()?.value());
    for (label, exposure) in state.get_fx_sensitivities()? {
        labels.push(label);
        exposures.push(exposure);
    }
    Ok(SensitivityMap::default()
        .with_instrument_keys(&labels)
        .with_exposure(&exposures)
        .aggregate())
}

/// Runs quote conversion, spread solving, valuation, analytics, and risk for
/// either callable-bond numerical engine.
#[allow(clippy::too_many_lines)]
pub(super) fn evaluate_bond<E, T>(
    engine: &E,
    trade: &T,
    requests: &[Request],
    context: &impl MarketDataProvider,
) -> Result<EvaluationResults>
where
    T: BondTerms<DualFwd>,
    E: BondPricingEngine + HandleCashflows<T, BondState>,
{
    let callable = !trade.call_schedule().is_empty();
    let supported: Vec<Request> = requests
        .iter()
        .copied()
        .filter(|request| match request {
            Request::Value
            | Request::Sensitivities
            | Request::Cashflows
            | Request::AccruedAmount(_, _) => true,
            Request::YieldToMaturity | Request::ModifiedDuration => {
                E::INSTRUMENT_TYPE == InstrumentType::FixedRateBond
            }
            Request::OAS => callable,
            Request::ZSpread => !callable,
            Request::FairRate(_) => false,
        })
        .collect();
    let requests = supported.as_slice();
    Request::validate(requests)?;
    if requests.is_empty() {
        return Ok(EvaluationResults::new(
            context.evaluation_date(),
            trade.bond_identifier(),
        ));
    }
    if let [Request::AccruedAmount(start, end)] = requests {
        let evaluation_date = context.evaluation_date();
        let request = contractual_market_request(
            trade,
            None,
            evaluation_date,
            context.anchor_fixings(),
            Some((*start, *end)),
        )?;
        let market = context
            .handle_request(&request)?
            .with_anchor_fixings(context.anchor_fixings());
        return Ok(
            EvaluationResults::new(evaluation_date, trade.bond_identifier()).with_accrued_amount(
                bond_accrued_amount(trade, &market, evaluation_date, *start, *end)?,
            ),
        );
    }
    let anchor = trade.pricing_anchor();
    anchor.validate()?;
    if callable && matches!(anchor, BondPricingAnchor::ZSpread(_)) {
        return Err(QSError::InvalidValueErr(
            "An issuer-call bond must be anchored by clean price, dirty price, or OAS".into(),
        ));
    }
    if !callable && matches!(anchor, BondPricingAnchor::OAS(_)) {
        return Err(QSError::InvalidValueErr(
            "OAS requires a bond with an issuer call schedule".into(),
        ));
    }
    let evaluation_date = context.evaluation_date();
    let settlement_date = trade.settlement_date().unwrap_or(evaluation_date);
    if settlement_date < evaluation_date {
        return Err(QSError::InvalidValueErr(
            "Bond settlement date cannot precede the evaluation date".into(),
        ));
    }
    if callable && settlement_date != evaluation_date {
        return Err(QSError::InvalidValueErr(
            "Callable-bond lattice settlement must equal the context evaluation date".into(),
        ));
    }

    let discount_index = discount_index(engine.policy(), trade)?;
    let (market_data, model) = engine.prepare(trade, &discount_index, context)?;
    let market_data = market_data.with_anchor_fixings(context.anchor_fixings());
    let element = market_data
        .constructed_elements()
        .discount_curve(&discount_index)
        .ok_or_else(|| {
            QSError::NotFoundErr(format!(
                "Discount curve not found for index {discount_index}"
            ))
        })?;
    let curve = element.curve();
    let spread_day_counter = trade
        .spread_day_counter()
        .or_else(|| curve.day_counter())
        .unwrap_or(DayCounter::Actual365);
    let resolved_leg = resolve_leg(trade.bond_leg(), &market_data, evaluation_date, None)?;
    let resolved_cashflows = remaining_cashflows(&resolved_leg, settlement_date)?;
    let cashflows = resolved_cashflows
        .iter()
        .map(|cashflow| ResolvedCashflow {
            payment_date: cashflow.payment_date,
            amount: cashflow.amount.value(),
        })
        .collect::<Vec<_>>();
    let primal_curve = PrimalCurve::new(&*curve);
    let valuation = BondValuation {
        curve: &primal_curve,
        cashflows: &cashflows,
        leg: &resolved_leg,
        units: trade.units(),
        calls: trade.call_schedule(),
        reference_date: settlement_date,
        spread_day_counter,
    };
    let principal = outstanding_principal(&resolved_leg, settlement_date)?;
    let dirty_quote = quoted_dirty_price(
        anchor,
        &resolved_leg,
        trade.units(),
        settlement_date,
        principal,
    )?;
    let target_settlement_value = dirty_quote.map(|price| price * principal / trade.units());
    let resolved_spread = match anchor {
        BondPricingAnchor::CleanPrice(_) | BondPricingAnchor::DirtyPrice(_) => {
            let target = target_settlement_value
                .ok_or_else(|| QSError::ValueNotSetErr("Dirty settlement value".into()))?;
            bracketed_root(
                |spread| Ok(engine.value(&model, &valuation, spread)? - target),
                -0.05,
                0.05,
                -10.0,
            )?
        }
        BondPricingAnchor::ZSpread(spread) | BondPricingAnchor::OAS(spread) => spread,
    };
    let model_settlement_value = engine.value(&model, &valuation, resolved_spread)?;
    let needs_yield = requests.iter().any(|request| {
        matches!(
            request,
            Request::YieldToMaturity | Request::ModifiedDuration
        )
    });
    let rate_definition = needs_yield
        .then(|| trade.yield_rate_definition())
        .transpose()?;
    drop(curve);

    let identifier = trade.bond_identifier();
    let mut state = BondState {
        value: None,
        market_data: Some(market_data),
        legs: vec![trade.bond_leg().clone()],
        evaluation_date,
        settlement_date,
        discount_index,
        spread_day_counter,
        resolved_spread,
        spread_node: None,
        spread_label: if callable {
            format!("{identifier}.OAS")
        } else {
            format!("{identifier}.ZSpread")
        },
    };
    let value = build_ad_value(engine, trade, &mut state, &model)?;

    let yield_to_maturity = needs_yield
        .then(|| {
            solve_yield(
                &cashflows,
                settlement_date,
                rate_definition
                    .ok_or_else(|| QSError::ValueNotSetErr("Fixed bond rate definition".into()))?,
                model_settlement_value,
            )
        })
        .transpose()?;

    let mut results = EvaluationResults::new(evaluation_date, identifier);
    for request in requests {
        match request {
            Request::Value => results = results.with_price(value),
            Request::YieldToMaturity => {
                results = results.with_ytm(
                    yield_to_maturity
                        .ok_or_else(|| QSError::ValueNotSetErr("Yield to maturity".into()))?,
                );
            }
            Request::ModifiedDuration => {
                let yield_rate = yield_to_maturity
                    .ok_or_else(|| QSError::ValueNotSetErr("Yield to maturity".into()))?;
                let (yield_price, derivative) = yield_price_and_derivative(
                    &cashflows,
                    settlement_date,
                    rate_definition.ok_or_else(|| {
                        QSError::ValueNotSetErr("Fixed bond rate definition".into())
                    })?,
                    yield_rate,
                )?;
                if yield_price.abs() < f64::EPSILON {
                    return Err(QSError::InvalidValueErr(
                        "Cannot compute duration for a zero bond price".into(),
                    ));
                }
                results = results.with_modified_duration(-derivative / yield_price);
            }
            Request::ZSpread => results = results.with_z_spread(resolved_spread),
            Request::OAS => results = results.with_oas(resolved_spread),
            Request::Sensitivities => {
                results = results.with_sensitivities(sensitivities(&state)?);
            }
            Request::Cashflows => {
                results = results.with_cashflows(engine.handle_cashflows(trade, &mut state)?);
            }
            Request::AccruedAmount(start, end) => {
                results = results.with_accrued_amount(
                    engine.handle_accrued_amount(trade, &mut state, *start, *end)?,
                );
            }
            Request::FairRate(_) => {}
        }
    }
    Ok(results)
}

/// Requests contractual curves and the engine configuration. Evaluation adds
/// historical fixings and any dependencies required by model preparation.
pub(super) fn bond_market_data_request(
    policy: Option<&dyn DiscountPolicy>,
    trade: &impl BondTerms<DualFwd>,
    instrument_type: InstrumentType,
) -> Option<MarketDataRequest> {
    let index = discount_index(policy, trade).ok()?;
    let mut indices = HashSet::from([index.clone()]);
    let mut elements = vec![ConstructedElementRequest::DiscountCurve {
        market_index: index,
    }];
    for cashflow in trade.bond_leg().cashflows() {
        let coupon_index = match cashflow {
            CashflowType::FloatingRateCoupon(coupon) => Some(coupon.market_index()),
            CashflowType::OptionEmbeddedCoupon(coupon) => Some(coupon.market_index()),
            _ => None,
        };
        if let Some(coupon_index) = coupon_index {
            if indices.insert(coupon_index.clone()) {
                elements.push(ConstructedElementRequest::DiscountCurve {
                    market_index: coupon_index.clone(),
                });
            }
        }
    }
    let mut request = MarketDataRequest::default().with_constructed_elements_request(elements);
    if !trade.call_schedule().is_empty() {
        request = request.with_pricer_configuration_request(vec![instrument_type]);
    }
    request = request.with_fx_request(vec![FxRequest::single(trade.bond_leg().currency())]);
    Some(request)
}

/// Deterministic bond valuation using the trade's price or Z-spread anchor.
#[derive(Default)]
pub struct BondPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}
impl BondPricer {
    /// Creates a reusable pricer; quote and settlement terms belong to each trade.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
impl BondPricingEngine for BondPricer {
    const INSTRUMENT_TYPE: InstrumentType = InstrumentType::FixedRateBond;
    type Model<T: Scalar> = ();
    fn policy(&self) -> Option<&dyn DiscountPolicy> {
        self.discount_policy.as_deref()
    }
    fn prepare(
        &self,
        trade: &impl BondTerms<DualFwd>,
        index: &MarketIndex,
        context: &impl MarketDataProvider,
    ) -> Result<(MarketData, ())> {
        let request = contractual_market_request(
            trade,
            Some(index),
            context.evaluation_date(),
            context.anchor_fixings(),
            None,
        )?;
        let market = context.handle_request(&request)?;
        Ok((market, ()))
    }
    fn ad_model(&self, (): &(), _: &MarketData) -> Result<()> {
        Ok(())
    }
    fn value<T: Scalar>(&self, (): &(), valuation: &BondValuation<'_, T>, spread: T) -> Result<T> {
        discounted_value(valuation, spread)
    }
}
impl<T: BondTerms<DualFwd>> HandleCashflows<T, BondState> for BondPricer {}
impl Pricer for BondPricer {
    type Item = FixedRateBondTrade<DualFwd>;
    type Policy = dyn DiscountPolicy;

    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[Request],
        context: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        evaluate_bond(self, trade, requests, context)
    }
    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest> {
        bond_market_data_request(self.policy(), trade, Self::INSTRUMENT_TYPE)
    }
    fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
        self.discount_policy = Some(policy);
    }
    fn discount_policy(&self) -> Option<&Self::Policy> {
        self.discount_policy.as_deref()
    }
}

/// Shared fixtures for bond-pricer tests.
#[cfg(test)]
pub mod test_support {
    use super::BondPricer;
    use crate::prelude::*;
    use crate::{
        core::{
            collateral::FixedIncomeDiscountPolicy,
            elements::curveelement::DiscountCurveElement,
            marketdatahandling::constructedelementstore::ConstructedElementStore,
            pricerconfiguration::{PricerConfiguration, PricingModelConfiguration},
        },
        currencies::currency::Currency,
        instruments::{
            cashflows::makeleg::PaymentStructure, fixedincome::makefixedratebond::MakeFixedRateBond,
        },
        models::modelconfiguration::{GaussianRateModelParameters, ParameterSource},
        rates::yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        time::enums::Frequency,
    };
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    /// Face value and quote units used by test bonds.
    pub const FACE: f64 = 100.0;

    /// Flat continuously compounded discount rate.
    pub const RATE: f64 = 0.03;

    /// Hull-White mean reversion used by callable-bond tests.
    pub const ALPHA: f64 = 0.05;

    /// Hull-White volatility used by callable-bond tests.
    pub const SIGMA: f64 = 0.01;

    /// Creates a discount policy using the USD SOFR curve.
    pub fn policy() -> Box<dyn DiscountPolicy> {
        Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_discount_curve_index(Currency::USD, MarketIndex::SOFR),
        )
    }

    /// Creates a straight-bond pricer with the test discount policy.
    pub fn fixed_pricer() -> BondPricer {
        let mut pricer = BondPricer::new();
        pricer.set_discount_policy(policy());
        pricer
    }

    /// Supplies fixed market data to bond-pricer tests.
    #[derive(Clone)]
    pub struct TestProvider {
        /// Valuation date reported to pricers.
        pub evaluation_date: Date,
        /// Market data returned for each request.
        pub market_data: MarketData,
    }

    impl MarketDataProvider for TestProvider {
        fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
            Ok(self.market_data.clone())
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    /// Creates a market provider with tree controls and a benchmark flat curve.
    pub fn provider(
        evaluation_date: Date,
        steps_per_year: usize,
    ) -> (TestProvider, FlatForwardTermStructure<f64>) {
        provider_with_method(evaluation_date, steps_per_year, false)
    }

    /// Creates a market provider with tree or Fourier controls and a benchmark curve.
    pub fn provider_with_method(
        evaluation_date: Date,
        steps_per_year: usize,
        use_fourier: bool,
    ) -> (TestProvider, FlatForwardTermStructure<f64>) {
        let definition = RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        );
        let ad_curve =
            FlatForwardTermStructure::new(evaluation_date, DualFwd::from(RATE), definition);
        let benchmark_curve = FlatForwardTermStructure::new(evaluation_date, RATE, definition);
        let mut elements = ConstructedElementStore::default();
        elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(ad_curve))),
        );
        let time_grid = TimeGridConfiguration::new(steps_per_year, 4096, DayCounter::Actual365);
        let pricer = if use_fourier {
            PricerImplementation::BackwardFourierEvolution {
                time_grid,
                fourier_grid: FourierGridConfiguration::new(256, 10.0),
            }
        } else {
            PricerImplementation::TreeBased { time_grid }
        };
        let configuration = PricerConfiguration::new(
            InstrumentType::SteppedCouponBond,
            pricer,
            vec![PricingModelConfiguration::new(
                MarketIndex::SOFR,
                ModelConfiguration::HullWhite {
                    alpha: ALPHA,
                    parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(
                        SIGMA,
                    )),
                },
            )],
        );
        let market_data =
            MarketData::new(HashMap::new(), elements).with_pricer_configurations(HashMap::from([
                (InstrumentType::SteppedCouponBond, configuration),
            ]));
        (
            TestProvider {
                evaluation_date,
                market_data,
            },
            benchmark_curve,
        )
    }

    /// Creates a semiannual USD bullet bond with a zero-spread anchor.
    pub fn fixed_bond(
        evaluation_date: Date,
        maturity_date: Date,
        coupon: f64,
    ) -> FixedRateBondTrade<DualFwd> {
        let definition = RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Compounded,
            Frequency::Semiannual,
        );
        let instrument = MakeFixedRateBond::<DualFwd>::default()
            .with_identifier("TEST-BOND".into())
            .with_start_date(evaluation_date)
            .with_maturity_date(maturity_date)
            .with_rate(coupon)
            .with_units(FACE)
            .with_notional(FACE)
            .with_rate_definition(definition)
            .with_discount_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Semiannual)
            .with_payment_structure(PaymentStructure::Bullet)
            .build()
            .unwrap_or_else(|error| panic!("test fixed bond: {error}"));
        FixedRateBondTrade::new(
            instrument,
            evaluation_date,
            FACE,
            crate::core::trade::Side::LongReceive,
            BondPricingAnchor::ZSpread(0.0),
        )
    }

    /// Returns a straight bond's value, panicking if evaluation fails.
    pub fn fixed_value(
        pricer: &BondPricer,
        trade: &FixedRateBondTrade<DualFwd>,
        provider: &TestProvider,
    ) -> f64 {
        pricer
            .evaluate(trade, &[Request::Value], provider)
            .unwrap_or_else(|error| panic!("fixed bond value: {error}"))
            .price()
            .unwrap_or_else(|| panic!("fixed bond value must be present"))
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::*, BondPricer};
    use crate::prelude::*;

    #[test]
    fn unsupported_bond_requests_are_ignored_before_validating_scalar_targets() -> Result<()> {
        let date = Date::new(2025, 1, 1);
        let trade = fixed_bond(date, Date::new(2027, 1, 1), 0.06);
        let provider = TestProvider {
            evaluation_date: date,
            market_data: MarketData::default(),
        };
        let result = BondPricer::new().evaluate(
            &trade,
            &[Request::OAS, Request::FairRate(0), Request::FairRate(1)],
            &provider,
        )?;
        assert!(result.price().is_none() && result.oas().is_none() && result.fair_rate().is_none());
        Ok(())
    }

    #[test]
    fn bond_accrual_reports_paid_coupon_interest_without_pv_or_tape() -> Result<()> {
        use crate::ad::forward::TAPE_FWD;
        let origin = Date::new(2025, 1, 1);
        let template = fixed_bond(origin, Date::new(2026, 1, 1), 0.06);
        let provider = TestProvider {
            evaluation_date: Date::new(2026, 3, 1),
            market_data: MarketData::default(),
        };
        let start = Date::new(2025, 1, 15);
        let end = Date::new(2025, 2, 15);
        Tape::stop_recording_fwd();
        let before = TAPE_FWD.with(|tape| (tape.borrow().book.len(), tape.borrow().mark));
        for side in [Side::LongReceive, Side::PayShort] {
            let trade = FixedRateBondTrade::new(
                template.instrument().clone(),
                origin,
                FACE,
                side,
                BondPricingAnchor::CleanPrice(97.0),
            )
            .with_settlement_date(origin);
            let result = BondPricer::new().evaluate(
                &trade,
                &[Request::AccruedAmount(start, end)],
                &provider,
            )?;
            assert!(
                (result.accrued_amount().unwrap_or_default()
                    - side.sign() * FACE * 0.06 * 31.0 / 365.0)
                    .abs()
                    < 1e-12
            );
            assert!(result.price().is_none() && result.cashflows().is_none());
            assert_eq!(
                BondPricer::new()
                    .evaluate(&trade, &[Request::AccruedAmount(start, start)], &provider)?
                    .accrued_amount(),
                Some(0.0)
            );
            assert!(BondPricer::new()
                .evaluate(&trade, &[Request::AccruedAmount(end, start)], &provider)
                .is_err());
        }
        let after = TAPE_FWD.with(|tape| (tape.borrow().book.len(), tape.borrow().mark));
        assert_eq!(before, after);
        Ok(())
    }

    #[test]
    fn callable_contractual_accrual_uses_no_exercise_model() -> Result<()> {
        use crate::pricers::fixedincome::{
            backwardfourierevolutionbondpricer::BackwardFourierEvolutionBondPricer,
            treebasedbondpricer::{test_support::bond, TreeBasedBondPricer},
        };
        let origin = Date::new(2025, 1, 1);
        let trade = bond(
            origin,
            Date::new(2027, 1, 1),
            0.06,
            crate::instruments::cashflows::makeleg::PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(Date::new(2026, 1, 1), FACE)],
        );
        let provider = TestProvider {
            evaluation_date: origin,
            market_data: MarketData::default(),
        };
        let request = Request::AccruedAmount(Date::new(2026, 2, 1), Date::new(2026, 3, 1));
        let tree = TreeBasedBondPricer::new().evaluate(
            &trade,
            &[
                request,
                Request::YieldToMaturity,
                Request::ModifiedDuration,
                Request::ZSpread,
                Request::FairRate(0),
                Request::FairRate(1),
            ],
            &provider,
        )?;
        let fourier = BackwardFourierEvolutionBondPricer::new().evaluate(
            &trade,
            &[
                request,
                Request::YieldToMaturity,
                Request::ModifiedDuration,
                Request::ZSpread,
                Request::FairRate(0),
                Request::FairRate(1),
            ],
            &provider,
        )?;
        assert!(
            (tree.accrued_amount().unwrap_or_default() - FACE * 0.06 * 28.0 / 365.0).abs() < 1e-12
        );
        assert_eq!(tree.accrued_amount(), fourier.accrued_amount());
        assert!(tree.price().is_none() && fourier.price().is_none());
        Ok(())
    }

    #[test]
    fn bond_accrual_resolves_only_in_window_fixings_including_capped_coupons() -> Result<()> {
        use crate::pricers::fixedincome::treebasedbondpricer::TreeBasedBondPricer;
        use std::collections::{BTreeMap, HashMap};
        let origin = Date::new(2025, 1, 1);
        let reset = Date::new(2025, 7, 1);
        let provider = TestProvider {
            evaluation_date: Date::new(2025, 8, 1),
            market_data: MarketData::new(
                HashMap::from([(MarketIndex::EURIBOR3m, BTreeMap::from([(reset, 0.04)]))]),
                ConstructedElementStore::default(),
            ),
        };
        for (cap, rate) in [(None, 0.05), (Some(0.02), 0.02)] {
            let bond = MakeSteppedCouponBond::<DualFwd>::default()
                .with_identifier("COUPON-ACCRUAL".into())
                .with_start_date(origin)
                .with_maturity_date(Date::new(2026, 1, 1))
                .with_notional(1000.0)
                .with_units(100.0)
                .with_currency(Currency::EUR)
                .with_notional_percentages(vec![1.0])
                .with_payment_frequency(Frequency::Semiannual)
                .with_coupons(vec![CashflowType::floating(
                    MarketIndex::EURIBOR3m,
                    0.01,
                    None,
                    cap,
                )])
                .build()?;
            let trade = SteppedCouponBondTrade::new(
                bond,
                origin,
                1000.0,
                Side::LongReceive,
                BondPricingAnchor::ZSpread(0.0),
            );
            let result = TreeBasedBondPricer::new().evaluate(
                &trade,
                &[Request::AccruedAmount(
                    provider.evaluation_date,
                    Date::new(2025, 9, 1),
                )],
                &provider,
            )?;
            assert!(
                (result.accrued_amount().unwrap_or_default() - 1000.0 * rate * 31.0 / 360.0).abs()
                    < 1e-10
            );
        }
        Ok(())
    }

    #[test]
    fn native_base_currency_changes_value_and_risk_but_preserves_bond_analytics() -> Result<()> {
        let date = Date::new(2025, 1, 1);
        let trade = fixed_bond(date, Date::new(2030, 1, 1), 0.04);
        let (provider, _) = provider(date, 12);
        let mut fx = FxStore::new();
        fx.add_fx_rate(Currency::USD, Currency::EUR, DualFwd::new(0.9));
        let context = PricingContext::new()
            .with_quote_store(QuoteStore::new(date))
            .with_constructed_elements(provider.market_data.constructed_elements().clone())
            .with_fx_store(fx);
        let requests = [
            Request::Value,
            Request::Sensitivities,
            Request::YieldToMaturity,
            Request::ModifiedDuration,
            Request::ZSpread,
            Request::Cashflows,
            Request::AccruedAmount(date, Date::new(2025, 2, 1)),
        ];
        let local = fixed_pricer().evaluate(&trade, &requests, &context)?;
        let context = context.with_base_currency(Currency::EUR);
        let mut pricer = BondPricer::new();
        pricer.set_discount_policy(Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_discount_curve_index(Currency::USD, MarketIndex::SOFR),
        ));
        let converted = pricer.evaluate(&trade, &requests, &context)?;
        let local_pv = local
            .price()
            .ok_or_else(|| QSError::ValueNotSetErr("PV".into()))?;
        assert!((converted.price().unwrap_or_default() - 0.9 * local_pv).abs() < 1e-10);
        assert!(
            (converted.accrued_amount().unwrap_or_default()
                - 0.9 * local.accrued_amount().unwrap_or_default())
            .abs()
                < 1e-12
        );
        let standalone = pricer.evaluate(
            &trade,
            &[Request::AccruedAmount(date, Date::new(2025, 2, 1))],
            &context,
        )?;
        assert_eq!(standalone.accrued_amount(), converted.accrued_amount());
        assert_eq!(converted.ytm(), local.ytm());
        assert_eq!(converted.modified_duration(), local.modified_duration());
        assert_eq!(converted.z_spread(), local.z_spread());
        let risk = converted
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Risk".into()))?;
        let exposure = |label: &str| {
            risk.instrument_keys()
                .iter()
                .zip(risk.exposure())
                .find_map(|(key, value)| (key == label).then_some(*value))
        };
        assert!((exposure("USD/EUR").unwrap_or_default() - local_pv).abs() < 1e-10);
        for (label, value) in local
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Risk".into()))?
            .instrument_keys()
            .iter()
            .zip(
                local
                    .sensitivities()
                    .ok_or_else(|| QSError::ValueNotSetErr("Risk".into()))?
                    .exposure(),
            )
        {
            assert!((exposure(label).unwrap_or_default() - 0.9 * value).abs() < 1e-10);
        }
        assert!(converted
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Cashflows".into()))?
            .currencies()
            .iter()
            .all(|currency| *currency == Currency::USD));
        Ok(())
    }

    #[test]
    fn z_spread_price_anchor_round_trip_duration_and_risk() {
        let evaluation_date = Date::new(2025, 1, 1);
        let (provider, _) = provider(evaluation_date, 48);
        let trade = fixed_bond(evaluation_date, Date::new(2030, 1, 1), 0.04);
        let spread = 0.0125;
        let direct = fixed_pricer();
        let direct_results = direct
            .evaluate(
                &trade
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::ZSpread(spread)),
                &[
                    Request::Value,
                    Request::ZSpread,
                    Request::YieldToMaturity,
                    Request::ModifiedDuration,
                    Request::Sensitivities,
                ],
                &provider,
            )
            .unwrap_or_else(|error| panic!("direct bond price: {error}"));
        let direct_value = direct_results
            .price()
            .unwrap_or_else(|| panic!("price result"));
        assert_eq!(direct_results.z_spread(), Some(spread));
        assert!(direct_results.ytm().is_some_and(f64::is_finite));
        assert!(direct_results
            .modified_duration()
            .is_some_and(|duration| duration > 0.0));

        let quoted_price = direct_value / FACE * FACE;
        let implied = fixed_pricer();
        let implied_results = implied
            .evaluate(
                &trade
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::DirtyPrice(quoted_price)),
                &[Request::Value, Request::ZSpread],
                &provider,
            )
            .unwrap_or_else(|error| panic!("price-anchored bond: {error}"));
        assert!((implied_results.z_spread().unwrap_or_default() - spread).abs() < 1.0e-10);
        assert!((implied_results.price().unwrap_or_default() - direct_value).abs() < 1.0e-9);

        let risk = direct_results
            .sensitivities()
            .unwrap_or_else(|| panic!("sensitivity result"));
        let spread_risk = risk
            .instrument_keys()
            .iter()
            .zip(risk.exposure())
            .find_map(|(label, exposure)| (label == "TEST-BOND.ZSpread").then_some(*exposure))
            .unwrap_or_else(|| panic!("Z-spread sensitivity"));
        let bump = 1.0e-6;
        let finite_difference = (fixed_value(
            &fixed_pricer(),
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::ZSpread(spread + bump)),
            &provider,
        ) - fixed_value(
            &fixed_pricer(),
            &trade.with_pricing_anchor(BondPricingAnchor::ZSpread(spread - bump)),
            &provider,
        )) / (2.0 * bump);
        assert!((spread_risk - finite_difference).abs() < 1.0e-4);
    }

    #[test]
    fn trade_owns_anchor_settlement_and_side_across_scalar_conversion() -> Result<()> {
        let evaluation = Date::new(2025, 1, 1);
        let settlement = Date::new(2025, 7, 1);
        let maturity = Date::new(2027, 1, 1);
        let (provider, _) = provider(evaluation, 12);
        let base = fixed_bond(evaluation, maturity, 0.0);
        let long = FixedRateBondTrade::new(
            base.instrument().clone(),
            evaluation,
            FACE,
            Side::LongReceive,
            BondPricingAnchor::DirtyPrice(97.0),
        )
        .with_settlement_date(settlement)
        .with_spread_day_counter(DayCounter::Actual360);
        let scalar: FixedRateBondTrade<f64> = long.clone().into();
        let roundtrip: FixedRateBondTrade<DualFwd> = scalar.into();
        assert_eq!(roundtrip.pricing_anchor(), long.pricing_anchor());
        assert_eq!(roundtrip.settlement_date(), Some(settlement));
        assert_eq!(roundtrip.spread_day_counter(), Some(DayCounter::Actual360));
        let short = FixedRateBondTrade::new(
            base.instrument().clone(),
            evaluation,
            FACE,
            Side::PayShort,
            long.pricing_anchor(),
        )
        .with_settlement_date(settlement)
        .with_spread_day_counter(DayCounter::Actual360);
        let pricer = fixed_pricer();
        let requests = [
            Request::Value,
            Request::ZSpread,
            Request::Sensitivities,
            Request::Cashflows,
        ];
        let long_result = pricer.evaluate(&roundtrip, &requests, &provider)?;
        let short_result = pricer.evaluate(&short, &requests, &provider)?;
        assert!(
            (long_result.price().unwrap_or_default() + short_result.price().unwrap_or_default())
                .abs()
                < 1.0e-10
        );
        assert_eq!(long_result.z_spread(), short_result.z_spread());
        let long_cashflows = long_result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Long cashflows".into()))?;
        let short_cashflows = short_result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Short cashflows".into()))?;
        assert_eq!(
            long_cashflows.payment_dates(),
            short_cashflows.payment_dates()
        );
        assert_eq!(
            long_cashflows.amounts().len(),
            short_cashflows.amounts().len()
        );
        for (long, short) in long_cashflows
            .amounts()
            .iter()
            .zip(short_cashflows.amounts())
        {
            assert!((long + short).abs() < 1e-10);
        }
        let spread = long_result
            .z_spread()
            .ok_or_else(|| QSError::ValueNotSetErr("Z-spread".into()))?;
        let spread_result = pricer.evaluate(
            &long.with_pricing_anchor(BondPricingAnchor::ZSpread(spread)),
            &requests,
            &provider,
        )?;
        assert!(
            (spread_result.price().unwrap_or_default() - long_result.price().unwrap_or_default())
                .abs()
                < 1.0e-9
        );
        let long_risk = long_result
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Long risk".into()))?;
        let short_risk = short_result
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Short risk".into()))?;
        for (left, right) in long_risk.exposure().iter().zip(short_risk.exposure()) {
            assert!((left + right).abs() < 1.0e-9);
        }
        let short_scalar: FixedRateBondTrade<f64> = short.into();
        assert!(short_scalar
            .into_contingent_claims("SHORT")?
            .iter()
            .all(|claim| claim.side() == Side::PayShort));
        assert_eq!(base.instrument().leg().side(), Side::LongReceive);
        Ok(())
    }

    #[test]
    fn csa_policy_resolves_collateral_curve_for_foreign_bond() -> Result<()> {
        let date = Date::new(2025, 1, 1);
        let trade = fixed_bond(date, Date::new(2027, 1, 1), 0.0);
        let mut pricer = BondPricer::new();
        pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
            MarketIndex::Other("EUR-COLLATERAL".into()),
            Currency::EUR,
        )));
        let request = pricer
            .market_data_request(&trade)
            .ok_or_else(|| QSError::NotFoundErr("Foreign CSA request".into()))?;
        assert!(matches!(
            request
                .constructed_elements_request()
                .and_then(|elements| elements.first()),
            Some(ConstructedElementRequest::DiscountCurve {
                market_index: MarketIndex::Collateral(Currency::USD, Currency::EUR)
            })
        ));
        Ok(())
    }
}

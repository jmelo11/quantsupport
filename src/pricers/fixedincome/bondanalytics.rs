//! Shared bond quote, settlement, analytics and risk semantics.
use crate::rates::yieldtermstructure::primalcurve::PrimalCurve;
use crate::{
    ad::{dual::DualFwd, scalar::Scalar, tape::Tape},
    core::{
        collateral::DiscountPolicy,
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            fixingrequest::FixingRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricerconfiguration::InstrumentType,
        pricerstate::PricerState,
        request::{HandleCashflows, LegsProvider, Request},
        trade::Trade,
    },
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::{
            cashflow::Cashflow,
            cashflowtype::CashflowType,
            coupons::{LinearCoupon, NonLinearCoupon},
            leg::Leg,
        },
        fixedincome::{
            bondpricinganchor::BondPricingAnchor, callschedule::CallScheduleEntry,
            fixedratebond::FixedRateBondTrade, steppedcouponbond::SteppedCouponBondTrade,
        },
    },
    rates::{
        compounding::Compounding, interestrate::RateDefinition,
        yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};
use std::collections::HashSet;

/// Common contract exposed by straight and stepped-coupon bond trades.
pub(super) trait BondTerms: LegsProvider<DualFwd> {
    fn pricing_anchor(&self) -> BondPricingAnchor;
    fn settlement_date(&self) -> Option<Date>;
    fn spread_day_counter(&self) -> Option<DayCounter>;
    /// Returns the single bond cashflow leg.
    fn bond_leg(&self) -> &Leg<DualFwd>;
    /// Returns the quote units, normally 100.
    fn units(&self) -> f64;
    /// Returns the issuer call schedule.
    fn call_schedule(&self) -> &[CallScheduleEntry];
    /// Returns the convention for yield and duration.
    fn yield_rate_definition(&self) -> Result<RateDefinition>;
    /// Returns the context configuration key.
    fn instrument_type(&self) -> InstrumentType;
    /// Returns the instrument identifier.
    fn identifier(&self) -> String;
    /// Returns the signed trade side.
    fn side(&self) -> crate::core::trade::Side;
}

impl BondTerms for FixedRateBondTrade<DualFwd> {
    fn pricing_anchor(&self) -> BondPricingAnchor {
        self.pricing_anchor()
    }
    fn settlement_date(&self) -> Option<Date> {
        self.settlement_date()
    }
    fn spread_day_counter(&self) -> Option<DayCounter> {
        self.spread_day_counter()
    }
    fn bond_leg(&self) -> &Leg<DualFwd> {
        self.instrument().leg()
    }

    fn units(&self) -> f64 {
        self.instrument().units()
    }

    fn call_schedule(&self) -> &[CallScheduleEntry] {
        &[]
    }

    fn yield_rate_definition(&self) -> Result<RateDefinition> {
        self.instrument()
            .leg()
            .interest_rate()
            .map(|rate| rate.rate_definition())
            .ok_or_else(|| QSError::ValueNotSetErr("Fixed bond rate definition".into()))
    }

    fn instrument_type(&self) -> InstrumentType {
        InstrumentType::FixedRateBond
    }

    fn identifier(&self) -> String {
        self.instrument().identifier()
    }

    fn side(&self) -> crate::core::trade::Side {
        Trade::side(self)
    }
}

impl BondTerms for SteppedCouponBondTrade<DualFwd> {
    fn pricing_anchor(&self) -> BondPricingAnchor {
        self.pricing_anchor()
    }
    fn settlement_date(&self) -> Option<Date> {
        self.settlement_date()
    }
    fn spread_day_counter(&self) -> Option<DayCounter> {
        self.spread_day_counter()
    }
    fn bond_leg(&self) -> &Leg<DualFwd> {
        self.instrument().leg()
    }

    fn units(&self) -> f64 {
        self.instrument().units()
    }

    fn call_schedule(&self) -> &[CallScheduleEntry] {
        self.instrument().call_schedule()
    }

    fn yield_rate_definition(&self) -> Result<RateDefinition> {
        Err(QSError::InvalidValueErr(
            "Yield-to-maturity and modified duration are not defined for SteppedCouponBond; use price, OAS, and sensitivities"
                .into(),
        ))
    }

    fn instrument_type(&self) -> InstrumentType {
        InstrumentType::SteppedCouponBond
    }

    fn identifier(&self) -> String {
        self.instrument().identifier()
    }

    fn side(&self) -> crate::core::trade::Side {
        Trade::side(self)
    }
}

fn discount_index(
    policy: Option<&dyn DiscountPolicy>,
    trade: &impl BondTerms,
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
fn resolve_leg(
    leg: &Leg<DualFwd>,
    market_data: &MarketData,
    evaluation_date: Date,
) -> Result<Leg<DualFwd>> {
    let mut resolved = leg.clone();
    for cashflow in resolved.cashflows_mut() {
        match cashflow {
            CashflowType::FloatingRateCoupon(coupon) => {
                let index = coupon.market_index().clone();
                let fixing = resolve_coupon_fixing(
                    market_data,
                    &index,
                    coupon.accrual_start_date(),
                    coupon.accrual_end_date(),
                    evaluation_date,
                )?;
                coupon.set_fixing(fixing);
            }
            CashflowType::OptionEmbeddedCoupon(coupon) => {
                let index = coupon.market_index().clone();
                let fixing = resolve_coupon_fixing(
                    market_data,
                    &index,
                    coupon.accrual_start_date(),
                    coupon.accrual_end_date(),
                    evaluation_date,
                )?;
                coupon.set_fixing(fixing);
            }
            _ => {}
        }
    }
    Ok(resolved)
}

/// Resolves one in-advance coupon fixing from the fixing store or forward curve.
fn resolve_coupon_fixing(
    market_data: &MarketData,
    index: &MarketIndex,
    accrual_start: Date,
    accrual_end: Date,
    evaluation_date: Date,
) -> Result<DualFwd> {
    if accrual_start < evaluation_date {
        return market_data
            .fixings()
            .get(index)
            .and_then(|fixings| fixings.get(&accrual_start))
            .copied()
            .map(DualFwd::new)
            .ok_or_else(|| {
                QSError::NotFoundErr(format!(
                    "Fixing for index {index} on date {accrual_start} not found"
                ))
            });
    }
    let curve = market_data
        .constructed_elements()
        .discount_curve(index)
        .ok_or_else(|| QSError::NotFoundErr(format!("Forward curve not found for {index}")))?
        .curve();
    let definition = index.rate_index_details()?.rate_definition();
    curve.forward_rate(
        accrual_start,
        accrual_end,
        definition.compounding(),
        definition.frequency(),
    )
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

/// Calculates coupon accrual from the active coupon's accrual start through
/// settlement.
fn accrued_interest(leg: &Leg<DualFwd>, settlement_date: Date) -> Result<f64> {
    for cashflow in leg.cashflows() {
        let amount = match cashflow {
            CashflowType::FixedRateCoupon(coupon)
                if coupon.accrual_start_date() < settlement_date
                    && settlement_date < coupon.accrual_end_date() =>
            {
                Some(
                    coupon
                        .accrued_amount(coupon.accrual_start_date(), settlement_date)?
                        .value(),
                )
            }
            CashflowType::FloatingRateCoupon(coupon)
                if coupon.accrual_start_date() < settlement_date
                    && settlement_date < coupon.accrual_end_date() =>
            {
                Some(
                    coupon
                        .accrued_amount(coupon.accrual_start_date(), settlement_date)?
                        .value(),
                )
            }
            CashflowType::OptionEmbeddedCoupon(coupon)
                if coupon.accrual_start_date() < settlement_date
                    && settlement_date < coupon.accrual_end_date() =>
            {
                Some(
                    coupon
                        .accrued_amount(coupon.accrual_start_date(), settlement_date)?
                        .value(),
                )
            }
            _ => None,
        };
        if let Some(amount) = amount {
            return Ok(amount);
        }
    }
    Ok(0.0)
}

/// Converts a clean or dirty quote into an optional dirty price per bond
/// units; spread anchors do not provide a target price.
fn quoted_dirty_price(
    anchor: BondPricingAnchor,
    leg: &Leg<DualFwd>,
    units: f64,
    settlement_date: Date,
    principal: f64,
) -> Result<Option<f64>> {
    match anchor {
        BondPricingAnchor::CleanPrice(clean) => {
            let accrued_per_unit = accrued_interest(leg, settlement_date)? * units / principal;
            Ok(Some(clean + accrued_per_unit))
        }
        BondPricingAnchor::DirtyPrice(dirty) => Ok(Some(dirty)),
        BondPricingAnchor::ZSpread(_) | BondPricingAnchor::OAS(_) => Ok(None),
    }
}

/// Solves a monotone decreasing bond-price objective by adaptive bracketing
/// followed by bisection.
fn bracketed_root(
    mut objective: impl FnMut(f64) -> Result<f64>,
    initial_lower: f64,
    initial_upper: f64,
    hard_lower: f64,
) -> Result<f64> {
    let mut lower = initial_lower.max(hard_lower);
    let mut upper = initial_upper;
    let mut f_lower = objective(lower)?;
    let mut f_upper = objective(upper)?;

    for _ in 0..64 {
        if f_lower >= 0.0 && f_upper <= 0.0 {
            break;
        }
        if f_lower < 0.0 {
            let candidate = 2.0f64.mul_add(lower, -0.01).max(hard_lower);
            if (candidate - lower).abs() < f64::EPSILON {
                break;
            }
            lower = candidate;
            f_lower = objective(lower)?;
        }
        if f_upper > 0.0 {
            upper = 2.0f64.mul_add(upper, 0.01);
            f_upper = objective(upper)?;
        }
    }

    if f_lower.abs() < 1e-12 {
        return Ok(lower);
    }
    if f_upper.abs() < 1e-12 {
        return Ok(upper);
    }
    if !(f_lower >= 0.0 && f_upper <= 0.0) {
        return Err(QSError::SolverErr(format!(
            "Could not bracket bond root: f({lower})={f_lower}, f({upper})={f_upper}"
        )));
    }

    for _ in 0..256 {
        let midpoint = f64::midpoint(lower, upper);
        let f_midpoint = objective(midpoint)?;
        if f_midpoint.abs() < 1e-12 || (upper - lower).abs() < 1e-14 {
            return Ok(midpoint);
        }
        if f_midpoint > 0.0 {
            lower = midpoint;
        } else {
            upper = midpoint;
        }
    }
    Err(QSError::SolverErr(
        "Bond root solve did not converge after 256 iterations".into(),
    ))
}

/// Returns a yield discount factor and its analytic first derivative for one
/// time under the requested compounding convention.
fn yield_discount_and_derivative(
    rate: f64,
    time: f64,
    compounding: Compounding,
    frequency: Frequency,
) -> Result<(f64, f64)> {
    let frequency_value = f64::from(frequency as i32);
    let simple = || {
        let compound = rate.mul_add(time, 1.0);
        if compound <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Yield produces a non-positive compound factor".into(),
            ));
        }
        Ok((1.0 / compound, -time / (compound * compound)))
    };
    let compounded = || {
        let base = rate / frequency_value + 1.0;
        if base <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Yield is outside the compounded-rate domain".into(),
            ));
        }
        let discount = base.powf(-frequency_value * time);
        Ok((discount, -time * discount / base))
    };

    match compounding {
        Compounding::Simple => simple(),
        Compounding::Compounded => compounded(),
        Compounding::Continuous => {
            let discount = (-rate * time).exp();
            Ok((discount, -time * discount))
        }
        Compounding::SimpleThenCompounded => {
            if time <= 1.0 / frequency_value {
                simple()
            } else {
                compounded()
            }
        }
        Compounding::CompoundedThenSimple => {
            if time > 1.0 / frequency_value {
                simple()
            } else {
                compounded()
            }
        }
    }
}

/// Returns dirty yield price and `dP/dy` for all remaining cashflows.
fn yield_price_and_derivative(
    cashflows: &[ResolvedCashflow<f64>],
    settlement_date: Date,
    rate_definition: RateDefinition,
    yield_rate: f64,
) -> Result<(f64, f64)> {
    cashflows
        .iter()
        .try_fold((0.0, 0.0), |(price, derivative), cashflow| {
            let time = rate_definition
                .day_counter()
                .year_fraction(settlement_date, cashflow.payment_date);
            let (discount, discount_derivative) = yield_discount_and_derivative(
                yield_rate,
                time,
                rate_definition.compounding(),
                rate_definition.frequency(),
            )?;
            Ok((
                f64::mul_add(cashflow.amount, discount, price),
                f64::mul_add(cashflow.amount, discount_derivative, derivative),
            ))
        })
}

/// Solves yield to maturity from the model dirty value under the bond's rate
/// definition.
fn solve_yield(
    cashflows: &[ResolvedCashflow<f64>],
    settlement_date: Date,
    rate_definition: RateDefinition,
    target_value: f64,
) -> Result<f64> {
    let frequency_value = f64::from(rate_definition.frequency() as i32);
    let hard_lower = match rate_definition.compounding() {
        Compounding::Compounded
        | Compounding::SimpleThenCompounded
        | Compounding::CompoundedThenSimple => -0.999_999 * frequency_value,
        Compounding::Simple | Compounding::Continuous => -10.0,
    };
    bracketed_root(
        |yield_rate| {
            Ok(
                yield_price_and_derivative(
                    cashflows,
                    settlement_date,
                    rate_definition,
                    yield_rate,
                )?
                .0 - target_value,
            )
        },
        -0.05,
        0.10,
        hard_lower,
    )
}

/// Engine-specific preparation and valuation. Analytics do not dispatch by method.
pub(super) trait BondPricingEngine {
    type Model<T: Scalar>;
    fn policy(&self) -> Option<&dyn DiscountPolicy>;
    fn prepare(
        &self,
        trade: &impl BondTerms,
        discount_index: &MarketIndex,
        context: &impl MarketDataProvider,
    ) -> Result<(MarketData, Self::Model<f64>)>;
    fn ad_model(
        &self,
        model: &Self::Model<f64>,
        market: &MarketData,
    ) -> Result<Self::Model<DualFwd>>;
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
            Ok(value.add_val(
                cashflow
                    .amount
                    .mul_val(discount)
                    .mul_val(spread.mul_val(T::scalar(-time)).exp()),
            ))
        })
}

/// Requests contractual discount/forward curves and past coupon fixings.
pub(super) fn contractual_market_request(
    trade: &impl BondTerms,
    discount_index: &MarketIndex,
    evaluation_date: Date,
) -> MarketDataRequest {
    let mut indices = HashSet::from([discount_index.clone()]);
    let mut elements = vec![ConstructedElementRequest::DiscountCurve {
        market_index: discount_index.clone(),
    }];
    let mut fixings = Vec::new();
    for cashflow in trade.bond_leg().cashflows() {
        let coupon = match cashflow {
            CashflowType::FloatingRateCoupon(coupon) => {
                Some((coupon.market_index(), coupon.accrual_start_date()))
            }
            CashflowType::OptionEmbeddedCoupon(coupon) => {
                Some((coupon.market_index(), coupon.accrual_start_date()))
            }
            _ => None,
        };
        if let Some((index, fixing_date)) = coupon {
            if indices.insert(index.clone()) {
                elements.push(ConstructedElementRequest::DiscountCurve {
                    market_index: index.clone(),
                });
            }
            if fixing_date < evaluation_date {
                fixings.push(FixingRequest::new(index.clone(), fixing_date));
            }
        }
    }
    MarketDataRequest::default()
        .with_constructed_elements_request(elements)
        .with_fixings_request(fixings)
}
/// Records the engine valuation and the trade's spread as an independent risk factor.
fn build_ad_value<E: BondPricingEngine>(
    engine: &E,
    trade: &impl BondTerms,
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
        let resolved_leg = resolve_leg(trade.bond_leg(), market, state.evaluation_date)?;
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
            .mul_val(DualFwd::scalar(trade.side().sign()));
        drop(curve);
        state.legs = vec![resolved_leg];
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
    T: BondTerms,
    E: BondPricingEngine + HandleCashflows<T, BondState>,
{
    let anchor = trade.pricing_anchor();
    anchor.validate()?;
    let callable = !trade.call_schedule().is_empty();
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
    if requests
        .iter()
        .any(|request| matches!(request, Request::OAS))
        && !callable
    {
        return Err(QSError::InvalidValueErr(
            "OAS is not defined for a bond without an issuer call schedule".into(),
        ));
    }
    if requests
        .iter()
        .any(|request| matches!(request, Request::ZSpread))
        && callable
    {
        return Err(QSError::InvalidValueErr(
            "Use OAS rather than ZSpread for an issuer-call bond".into(),
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
    let resolved_leg = resolve_leg(trade.bond_leg(), &market_data, evaluation_date)?;
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

    let identifier = trade.identifier();
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
            Request::FairRate => {}
        }
    }
    Ok(results)
}

/// Requests contractual curves and the engine configuration. Evaluation adds
/// historical fixings and any dependencies required by model preparation.
pub(super) fn bond_market_data_request(
    policy: Option<&dyn DiscountPolicy>,
    trade: &impl BondTerms,
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
        request = request.with_pricer_configuration_request(vec![trade.instrument_type()]);
    }
    Some(request)
}

macro_rules! impl_bond_pricer {
    ($pricer:ty, $item:ty) => {
        impl Pricer for $pricer {
            type Item = $item;
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
                bond_market_data_request(self.policy(), trade)
            }

            fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
                self.discount_policy = Some(policy);
            }

            fn discount_policy(&self) -> Option<&Self::Policy> {
                self.discount_policy.as_deref()
            }
        }
    };
}

pub(super) use impl_bond_pricer;

//! Fixed-rate bond pricing and risk.
//!
//! [`BondPricer`](crate::pricers::fixedincome::bondpricer::BondPricer),
//! [`TreeBasedBondPricer`](crate::pricers::fixedincome::bondpricer::TreeBasedBondPricer), and
//! [`BackwardFourierEvolutionBondPricer`](crate::pricers::fixedincome::bondpricer::BackwardFourierEvolutionBondPricer)
//! share
//! the same quote, settlement, analytics, and risk semantics. Each is
//! configured with exactly one
//! [`BondPricingAnchor`](crate::pricers::fixedincome::bondpricer::BondPricingAnchor). A
//! clean- or dirty-price anchor implies the continuously compounded Z-spread
//! for a straight fixed-rate bond or OAS for a callable stepped-coupon bond. Explicit spread
//! anchors produce the model price directly. The converged or supplied spread
//! is then placed on the AD tape as an independent market factor, so the
//! ordinary sensitivity result contains curve-quote risk and either
//! `dPV/dZSpread` or `dPV/dOAS`.
//!
//! Price anchors are quoted per the bond's `units` (normally 100). They are
//! converted to a dirty settlement value using accrued interest and remaining
//! principal. [`Request::Value`](crate::core::request::Request::Value) remains
//! the signed position PV at the context
//! evaluation date. When settlement differs from the evaluation date, the
//! anchor equation is solved at settlement and the reported PV is valued at
//! the evaluation date.
//!
//! # Issuer-call engines
//!
//! Both callable-bond engines use the one-factor Hull-White state
//! `dx = -alpha * x * dt + sigma * dW` and fit a deterministic rate shift to
//! the initial discount curve.
//! [`TreeBasedBondPricer`](crate::pricers::fixedincome::bondpricer::TreeBasedBondPricer)
//! approximates the
//! conditional Gaussian transition with a recombining trinomial tree.
//! [`BackwardFourierEvolutionBondPricer`](crate::pricers::fixedincome::bondpricer::BackwardFourierEvolutionBondPricer)
//! evolves the value function backward
//! on a uniform state grid: an FFT, multiplication by
//! `exp(-0.5 * variance * k^2)`, and an inverse FFT evaluate the Gaussian
//! conditional expectation. At an issuer call date the obstacle applies
//! `min(V, K)`. OAS enters both engines as an additive
//! continuously compounded short-rate spread.
//!
//! A calibrated parameter source uses the existing Hull-White calibrator,
//! also used by `LgmRateModel`, against the requested constructed surface or
//! cube. Both engines retain its entire piecewise sigma schedule and split
//! innovation-variance integration at every boundary. The first sigma applies
//! from time zero; the last is extrapolated beyond the final pillar. Calibrated
//! propagation uses the calibration curve's reference date and day counter,
//! preserving the configured step density and step limit.
//!
//! Calibration volatility derivatives are chained through the sampled AD
//! surface/cube values to their original risk pillars. Curve and OAS risk are
//! also reported, but sigma's response to curve quotes and calibration-strike
//! changes is not included: the existing calibrator supplies only the
//! volatility Jacobian. Cube baskets must respect the source interpolator's
//! domain and select at most one underlying tenor at each expiry.

use super::{
    backwardfourierevolutionbondpricer::backward_fourier_value, treebasedbondpricer::lattice_value,
};
use std::collections::HashSet;

pub use super::{
    backwardfourierevolutionbondpricer::BackwardFourierEvolutionBondPricer,
    treebasedbondpricer::TreeBasedBondPricer,
};

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
        pricer::Pricer,
        pricerconfiguration::{
            FourierGridConfiguration, InstrumentType, PricerImplementation, TimeGridConfiguration,
        },
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
            callschedule::CallScheduleEntry, fixedratebond::FixedRateBondTrade,
            steppedcouponbond::SteppedCouponBondTrade,
        },
    },
    models::{
        hullwhite::{
            hullwhitecalibrationquality::HullWhiteCalibrationQuality, hullwhitemodel::HullWhite,
        },
        modelconfiguration::{ModelConfiguration, ParameterSource},
    },
    quotes::quote::{Level, QuoteDetails},
    rates::{
        compounding::Compounding, interestrate::RateDefinition,
        yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
    volatility::modelcalibration::CalibrationSource,
};

/// Market observable used to anchor a bond valuation.
///
/// Clean and dirty prices are quoted per the bond's configured units. A price
/// anchor implies a Z-spread for a straight fixed-rate bond. `OAS` is reserved
/// for issuer-call bonds and is rejected by the deterministic engine.
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BondPricingAnchor {
    /// Clean price, excluding accrued interest.
    CleanPrice(f64),
    /// Dirty price, including accrued interest.
    DirtyPrice(f64),
    /// Continuously compounded spread over the discount curve.
    ZSpread(f64),
    /// Option-adjusted spread for an issuer-call pricing model.
    OAS(f64),
}

impl BondPricingAnchor {
    /// Returns the numeric quote carried by the anchor variant.
    const fn value(self) -> f64 {
        match self {
            Self::CleanPrice(value)
            | Self::DirtyPrice(value)
            | Self::ZSpread(value)
            | Self::OAS(value) => value,
        }
    }

    /// Checks finite spreads and strictly positive quoted prices.
    fn validate(self) -> Result<()> {
        let value = self.value();
        if !value.is_finite() {
            return Err(QSError::InvalidValueErr(
                "Bond pricing anchor must be finite".into(),
            ));
        }
        if matches!(self, Self::CleanPrice(_) | Self::DirtyPrice(_)) && value <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Clean and dirty bond prices must be positive".into(),
            ));
        }
        Ok(())
    }
}

/// Common contract exposed by straight and stepped-coupon bond trades.
pub(super) trait BondTerms: LegsProvider<DualFwd> {
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

/// Quote, settlement, spread-time, and discount-policy settings shared by the
/// straight and issuer-call bond engines.
pub(super) struct BondPricerSettings {
    anchor: BondPricingAnchor,
    settlement_date: Option<Date>,
    spread_day_counter: Option<DayCounter>,
    pub(super) discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl BondPricerSettings {
    /// Creates shared bond-pricer settings.
    pub(super) const fn new(anchor: BondPricingAnchor) -> Self {
        Self {
            anchor,
            settlement_date: None,
            spread_day_counter: None,
            discount_policy: None,
        }
    }

    /// Stores an explicit settlement date in the shared settings.
    #[must_use]
    pub(super) const fn with_settlement_date(mut self, settlement_date: Date) -> Self {
        self.settlement_date = Some(settlement_date);
        self
    }

    /// Stores an explicit spread day counter in the shared settings.
    #[must_use]
    pub(super) const fn with_spread_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.spread_day_counter = Some(day_counter);
        self
    }

    /// Returns the configured pricing anchor.
    #[must_use]
    pub(super) const fn anchor(&self) -> BondPricingAnchor {
        self.anchor
    }

    /// Resolves the discount index exclusively through the installed policy.
    fn discount_index(&self, trade: &impl BondTerms) -> Result<MarketIndex> {
        self.discount_policy
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Bond discount policy".into()))?
            .accept(trade.bond_leg())
    }
}

impl Default for BondPricerSettings {
    fn default() -> Self {
        Self::new(BondPricingAnchor::ZSpread(0.0))
    }
}

/// Deterministic clean-price, dirty-price, and Z-spread pricer for straight
/// fixed-rate bonds.
pub struct BondPricer {
    settings: BondPricerSettings,
}

impl BondPricer {
    /// Creates a straight-bond pricer using context-date settlement.
    #[must_use]
    pub const fn new(anchor: BondPricingAnchor) -> Self {
        Self {
            settings: BondPricerSettings::new(anchor),
        }
    }

    /// Uses an explicit settlement date.
    #[must_use]
    pub fn with_settlement_date(mut self, settlement_date: Date) -> Self {
        self.settings = self.settings.with_settlement_date(settlement_date);
        self
    }

    /// Overrides the spread day counter.
    #[must_use]
    pub fn with_spread_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.settings = self.settings.with_spread_day_counter(day_counter);
        self
    }

    /// Returns the configured price or spread anchor.
    #[must_use]
    pub const fn anchor(&self) -> BondPricingAnchor {
        self.settings.anchor()
    }
}

impl Default for BondPricer {
    fn default() -> Self {
        Self::new(BondPricingAnchor::ZSpread(0.0))
    }
}

#[derive(Clone, Copy)]
struct ResolvedCashflow<T> {
    payment_date: Date,
    amount: T,
}

#[derive(Clone, Copy)]
pub(super) struct CallStep {
    pub(super) call_price: Option<f64>,
}

impl CallStep {
    /// Creates a grid point without an issuer-call obstacle.
    const fn new() -> Self {
        Self { call_price: None }
    }
}

pub(super) struct LatticeTerms<T> {
    pub(super) maturity_time: f64,
    pub(super) steps: usize,
    pub(super) cashflows: Vec<T>,
    pub(super) calls: Vec<CallStep>,
}

#[derive(Clone)]
struct CallableConfiguration<T: Scalar = f64> {
    model: ModelConfiguration,
    alpha: f64,
    sigma_schedule: Vec<(f64, T)>,
    calibration: Option<(HullWhiteCalibrationQuality, Vec<Vec<f64>>)>,
    time_grid: TimeGridConfiguration,
    fourier_grid: Option<FourierGridConfiguration>,
}

#[derive(Clone, Copy)]
pub(super) enum NumericalMethod {
    Deterministic,
    Tree,
    BackwardFourierEvolution,
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

impl<T: BondTerms> HandleCashflows<T, BondState> for BondPricer {}

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

/// Returns principal cancelled when an issuer call terminates the bond.
fn call_principal(leg: &Leg<DualFwd>, call_date: Date) -> Result<f64> {
    leg.cashflows()
        .iter()
        .filter_map(|cashflow| match cashflow {
            CashflowType::Redemption(redemption) if redemption.payment_date() > call_date => {
                Some(redemption.amount())
            }
            _ => None,
        })
        .try_fold(0.0, |total, amount| amount.map(|value| total + value))
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
/// Maps contractual cashflows and issuer calls to the nearest points of a
/// uniform time grid shared by the two callable engines.
fn lattice_terms<T: Scalar>(
    leg: &Leg<DualFwd>,
    units: f64,
    call_schedule: &[CallScheduleEntry],
    cashflows: &[ResolvedCashflow<T>],
    settlement_date: Date,
    controls: TimeGridConfiguration,
) -> Result<LatticeTerms<T>> {
    let maturity_date = cashflows
        .last()
        .ok_or_else(|| QSError::InvalidValueErr("Bond has no remaining cashflows".into()))?
        .payment_date;
    let maturity_time = controls
        .day_counter()
        .year_fraction(settlement_date, maturity_date);
    if maturity_time <= 0.0 {
        return Err(QSError::InvalidValueErr(
            "Callable bond maturity must follow settlement".into(),
        ));
    }
    let target_steps = (maturity_time * controls.steps_per_year() as f64).ceil() as usize;
    let steps = target_steps.clamp(1, controls.max_steps());
    let step_for_date = |date: Date, can_be_maturity: bool| {
        let time = controls.day_counter().year_fraction(settlement_date, date);
        let raw = (time / maturity_time * steps as f64).round() as usize;
        if can_be_maturity {
            raw.clamp(1, steps)
        } else {
            raw.clamp(1, steps.saturating_sub(1).max(1))
        }
    };

    let mut step_cashflows = vec![T::zero(); steps + 1];
    for cashflow in cashflows {
        let step = step_for_date(cashflow.payment_date, true);
        step_cashflows[step] = step_cashflows[step].add_val(cashflow.amount);
    }

    let mut calls = vec![CallStep::new(); steps + 1];
    for event in call_schedule
        .iter()
        .filter(|event| event.end_date() >= settlement_date)
    {
        let start_time = controls
            .day_counter()
            .year_fraction(settlement_date, event.date());
        let end_time = controls
            .day_counter()
            .year_fraction(settlement_date, event.end_date());
        let discrete = event.date() == event.end_date();
        for (step, call) in calls.iter_mut().enumerate().take(steps) {
            let time = maturity_time * step as f64 / steps as f64;
            let eligible = if discrete {
                if event.date() == settlement_date {
                    step == 0
                } else {
                    step == step_for_date(event.date(), false)
                }
            } else {
                time + 1.0e-12 >= start_time && time <= end_time + 1.0e-12
            };
            if !eligible {
                continue;
            }
            let principal = if discrete {
                call_principal(leg, event.date())?
            } else {
                leg.cashflows()
                    .iter()
                    .try_fold(0.0, |principal, cashflow| -> Result<f64> {
                        match cashflow {
                            CashflowType::Redemption(redemption)
                                if step_for_date(redemption.payment_date(), true) > step =>
                            {
                                Ok(principal + redemption.amount()?)
                            }
                            _ => Ok(principal),
                        }
                    })?
            };
            if principal <= 0.0 {
                return Err(QSError::InvalidValueErr(format!(
                    "No principal remains at bond exercise step {step}",
                )));
            }
            let amount = event.strike() * principal / units;
            call.call_price = Some(
                call.call_price
                    .map_or(amount, |existing| existing.min(amount)),
            );
        }
    }

    Ok(LatticeTerms {
        maturity_time,
        steps,
        cashflows: step_cashflows,
        calls,
    })
}

/// Resolves the supported mean reversion and any fixed sigma schedule.
fn hull_white_parameters(configuration: &ModelConfiguration) -> Result<(f64, Vec<(f64, f64)>)> {
    match configuration {
        ModelConfiguration::HullWhite {
            alpha,
            parameter_source: ParameterSource::Fixed(parameters),
        } => {
            parameters.validate()?;
            Ok((*alpha, vec![(0.0, parameters.sigma)]))
        }
        ModelConfiguration::HullWhite {
            alpha,
            parameter_source: ParameterSource::Calibrated(_),
        } => Ok((*alpha, Vec::new())),
        ModelConfiguration::Lgm { .. } => Err(QSError::InvalidValueErr(
            "Callable-bond lattices currently support HullWhite; Lgm support is not yet implemented"
                .into(),
        )),
        ModelConfiguration::BrownianMotion { .. } => Err(QSError::InvalidValueErr(
            "BrownianMotion is not a short-rate callable-bond model".into(),
        )),
    }
}

/// Resolves and validates the instrument's pricer configuration carried by
/// a market-data response.
fn callable_configuration(
    market_data: &MarketData,
    discount_index: &MarketIndex,
    numerical_method: NumericalMethod,
    instrument_type: InstrumentType,
) -> Result<CallableConfiguration> {
    let configuration = market_data
        .pricer_configurations()
        .get(&instrument_type)
        .ok_or_else(|| {
            QSError::NotFoundErr(format!(
                "Pricer configuration not found for instrument type {instrument_type:?}"
            ))
        })?;
    configuration.validate()?;
    let model = configuration
        .model(discount_index)
        .cloned()
        .ok_or_else(|| {
            QSError::NotFoundErr(format!(
                "Bond pricing model not found for index {discount_index}"
            ))
        })?;
    let (time_grid, fourier_grid) = match (numerical_method, configuration.pricer()) {
        (NumericalMethod::Deterministic, actual) => {
            return Err(QSError::InvalidValueErr(format!(
                "Deterministic BondPricer cannot value an issuer call schedule with {actual:?}"
            )))
        }
        (NumericalMethod::Tree, PricerImplementation::TreeBased { time_grid }) => {
            (*time_grid, None)
        }
        (
            NumericalMethod::BackwardFourierEvolution,
            PricerImplementation::BackwardFourierEvolution {
                time_grid,
                fourier_grid,
            },
        ) => (*time_grid, Some(*fourier_grid)),
        (NumericalMethod::Tree, actual) => {
            return Err(QSError::InvalidValueErr(format!(
                "TreeBasedBondPricer requires a TreeBased context configuration, got {actual:?}"
            )))
        }
        (NumericalMethod::BackwardFourierEvolution, actual) => {
            return Err(QSError::InvalidValueErr(format!(
                "BackwardFourierEvolutionBondPricer requires a BackwardFourierEvolution context configuration, got {actual:?}"
            )))
        }
    };
    let (alpha, sigma_schedule) = hull_white_parameters(&model)?;
    Ok(CallableConfiguration {
        model,
        alpha,
        sigma_schedule,
        calibration: None,
        time_grid,
        fourier_grid,
    })
}

struct PrimalCurve<'a>(&'a dyn InterestRatesTermStructure<DualFwd>);

impl InterestRatesTermStructure<f64> for PrimalCurve<'_> {
    fn reference_date(&self) -> Date {
        self.0.reference_date()
    }
    fn discount_factor(&self, date: Date) -> Result<f64> {
        Ok(self.0.discount_factor(date)?.value())
    }
    fn forward_rate(
        &self,
        start: Date,
        end: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<f64> {
        Ok(self.0.forward_rate(start, end, comp, freq)?.value())
    }
    fn nodes(&self) -> Option<Vec<(Date, f64)>> {
        self.0.nodes().map(|nodes| {
            nodes
                .into_iter()
                .map(|(date, value)| (date, value.value()))
                .collect()
        })
    }
    fn day_counter(&self) -> Option<DayCounter> {
        self.0.day_counter()
    }
    fn discount_factor_from_time(&self, time: f64) -> Result<f64> {
        Ok(self.0.discount_factor_from_time(time)?.value())
    }
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<f64> {
        Ok(self.0.forward_rate_from_time(start, end)?.value())
    }
}

impl CallableConfiguration {
    fn resolve_calibration(
        &mut self,
        market_data: &MarketData,
        discount_index: &MarketIndex,
    ) -> Result<()> {
        let ModelConfiguration::HullWhite {
            parameter_source: ParameterSource::Calibrated(calibration),
            ..
        } = &self.model
        else {
            return Ok(());
        };
        let quotes = market_data.calibration_quotes().ok_or_else(|| {
            QSError::NotFoundErr(
                "Bond calibration quote details were not supplied by the market-data provider"
                    .into(),
            )
        })?;
        let curve = market_data
            .constructed_elements()
            .discount_curve(discount_index)
            .ok_or_else(|| {
                QSError::NotFoundErr(format!("Calibration curve not found for {discount_index}"))
            })?
            .curve();
        if curve.reference_date() != quotes.reference_date() {
            return Err(QSError::InvalidValueErr(
                "Calibrated bond model and quotes must share the curve reference date".into(),
            ));
        }
        self.time_grid = TimeGridConfiguration::new(
            self.time_grid.steps_per_year(),
            self.time_grid.max_steps(),
            curve
                .day_counter()
                .ok_or_else(|| QSError::ValueNotSetErr("Calibration curve day counter".into()))?,
        );
        let primal = PrimalCurve(&*curve);
        let mut model = HullWhite::new(self.alpha, &primal);
        model.calibrate_with_configuration(
            calibration,
            market_data.constructed_elements(),
            quotes,
            &primal,
            Level::Mid,
        )?;
        let volatility = model.vol_func().ok_or_else(|| {
            QSError::UnexpectedErr("Bond calibration produced no sigma schedule".into())
        })?;
        self.sigma_schedule = volatility.iter().copied().collect();
        if self.sigma_schedule.is_empty()
            || !self
                .sigma_schedule
                .windows(2)
                .all(|entries| entries[0].0 < entries[1].0)
        {
            return Err(QSError::InvalidValueErr("Calibrated bond sigma schedule must be nonempty and strictly ordered; select one tenor per expiry".into()));
        }
        self.calibration = Some((
            model
                .calibration_quality()
                .ok_or_else(|| QSError::UnexpectedErr("Missing bond calibration quality".into()))?,
            volatility
                .ift_sensitivities()
                .ok_or_else(|| {
                    QSError::UnexpectedErr(
                        "Missing bond calibration volatility sensitivities".into(),
                    )
                })?
                .clone(),
        ));
        Ok(())
    }

    fn ad_configuration(&self, market_data: &MarketData) -> Result<CallableConfiguration<DualFwd>> {
        let mut schedule = self
            .sigma_schedule
            .iter()
            .map(|&(time, sigma)| (time, DualFwd::scalar(sigma)))
            .collect::<Vec<_>>();
        if let Some((quality, sensitivities)) = &self.calibration {
            let ModelConfiguration::HullWhite {
                parameter_source: ParameterSource::Calibrated(calibration),
                ..
            } = &self.model
            else {
                return Err(QSError::UnexpectedErr(
                    "Missing calibrated bond configuration".into(),
                ));
            };
            let mut sampled_vols = Vec::with_capacity(quality.records.len());
            for record in &quality.records {
                let volatility = match calibration.source() {
                    CalibrationSource::Surface { market_index } => market_data
                        .constructed_elements()
                        .volatility_surface(market_index)
                        .ok_or_else(|| {
                            QSError::NotFoundErr(format!(
                                "Calibration surface not found for {market_index}"
                            ))
                        })?
                        .surface()
                        .volatility_from_period(record.expiry, record.effective_strike)?,
                    CalibrationSource::Cube { market_index } => {
                        let tenor = record
                            .identifier
                            .parse::<QuoteDetails>()?
                            .tenor()
                            .ok_or_else(|| {
                                QSError::NotFoundErr(format!(
                                    "Calibration tenor not found for {}",
                                    record.identifier
                                ))
                            })?;
                        market_data
                            .constructed_elements()
                            .volatility_cube(market_index)
                            .ok_or_else(|| {
                                QSError::NotFoundErr(format!(
                                    "Calibration cube not found for {market_index}"
                                ))
                            })?
                            .cube()
                            .volatility_from_period(record.expiry, tenor, record.effective_strike)?
                    }
                };
                sampled_vols.push(volatility.sub_val(DualFwd::scalar(record.market_vol)));
            }
            if sensitivities.len() != schedule.len()
                || sensitivities
                    .iter()
                    .any(|row| row.len() != sampled_vols.len())
            {
                return Err(QSError::UnexpectedErr(
                    "Invalid calibrated bond volatility Jacobian dimensions".into(),
                ));
            }
            for ((_, sigma), row) in schedule.iter_mut().zip(sensitivities) {
                for (&delta, &derivative) in sampled_vols.iter().zip(row) {
                    *sigma = sigma.add_val(delta.mul_val(DualFwd::scalar(derivative)));
                }
            }
        }
        Ok(CallableConfiguration {
            model: self.model.clone(),
            alpha: self.alpha,
            sigma_schedule: schedule,
            calibration: self.calibration.clone(),
            time_grid: self.time_grid,
            fourier_grid: self.fourier_grid,
        })
    }
}

/// Returns the exact conditional variance of the Hull-White Gaussian state
/// over one uniform time step.
pub(super) fn transition_variances<T: Scalar>(
    alpha: f64,
    schedule: &[(f64, T)],
    steps: usize,
    time_step: f64,
) -> Result<Vec<T>> {
    if schedule.is_empty() {
        return Err(QSError::ValueNotSetErr(
            "Resolved Hull-White sigma schedule".into(),
        ));
    }
    (0..steps)
        .map(|step| {
            let start = f64::from(u32::try_from(step).map_err(|_| {
                QSError::InvalidValueErr("Hull-White time index exceeds u32".into())
            })?) * time_step;
            Ok(hull_white_piecewise_variance(
                alpha,
                schedule,
                start,
                start + time_step,
            ))
        })
        .collect()
}

/// Integrates the OU innovation variance, including every sigma boundary
/// inside the propagation interval. The first sigma also applies from zero.
fn hull_white_piecewise_variance<T: Scalar>(
    alpha: f64,
    schedule: &[(f64, T)],
    start: f64,
    end: f64,
) -> T {
    let mut variance = T::zero();
    for (index, &(time, sigma)) in schedule.iter().enumerate() {
        let lower = if index == 0 { start } else { start.max(time) };
        let upper = schedule.get(index + 1).map_or(end, |next| end.min(next.0));
        if upper <= lower {
            continue;
        }
        let weight = if alpha.abs() < 1.0e-12 {
            upper - lower
        } else {
            (-2.0 * alpha * (end - upper)).exp() * (-(-2.0 * alpha * (upper - lower)).exp_m1())
                / (2.0 * alpha)
        };
        variance = variance.add_val(sigma.mul_val(sigma).mul_val(T::scalar(weight)));
    }
    variance
}

#[cfg(test)]
#[test]
fn piecewise_hull_white_variance_splits_transition_boundaries() {
    let schedule = [(0.5, 0.01_f64), (1.0, 0.02)];
    let variance = hull_white_piecewise_variance(0.05, &schedule, 0.75, 1.25);
    let expected = 0.01_f64.powi(2) * (-0.025_f64).exp() * (1.0 - (-0.025_f64).exp()) / 0.1
        + 0.02_f64.powi(2) * (1.0 - (-0.025_f64).exp()) / 0.1;
    assert!((variance - expected).abs() < 1.0e-15);
    assert!((hull_white_piecewise_variance(0.0, &schedule, 0.0, 1.25) - 0.0002).abs() < 1.0e-15);
}

/// Dispatches callable valuation to the selected numerical engine.
fn callable_value<T: Scalar>(
    discount_factor: impl FnMut(f64) -> Result<T>,
    terms: &LatticeTerms<T>,
    configuration: &CallableConfiguration<T>,
    spread: T,
) -> Result<T> {
    if let Some(grid) = configuration.fourier_grid {
        backward_fourier_value(
            discount_factor,
            terms,
            configuration.alpha,
            &configuration.sigma_schedule,
            spread,
            grid,
        )
    } else {
        lattice_value(
            discount_factor,
            terms,
            configuration.alpha,
            &configuration.sigma_schedule,
            spread,
        )
    }
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

/// Discounts all remaining straight-bond cashflows to settlement using a
/// continuously compounded Z-spread over the input curve.
fn settlement_value(
    cashflows: &[ResolvedCashflow<f64>],
    curve: &dyn InterestRatesTermStructure<DualFwd>,
    settlement_date: Date,
    day_counter: DayCounter,
    spread: f64,
) -> Result<f64> {
    let settlement_df = curve.discount_factor(settlement_date)?.value();
    if settlement_df <= 0.0 {
        return Err(QSError::InvalidValueErr(
            "Discount factor at settlement must be positive".into(),
        ));
    }
    cashflows.iter().try_fold(0.0, |value, cashflow| {
        let time = day_counter.year_fraction(settlement_date, cashflow.payment_date);
        let discount = curve.discount_factor(cashflow.payment_date)?.value() / settlement_df;
        Ok(f64::mul_add(
            cashflow.amount,
            discount * (-spread * time).exp(),
            value,
        ))
    })
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

/// Solves the deterministic Z-spread that reproduces a dirty settlement
/// value.
fn solve_z_spread(
    cashflows: &[ResolvedCashflow<f64>],
    curve: &dyn InterestRatesTermStructure<DualFwd>,
    settlement_date: Date,
    day_counter: DayCounter,
    target_value: f64,
) -> Result<f64> {
    bracketed_root(
        |spread| {
            Ok(
                settlement_value(cashflows, curve, settlement_date, day_counter, spread)?
                    - target_value,
            )
        },
        -0.05,
        0.05,
        -10.0,
    )
}

/// Solves the OAS that makes the selected callable engine reproduce a dirty
/// settlement value.
fn solve_oas(
    terms: &LatticeTerms<f64>,
    curve: &dyn InterestRatesTermStructure<DualFwd>,
    configuration: &CallableConfiguration,
    target_value: f64,
) -> Result<f64> {
    bracketed_root(
        |spread| {
            Ok(callable_value(
                |time| Ok(curve.discount_factor_from_time(time)?.value()),
                terms,
                configuration,
                spread,
            )? - target_value)
        },
        -0.05,
        0.05,
        -10.0,
    )
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

/// Records the selected bond valuation on the AD tape and stores the spread
/// leaf used by the common sensitivity request.
fn build_ad_value(
    trade: &impl BondTerms,
    state: &mut BondState,
    callable: Option<&CallableConfiguration>,
) -> Result<f64> {
    Tape::start_recording_fwd();
    Tape::set_mark_fwd();
    state.put_pillars_on_tape()?;
    for cube in state
        .market_data
        .as_mut()
        .ok_or_else(|| QSError::ValueNotSetErr("Bond market data".into()))?
        .constructed_elements_mut()
        .volatility_cubes_mut()
        .values_mut()
    {
        cube.cube_mut().put_pillars_on_tape();
    }
    let ad_configuration = callable
        .map(|configuration| {
            configuration.ad_configuration(
                state
                    .market_data
                    .as_ref()
                    .ok_or_else(|| QSError::ValueNotSetErr("Bond market data".into()))?,
            )
        })
        .transpose()?;

    let spread = DualFwd::new(state.resolved_spread);
    let resolved_leg = resolve_leg(
        trade.bond_leg(),
        state
            .market_data
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Bond market data".into()))?,
        state.evaluation_date,
    )?;
    let cashflows = remaining_cashflows(&resolved_leg, state.settlement_date)?;
    let lattice = callable
        .map(|configuration| {
            lattice_terms(
                &resolved_leg,
                trade.units(),
                trade.call_schedule(),
                &cashflows,
                state.settlement_date,
                configuration.time_grid,
            )
        })
        .transpose()?;
    state.legs = vec![resolved_leg];
    let curve = state
        .get_discount_curve_element(&state.discount_index)?
        .curve();
    let unsigned_value =
        if let Some((terms, configuration)) = lattice.as_ref().zip(ad_configuration.as_ref()) {
            callable_value(
                |time| curve.discount_factor_from_time(time),
                terms,
                configuration,
                spread,
            )?
        } else {
            let mut unsigned_value = DualFwd::zero();
            for cashflow in &cashflows {
                let amount = cashflow.amount;
                let payment_date = cashflow.payment_date;
                let time = state
                    .spread_day_counter
                    .year_fraction(state.evaluation_date, payment_date);
                let exponent: DualFwd = (spread * -time).into();
                let spread_discount = exponent.exp();
                let discount = curve.discount_factor(payment_date)?;
                unsigned_value = (unsigned_value + amount * discount * spread_discount).into();
            }
            unsigned_value
        };
    drop(curve);

    let value: DualFwd = (unsigned_value * trade.side().sign()).into();
    state.value = Some(value);
    state.spread_node = Some(spread);
    Tape::stop_recording_fwd();
    Ok(value.value())
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

pub(super) trait BondPricingEngine {
    /// Returns the quote and settlement conventions shared by both engines.
    fn settings(&self) -> &BondPricerSettings;

    /// Returns the numerical callable engine selected by the concrete type.
    fn numerical_method(&self) -> NumericalMethod;
}

impl BondPricingEngine for BondPricer {
    fn settings(&self) -> &BondPricerSettings {
        &self.settings
    }

    fn numerical_method(&self) -> NumericalMethod {
        NumericalMethod::Deterministic
    }
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
    let settings = engine.settings();
    settings.anchor.validate()?;
    let callable = !trade.call_schedule().is_empty();
    if callable && matches!(settings.anchor, BondPricingAnchor::ZSpread(_)) {
        return Err(QSError::InvalidValueErr(
            "An issuer-call bond must be anchored by clean price, dirty price, or OAS".into(),
        ));
    }
    if !callable && matches!(settings.anchor, BondPricingAnchor::OAS(_)) {
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
    let settlement_date = settings.settlement_date.unwrap_or(evaluation_date);
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

    let discount_index = settings.discount_index(trade)?;
    let mut model_configuration = if callable {
        let configurations = context.handle_request(
            &MarketDataRequest::default()
                .with_pricer_configuration_request(vec![trade.instrument_type()]),
        )?;
        Some(callable_configuration(
            &configurations,
            &discount_index,
            engine.numerical_method(),
            trade.instrument_type(),
        )?)
    } else {
        None
    };
    let mut requested_indices = HashSet::from([discount_index.clone()]);
    let mut constructed_elements = vec![ConstructedElementRequest::DiscountCurve {
        market_index: discount_index.clone(),
    }];
    let mut fixing_requests = Vec::new();
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
            if requested_indices.insert(index.clone()) {
                constructed_elements.push(ConstructedElementRequest::DiscountCurve {
                    market_index: index.clone(),
                });
            }
            if fixing_date < evaluation_date {
                fixing_requests.push(FixingRequest::new(index.clone(), fixing_date));
            }
        }
    }
    let calibration_request =
        model_configuration
            .as_ref()
            .and_then(|configuration| match &configuration.model {
                ModelConfiguration::HullWhite {
                    parameter_source: ParameterSource::Calibrated(calibration),
                    ..
                } => Some(calibration.clone()),
                _ => None,
            });
    if let Some(calibration) = &calibration_request {
        let (index, volatility_request) = match calibration.source() {
            CalibrationSource::Surface { market_index } => (
                market_index,
                ConstructedElementRequest::VolatilitySurface {
                    market_index: market_index.clone(),
                },
            ),
            CalibrationSource::Cube { market_index } => (
                market_index,
                ConstructedElementRequest::VolatilityCube {
                    market_index: market_index.clone(),
                },
            ),
        };
        constructed_elements.push(volatility_request);
        if requested_indices.insert(index.clone()) {
            constructed_elements.push(ConstructedElementRequest::DiscountCurve {
                market_index: index.clone(),
            });
        }
    }
    let mut market_request =
        MarketDataRequest::default().with_constructed_elements_request(constructed_elements);
    if let Some(calibration) = calibration_request {
        market_request = market_request.with_calibration_request(calibration);
    }
    if !fixing_requests.is_empty() {
        market_request = market_request.with_fixings_request(fixing_requests);
    }
    if callable {
        market_request =
            market_request.with_pricer_configuration_request(vec![trade.instrument_type()]);
    }
    let market_data = context.handle_request(&market_request)?;
    if let Some(configuration) = &mut model_configuration {
        configuration.resolve_calibration(&market_data, &discount_index)?;
    }
    let element = market_data
        .constructed_elements()
        .discount_curve(&discount_index)
        .ok_or_else(|| {
            QSError::NotFoundErr(format!(
                "Discount curve not found for index {discount_index}"
            ))
        })?;
    let curve = element.curve();
    let spread_day_counter = settings
        .spread_day_counter
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
    let lattice = model_configuration
        .as_ref()
        .map(|configuration| {
            lattice_terms(
                &resolved_leg,
                trade.units(),
                trade.call_schedule(),
                &cashflows,
                settlement_date,
                configuration.time_grid,
            )
            .map(|terms| (terms, configuration.clone()))
        })
        .transpose()?;
    let principal = outstanding_principal(&resolved_leg, settlement_date)?;
    let dirty_quote = quoted_dirty_price(
        settings.anchor,
        &resolved_leg,
        trade.units(),
        settlement_date,
        principal,
    )?;
    let target_settlement_value = dirty_quote.map(|price| price * principal / trade.units());
    let resolved_spread = match settings.anchor {
        BondPricingAnchor::CleanPrice(_) | BondPricingAnchor::DirtyPrice(_) => {
            let target = target_settlement_value
                .ok_or_else(|| QSError::ValueNotSetErr("Dirty settlement value".into()))?;
            if let Some((terms, configuration)) = &lattice {
                solve_oas(terms, &*curve, configuration, target)?
            } else {
                solve_z_spread(
                    &cashflows,
                    &*curve,
                    settlement_date,
                    spread_day_counter,
                    target,
                )?
            }
        }
        BondPricingAnchor::ZSpread(spread) | BondPricingAnchor::OAS(spread) => spread,
    };
    let model_settlement_value = if let Some((terms, configuration)) = &lattice {
        callable_value(
            |time| Ok(curve.discount_factor_from_time(time)?.value()),
            terms,
            configuration,
            resolved_spread,
        )?
    } else {
        settlement_value(
            &cashflows,
            &*curve,
            settlement_date,
            spread_day_counter,
            resolved_spread,
        )?
    };
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
    let value = build_ad_value(trade, &mut state, model_configuration.as_ref())?;

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

/// Builds the complete market-data request shared by both callable-bond
/// implementations.
pub(super) fn bond_market_data_request(
    settings: &BondPricerSettings,
    trade: &impl BondTerms,
) -> Option<MarketDataRequest> {
    let index = settings.discount_index(trade).ok()?;
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
                bond_market_data_request(&self.settings, trade)
            }

            fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
                self.settings.discount_policy = Some(policy);
            }

            fn discount_policy(&self) -> Option<&Self::Policy> {
                self.settings.discount_policy.as_deref()
            }
        }
    };
}

pub(super) use impl_bond_pricer;

impl_bond_pricer!(BondPricer, FixedRateBondTrade<DualFwd>);

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use super::*;
    use crate::{
        core::{
            collateral::FixedIncomeDiscountPolicy,
            elements::curveelement::DiscountCurveElement,
            marketdatahandling::constructedelementstore::ConstructedElementStore,
            pricerconfiguration::{PricerConfiguration, PricingModelConfiguration},
            pricingcontext::PricingContext,
        },
        currencies::currency::Currency,
        instruments::{
            cashflows::makeleg::PaymentStructure,
            fixedincome::{
                callschedule::CallScheduleEntry, makefixedratebond::MakeFixedRateBond,
                makesteppedcouponbond::MakeSteppedCouponBond,
            },
        },
        models::{
            hullwhite::hullwhitemodel::HullWhite,
            lgm::lgmcomponents::LgmRateModel,
            modelconfiguration::{GaussianRateModelParameters, ParameterSource},
        },
        quotes::{
            quote::{Quote, QuoteLevels},
            quotestore::{QuoteStore, QuoteStoreRecords},
        },
        rates::{
            compounding::Compounding, interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::{
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        volatility::{
            modelcalibration::{CalibrationBasket, ModelCalibrationConfiguration},
            volatilitycubeconfiguration::VolatilityCubeConfiguration,
            volatilityindexing::{SmileType, Strike, VolatilityType},
            volatilitysurfaceconfiguration::VolatilitySurfaceConfiguration,
        },
    };

    const FACE: f64 = 100.0;
    const RATE: f64 = 0.03;
    const ALPHA: f64 = 0.05;
    const SIGMA: f64 = 0.01;

    #[test]
    fn piecewise_native_engines_match_bond_option_and_straight_curve_fit() -> Result<()> {
        let evaluation = Date::new(2025, 1, 1);
        let curve = FlatForwardTermStructure::new(
            evaluation,
            RATE,
            RateDefinition::new(
                DayCounter::Actual365,
                Compounding::Continuous,
                Frequency::Annual,
            ),
        );
        let schedule = [(0.5, 0.01), (1.0, 0.02), (2.5, 0.005)];
        let mut terms = LatticeTerms {
            maturity_time: 3.0,
            steps: 384,
            cashflows: vec![0.0; 385],
            calls: vec![CallStep::new(); 385],
        };
        terms.cashflows[384] = FACE;
        terms.calls[256].call_price = Some(95.0);
        let variance = hull_white_piecewise_variance(ALPHA, &schedule, 0.0, 2.0);
        let unit_variance = hull_white_piecewise_variance(ALPHA, &[(0.0, 1.0)], 0.0, 2.0);
        let equivalent_sigma = (variance / unit_variance).sqrt();
        let benchmark = FACE
            * (curve.discount_factor_from_time(3.0)?
                - HullWhite::new(ALPHA, &curve).bond_call_price(
                    2.0,
                    3.0,
                    0.95,
                    equivalent_sigma,
                    &curve,
                )?);
        let tree = lattice_value(
            |time| curve.discount_factor_from_time(time),
            &terms,
            ALPHA,
            &schedule,
            0.0,
        )?;
        let fourier = backward_fourier_value(
            |time| curve.discount_factor_from_time(time),
            &terms,
            ALPHA,
            &schedule,
            0.0,
            FourierGridConfiguration::new(1024, 10.0),
        )?;
        assert!(
            (tree - benchmark).abs() < 0.08,
            "piecewise tree {tree} vs bond-option benchmark {benchmark}"
        );
        assert!(
            (fourier - benchmark).abs() < 0.10,
            "piecewise Fourier {fourier} vs bond-option benchmark {benchmark}"
        );
        terms.calls[256].call_price = None;
        let straight = FACE * curve.discount_factor_from_time(3.0)?;
        assert!(
            (lattice_value(
                |time| curve.discount_factor_from_time(time),
                &terms,
                ALPHA,
                &schedule,
                0.0
            )? - straight)
                .abs()
                < 1.0e-10
        );
        assert!(
            (backward_fourier_value(
                |time| curve.discount_factor_from_time(time),
                &terms,
                ALPHA,
                &schedule,
                0.0,
                FourierGridConfiguration::new(1024, 10.0)
            )? - straight)
                .abs()
                < 0.10
        );
        Ok(())
    }

    fn example_calibrated_context(
        use_fourier: bool,
        bump: Option<(&str, f64)>,
    ) -> Result<PricingContext> {
        let mut records: QuoteStoreRecords =
            serde_json::from_str(include_str!("../../../examples/hullwhite/data/quotes.json"))
                .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        if let Some((identifier, amount)) = bump {
            let quote = records
                .quotes
                .iter_mut()
                .find(|quote| quote.identifier == identifier)
                .ok_or_else(|| QSError::NotFoundErr(identifier.into()))?;
            quote.mid = quote.mid.map(|value| value + amount);
        }
        let quotes = QuoteStore::try_from(records)?;
        let curve_json: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/hullwhite/data/curve_specs.json"
        ))
        .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        let curves = serde_json::from_value(curve_json["curve_specs"].clone())
            .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        let surface_json: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/hullwhite/data/vol_specs.json"
        ))
        .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        let surfaces: Vec<VolatilitySurfaceConfiguration> =
            serde_json::from_value(surface_json["volatility_surfaces"].clone())
                .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        let time_grid = TimeGridConfiguration::new(48, 4096, DayCounter::Actual365);
        let pricer = if use_fourier {
            PricerImplementation::BackwardFourierEvolution {
                time_grid,
                fourier_grid: FourierGridConfiguration::new(512, 10.0),
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
                    alpha: 0.05,
                    parameter_source: ParameterSource::Calibrated(
                        ModelCalibrationConfiguration::new(CalibrationSource::Surface {
                            market_index: MarketIndex::SOFR,
                        })
                        .with_strike(Strike::Atm),
                    ),
                },
            )],
        );
        let mut context = PricingContext::new()
            .with_quote_store(quotes)
            .with_curve_configurations(curves)
            .with_volatility_surface_configurations(surfaces)
            .with_pricer_configurations(vec![configuration]);
        context.initialize()?;
        Ok(context)
    }

    struct RecordingProvider<'a> {
        context: &'a PricingContext,
        calibrated_request_seen: RefCell<bool>,
    }

    impl MarketDataProvider for RecordingProvider<'_> {
        fn evaluation_date(&self) -> Date {
            self.context.evaluation_date()
        }
        fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData> {
            if request.calibration_request().is_some() {
                let elements = request.constructed_elements_request().ok_or_else(|| {
                    QSError::ValueNotSetErr("Calibration elements request".into())
                })?;
                assert!(elements.iter().any(|element| matches!(
                    element,
                    ConstructedElementRequest::VolatilitySurface {
                        market_index: MarketIndex::SOFR
                    }
                )));
                assert!(elements.iter().any(|element| matches!(
                    element,
                    ConstructedElementRequest::DiscountCurve {
                        market_index: MarketIndex::SOFR
                    }
                )));
                *self.calibrated_request_seen.borrow_mut() = true;
            }
            self.context.handle_request(request)
        }
    }

    #[test]
    fn calibrated_surface_native_engines_preserve_schedule_and_quote_risk() -> Result<()> {
        let quote = "CapletFloorlet_USD_SOFR_3M_2Y_Absolute_0.045_Straddle_Black";
        let bump = 1.0e-4;
        let mut prices = Vec::new();
        for use_fourier in [false, true] {
            Tape::rewind_to_init_fwd();
            let context = example_calibrated_context(use_fourier, None)?;
            let market = MarketData::new(HashMap::new(), context.constructed_elements().clone())
                .with_calibration_quotes(context.quote_store().clone())
                .with_pricer_configurations(HashMap::from([(
                    InstrumentType::SteppedCouponBond,
                    context.pricer_configurations()[0].clone(),
                )]));
            let method = if use_fourier {
                NumericalMethod::BackwardFourierEvolution
            } else {
                NumericalMethod::Tree
            };
            let mut resolved = callable_configuration(
                &market,
                &MarketIndex::SOFR,
                method,
                InstrumentType::SteppedCouponBond,
            )?;
            resolved.resolve_calibration(&market, &MarketIndex::SOFR)?;
            assert_eq!(resolved.sigma_schedule.len(), 9);
            assert!(resolved
                .sigma_schedule
                .windows(2)
                .any(|entries| (entries[0].1 - entries[1].1).abs() > 1.0e-5));
            let curve = market
                .constructed_elements()
                .discount_curve(&MarketIndex::SOFR)
                .ok_or_else(|| QSError::NotFoundErr("SOFR curve".into()))?
                .curve();
            let primal = PrimalCurve(&*curve);
            let ModelConfiguration::HullWhite {
                parameter_source, ..
            } = &resolved.model
            else {
                return Err(QSError::UnexpectedErr(
                    "Expected Hull-White configuration".into(),
                ));
            };
            let lgm = LgmRateModel::from_configuration(
                &ModelConfiguration::Lgm {
                    lambda: 0.05,
                    parameter_source: parameter_source.clone(),
                },
                &primal,
                market.constructed_elements(),
                context.quote_store(),
                Level::Mid,
            )?;
            assert_eq!(resolved.sigma_schedule, lgm.sigma_schedule());
            drop(curve);
            let evaluation = context.evaluation_date();
            let trade = bond(
                evaluation,
                evaluation + 6 * 365,
                0.05,
                PaymentStructure::Bullet,
                vec![CallScheduleEntry::window(
                    evaluation + 3 * 365,
                    evaluation + 5 * 365,
                    100.0,
                )],
            );
            let provider = RecordingProvider {
                context: &context,
                calibrated_request_seen: RefCell::new(false),
            };
            let evaluate = |provider: &RecordingProvider<'_>, requests: &[Request]| {
                if use_fourier {
                    fourier_pricer(BondPricingAnchor::OAS(0.002))
                        .evaluate(&trade, requests, provider)
                } else {
                    tree_pricer(BondPricingAnchor::OAS(0.002)).evaluate(&trade, requests, provider)
                }
            };
            let results = evaluate(&provider, &[Request::Value, Request::Sensitivities])?;
            assert!(*provider.calibrated_request_seen.borrow());
            let price = results
                .price()
                .ok_or_else(|| QSError::ValueNotSetErr("Calibrated bond price".into()))?;
            assert!(price.is_finite() && price > 0.0);
            prices.push(price);
            let risk = results
                .sensitivities()
                .ok_or_else(|| QSError::ValueNotSetErr("Calibrated bond sensitivities".into()))?;
            assert!(risk
                .instrument_keys()
                .iter()
                .any(|label| label.starts_with("OIS_")));
            assert!(risk
                .instrument_keys()
                .iter()
                .any(|label| label == "TEST-BOND.OAS"));
            let adjoint = risk
                .instrument_keys()
                .iter()
                .zip(risk.exposure())
                .find_map(|(label, value)| (label == quote).then_some(*value))
                .ok_or_else(|| QSError::NotFoundErr("Calibrated caplet quote risk".into()))?;
            assert!(
                adjoint.abs() > 1.0e-4,
                "vol quote sensitivity must not be discarded"
            );
            let mut bumped_prices = Vec::new();
            for amount in [bump, -bump] {
                Tape::rewind_to_init_fwd();
                let bumped = example_calibrated_context(use_fourier, Some((quote, amount)))?;
                let bumped_provider = RecordingProvider {
                    context: &bumped,
                    calibrated_request_seen: RefCell::new(false),
                };
                bumped_prices.push(
                    evaluate(&bumped_provider, &[Request::Value])?
                        .price()
                        .ok_or_else(|| QSError::ValueNotSetErr("Bumped bond price".into()))?,
                );
            }
            let finite_difference = (bumped_prices[0] - bumped_prices[1]) / (2.0 * bump);
            eprintln!("Calibrated HullWhite Fourier={use_fourier}: PV={price:.8}, caplet AD={adjoint:.8}, rebuilt-market bump={finite_difference:.8}");
            assert!(
                (adjoint - finite_difference).abs() < 0.03 * finite_difference.abs().max(0.01),
                "Fourier={use_fourier}: AD {adjoint} vs rebuilt surface bump {finite_difference}"
            );
        }
        assert!(
            (prices[0] - prices[1]).abs() < 0.20,
            "calibrated tree {} vs Fourier {}",
            prices[0],
            prices[1]
        );
        Ok(())
    }

    #[test]
    fn calibrated_cube_native_engines_report_swaption_risk() -> Result<()> {
        for use_fourier in [false, true] {
            Tape::rewind_to_init_fwd();
            let evaluation = Date::new(2025, 1, 1);
            let (base, _) = provider_with_method(evaluation, 24, use_fourier);
            let mut quotes = QuoteStore::new(evaluation);
            let mut identifiers = Vec::new();
            for expiry in ["1Y", "2Y", "3Y"] {
                for tenor in ["5Y", "10Y"] {
                    for strike in ["0.02", "0.06"] {
                        let identifier =
                            format!("Swaption_USD_SOFR_{expiry}_{tenor}_Absolute_{strike}_Black");
                        quotes.add_quote(Quote::new(
                            identifier.parse()?,
                            QuoteLevels::with_mid(if expiry == "1Y" { 0.20 } else { 0.30 }),
                        ));
                        identifiers.push(identifier);
                    }
                }
            }
            let time_grid = TimeGridConfiguration::new(24, 4096, DayCounter::Actual365);
            let implementation = if use_fourier {
                PricerImplementation::BackwardFourierEvolution {
                    time_grid,
                    fourier_grid: FourierGridConfiguration::new(256, 10.0),
                }
            } else {
                PricerImplementation::TreeBased { time_grid }
            };
            let calibration = ModelCalibrationConfiguration::new(CalibrationSource::Cube {
                market_index: MarketIndex::SOFR,
            })
            .with_calibration_basket(
                CalibrationBasket::all()
                    .with_expiries(vec![
                        Period::new(1, TimeUnit::Years),
                        Period::new(2, TimeUnit::Years),
                    ])
                    .with_tenors(vec![Period::new(5, TimeUnit::Years)])
                    .with_strike(Strike::Atm),
            );
            let configuration = PricerConfiguration::new(
                InstrumentType::SteppedCouponBond,
                implementation,
                vec![PricingModelConfiguration::new(
                    MarketIndex::SOFR,
                    ModelConfiguration::HullWhite {
                        alpha: ALPHA,
                        parameter_source: ParameterSource::Calibrated(calibration),
                    },
                )],
            );
            let mut context = PricingContext::new()
                .with_quote_store(quotes)
                .with_constructed_elements(base.market_data.constructed_elements().clone())
                .with_volatility_cube_configurations(vec![VolatilityCubeConfiguration::new(
                    MarketIndex::SOFR,
                    VolatilityType::Black,
                    SmileType::Strike,
                    identifiers,
                )])
                .with_pricer_configurations(vec![configuration]);
            context.initialize()?;
            let trade = bond(
                evaluation,
                evaluation + 4 * 365,
                0.05,
                PaymentStructure::Bullet,
                vec![CallScheduleEntry::new(evaluation + 3 * 365, 100.0)],
            );
            let results = if use_fourier {
                fourier_pricer(BondPricingAnchor::OAS(0.0)).evaluate(
                    &trade,
                    &[Request::Value, Request::Sensitivities],
                    &context,
                )?
            } else {
                tree_pricer(BondPricingAnchor::OAS(0.0)).evaluate(
                    &trade,
                    &[Request::Value, Request::Sensitivities],
                    &context,
                )?
            };
            assert!(results
                .price()
                .is_some_and(|price| price.is_finite() && price > 0.0));
            let risk = results
                .sensitivities()
                .ok_or_else(|| QSError::ValueNotSetErr("Cube bond sensitivities".into()))?;
            assert!(
                risk.instrument_keys()
                    .iter()
                    .zip(risk.exposure())
                    .any(|(label, exposure)| label.starts_with("Swaption_")
                        && exposure.abs() > 1.0e-4)
            );
        }
        Tape::rewind_to_init_fwd();
        Ok(())
    }

    fn policy() -> Box<dyn DiscountPolicy> {
        Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_risk_free_index(Currency::USD, MarketIndex::SOFR),
        )
    }

    fn fixed_pricer(anchor: BondPricingAnchor) -> BondPricer {
        let mut pricer = BondPricer::new(anchor);
        pricer.set_discount_policy(policy());
        pricer
    }

    fn tree_pricer(anchor: BondPricingAnchor) -> TreeBasedBondPricer {
        let mut pricer = TreeBasedBondPricer::new(anchor);
        pricer.set_discount_policy(policy());
        pricer
    }

    fn fourier_pricer(anchor: BondPricingAnchor) -> BackwardFourierEvolutionBondPricer {
        let mut pricer = BackwardFourierEvolutionBondPricer::new(anchor);
        pricer.set_discount_policy(policy());
        pricer
    }

    #[derive(Clone)]
    struct TestProvider {
        evaluation_date: Date,
        market_data: MarketData,
    }

    impl MarketDataProvider for TestProvider {
        fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
            Ok(self.market_data.clone())
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    fn provider(
        evaluation_date: Date,
        steps_per_year: usize,
    ) -> (TestProvider, FlatForwardTermStructure<f64>) {
        provider_with_method(evaluation_date, steps_per_year, false)
    }

    fn fourier_provider(
        evaluation_date: Date,
        steps_per_year: usize,
    ) -> (TestProvider, FlatForwardTermStructure<f64>) {
        provider_with_method(evaluation_date, steps_per_year, true)
    }

    fn provider_with_method(
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

    fn bond(
        evaluation_date: Date,
        maturity_date: Date,
        coupon: f64,
        _structure: PaymentStructure,
        call_schedule: Vec<CallScheduleEntry>,
    ) -> SteppedCouponBondTrade<DualFwd> {
        let instrument = MakeSteppedCouponBond::<DualFwd>::default()
            .with_identifier("TEST-BOND".into())
            .with_start_date(evaluation_date)
            .with_maturity_date(maturity_date)
            .with_units(FACE)
            .with_notional(FACE)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Semiannual)
            .with_coupons(vec![CashflowType::fixed(coupon)])
            .with_notional_percentages(vec![1.0])
            .with_call_schedule(call_schedule)
            .build()
            .unwrap_or_else(|error| panic!("test bond: {error}"));
        SteppedCouponBondTrade::new(
            instrument,
            evaluation_date,
            FACE,
            crate::core::trade::Side::LongReceive,
        )
    }

    fn value(
        pricer: &TreeBasedBondPricer,
        trade: &SteppedCouponBondTrade<DualFwd>,
        provider: &TestProvider,
    ) -> f64 {
        pricer
            .evaluate(trade, &[Request::Value], provider)
            .unwrap_or_else(|error| panic!("bond value: {error}"))
            .price()
            .unwrap_or_else(|| panic!("bond value must be present"))
    }

    fn fixed_bond(
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
        )
    }

    fn fixed_value(
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

    fn fourier_value(
        pricer: &BackwardFourierEvolutionBondPricer,
        trade: &SteppedCouponBondTrade<DualFwd>,
        provider: &TestProvider,
    ) -> f64 {
        pricer
            .evaluate(trade, &[Request::Value], provider)
            .unwrap_or_else(|error| panic!("Fourier bond value: {error}"))
            .price()
            .unwrap_or_else(|| panic!("Fourier bond value must be present"))
    }

    #[test]
    fn discount_policy_controls_curve_requests_and_bond_values() -> Result<()> {
        let evaluation = Date::new(2025, 1, 1);
        let maturity = Date::new(2027, 1, 1);
        let selected_index = MarketIndex::Other("USD-TREASURY".into());
        let definition = RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        );
        let mut elements = ConstructedElementStore::default();
        for (index, rate) in [(MarketIndex::SOFR, RATE), (selected_index.clone(), 0.08)] {
            elements.discount_curves_mut().insert(
                index.clone(),
                DiscountCurveElement::new(
                    index,
                    Rc::new(RefCell::new(FlatForwardTermStructure::new(
                        evaluation,
                        DualFwd::from(rate),
                        definition,
                    ))),
                ),
            );
        }
        let market = TestProvider {
            evaluation_date: evaluation,
            market_data: MarketData::new(HashMap::new(), elements),
        };
        let selected_policy = || -> Box<dyn DiscountPolicy> {
            Box::new(
                FixedIncomeDiscountPolicy::new(false)
                    .with_risk_free_index(Currency::USD, selected_index.clone()),
            )
        };
        let expected = FACE * (-0.08_f64 * 2.0).exp();
        let straight = fixed_bond(evaluation, maturity, 0.0);
        let stepped = bond(evaluation, maturity, 0.0, PaymentStructure::Zero, vec![]);
        let mut fixed = BondPricer::new(BondPricingAnchor::ZSpread(0.0));
        let mut tree = TreeBasedBondPricer::new(BondPricingAnchor::ZSpread(0.0));
        let mut fourier = BackwardFourierEvolutionBondPricer::new(BondPricingAnchor::ZSpread(0.0));
        assert!(fixed
            .evaluate(&straight, &[Request::Value], &market)
            .is_err());
        assert!(tree.evaluate(&stepped, &[Request::Value], &market).is_err());
        assert!(fourier
            .evaluate(&stepped, &[Request::Value], &market)
            .is_err());
        fixed.set_discount_policy(selected_policy());
        tree.set_discount_policy(selected_policy());
        fourier.set_discount_policy(selected_policy());
        for request in [
            fixed.market_data_request(&straight),
            tree.market_data_request(&stepped),
            fourier.market_data_request(&stepped),
        ] {
            let request =
                request.ok_or_else(|| QSError::NotFoundErr("bond curve request".into()))?;
            assert!(
                matches!(request.constructed_elements_request().and_then(|elements| elements.first()),
                Some(ConstructedElementRequest::DiscountCurve { market_index }) if market_index == &selected_index)
            );
        }
        for price in [
            fixed_value(&fixed, &straight, &market),
            value(&tree, &stepped, &market),
            fourier_value(&fourier, &stepped, &market),
        ] {
            assert!(
                (price - expected).abs() < 1.0e-10,
                "policy-selected {price} vs analytic {expected}"
            );
        }
        assert!(
            expected
                < fixed_value(
                    &fixed_pricer(BondPricingAnchor::ZSpread(0.0)),
                    &straight,
                    &market
                )
        );
        Ok(())
    }

    #[test]
    fn z_spread_price_anchor_round_trip_duration_and_risk() {
        let evaluation_date = Date::new(2025, 1, 1);
        let (provider, _) = provider(evaluation_date, 48);
        let trade = fixed_bond(evaluation_date, Date::new(2030, 1, 1), 0.04);
        let spread = 0.0125;
        let direct = fixed_pricer(BondPricingAnchor::ZSpread(spread));
        let direct_results = direct
            .evaluate(
                &trade,
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
        let implied = fixed_pricer(BondPricingAnchor::DirtyPrice(quoted_price));
        let implied_results = implied
            .evaluate(&trade, &[Request::Value, Request::ZSpread], &provider)
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
            &fixed_pricer(BondPricingAnchor::ZSpread(spread + bump)),
            &trade,
            &provider,
        ) - fixed_value(
            &fixed_pricer(BondPricingAnchor::ZSpread(spread - bump)),
            &trade,
            &provider,
        )) / (2.0 * bump);
        assert!((spread_risk - finite_difference).abs() < 1.0e-4);
    }

    fn callable_zero_coupon_value(steps_per_year: usize) -> (f64, f64) {
        let evaluation_date = Date::new(2025, 1, 1);
        let call_date = Date::new(2026, 1, 1);
        let maturity_date = Date::new(2027, 1, 1);
        let (provider, curve) = provider(evaluation_date, steps_per_year);
        let strike = 0.95;
        let trade = bond(
            evaluation_date,
            maturity_date,
            0.0,
            PaymentStructure::Zero,
            vec![CallScheduleEntry::new(call_date, strike * FACE)],
        );
        let lattice = value(&tree_pricer(BondPricingAnchor::OAS(0.0)), &trade, &provider);
        let model = HullWhite::new(ALPHA, &curve).with_constant_volatility(SIGMA);
        let option = model
            .bond_call_price(1.0, 2.0, strike, SIGMA, &curve)
            .unwrap_or_else(|error| panic!("closed-form bond call: {error}"));
        let benchmark = FACE
            * (curve
                .discount_factor_from_time(2.0)
                .unwrap_or_else(|error| panic!("discount factor: {error}"))
                - option);
        (lattice, benchmark)
    }

    #[test]
    fn callable_zero_coupon_matches_hull_white_bond_option_formula() {
        let (lattice, benchmark) = callable_zero_coupon_value(192);
        assert!(
            (lattice - benchmark).abs() < 0.08,
            "{lattice} vs {benchmark}"
        );
    }

    #[test]
    fn backward_fourier_zero_coupon_matches_hull_white_bond_option_formula() {
        let evaluation_date = Date::new(2025, 1, 1);
        let maturity_date = Date::new(2027, 1, 1);
        let (provider, curve) = fourier_provider(evaluation_date, 96);
        let strike = 0.95;
        let trade = bond(
            evaluation_date,
            maturity_date,
            0.0,
            PaymentStructure::Zero,
            vec![CallScheduleEntry::new(Date::new(2026, 1, 1), strike * FACE)],
        );
        let fourier = fourier_value(
            &fourier_pricer(BondPricingAnchor::OAS(0.0)),
            &trade,
            &provider,
        );
        let model = HullWhite::new(ALPHA, &curve).with_constant_volatility(SIGMA);
        let option = model
            .bond_call_price(1.0, 2.0, strike, SIGMA, &curve)
            .unwrap_or_else(|error| panic!("closed-form bond call: {error}"));
        let benchmark = FACE
            * (curve
                .discount_factor_from_time(2.0)
                .unwrap_or_else(|error| panic!("discount factor: {error}"))
                - option);
        assert!(
            (fourier - benchmark).abs() < 0.10,
            "{fourier} vs {benchmark}"
        );
    }

    #[test]
    fn call_windows_cover_interior_grid_points_and_active_settlement() -> Result<()> {
        let start = Date::new(2025, 1, 1);
        let maturity = Date::new(2027, 1, 1);
        let trade = bond(start, maturity, 0.0, PaymentStructure::Zero, vec![]);
        let cashflows = vec![ResolvedCashflow {
            payment_date: maturity,
            amount: FACE,
        }];
        let terms = lattice_terms(
            trade.bond_leg(),
            FACE,
            &[CallScheduleEntry::window(start + 90, start + 640, 95.0)],
            &cashflows,
            start,
            TimeGridConfiguration::new(2, 4, DayCounter::Actual365),
        )?;
        assert_eq!(terms.steps, 4);
        assert_eq!(
            terms
                .calls
                .iter()
                .map(|call| call.call_price)
                .collect::<Vec<_>>(),
            vec![None, Some(95.0), Some(95.0), Some(95.0), None]
        );
        let active = lattice_terms(
            trade.bond_leg(),
            FACE,
            &[CallScheduleEntry::window(start + 90, start + 640, 95.0)],
            &cashflows,
            start + 365,
            TimeGridConfiguration::new(4, 4, DayCounter::Actual365),
        )?;
        assert_eq!(active.calls[0].call_price, Some(95.0));
        let amortizing = MakeSteppedCouponBond::<DualFwd>::default()
            .with_identifier("USD-AMORTIZING-WINDOW".into())
            .with_start_date(start)
            .with_maturity_date(maturity)
            .with_notional(FACE)
            .with_currency(Currency::USD)
            .with_coupons(vec![CashflowType::fixed(0.0)])
            .with_notional_percentages(vec![1.0, 0.8])
            .build()?;
        let amortizing_terms = lattice_terms(
            amortizing.leg(),
            FACE,
            &[CallScheduleEntry::window(start + 90, start + 640, 95.0)],
            &cashflows,
            start,
            TimeGridConfiguration::new(2, 4, DayCounter::Actual365),
        )?;
        assert_eq!(amortizing_terms.calls[1].call_price, Some(76.0));
        assert_eq!(amortizing_terms.calls[2].call_price, Some(76.0));
        Ok(())
    }

    #[test]
    fn interior_window_exercise_reduces_price_in_both_engines() {
        let evaluation = Date::new(2025, 1, 1);
        let first = Date::new(2026, 1, 1);
        let last = Date::new(2028, 1, 1);
        let maturity = Date::new(2030, 1, 1);
        let endpoints = bond(
            evaluation,
            maturity,
            0.045,
            PaymentStructure::Bullet,
            vec![
                CallScheduleEntry::new(first, 100.0),
                CallScheduleEntry::new(last, 100.0),
            ],
        );
        let window = bond(
            evaluation,
            maturity,
            0.045,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::window(first, last, 100.0)],
        );
        let (tree_market, _) = provider(evaluation, 96);
        let (fourier_market, _) = fourier_provider(evaluation, 96);
        let tree = tree_pricer(BondPricingAnchor::OAS(0.0));
        let fourier = fourier_pricer(BondPricingAnchor::OAS(0.0));
        let tree_window = value(&tree, &window, &tree_market);
        let tree_endpoints = value(&tree, &endpoints, &tree_market);
        let fourier_window = fourier_value(&fourier, &window, &fourier_market);
        let fourier_endpoints = fourier_value(&fourier, &endpoints, &fourier_market);
        assert!(
            tree_window < tree_endpoints - 1.0e-4,
            "{tree_window} vs {tree_endpoints}"
        );
        assert!(
            fourier_window < fourier_endpoints - 1.0e-4,
            "{fourier_window} vs {fourier_endpoints}"
        );
        assert!(
            (tree_window - fourier_window).abs() < 0.15,
            "window tree {tree_window} vs Fourier {fourier_window}"
        );
    }

    #[test]
    fn tree_and_backward_fourier_prices_agree() {
        let evaluation_date = Date::new(2025, 1, 1);
        let trade = bond(
            evaluation_date,
            Date::new(2032, 1, 1),
            0.045,
            PaymentStructure::Bullet,
            vec![
                CallScheduleEntry::new(Date::new(2028, 1, 1), 101.0),
                CallScheduleEntry::new(Date::new(2030, 1, 1), 100.0),
            ],
        );
        let (tree_provider, _) = provider(evaluation_date, 96);
        let (fourier_provider, _) = fourier_provider(evaluation_date, 96);
        let tree = value(
            &tree_pricer(BondPricingAnchor::OAS(0.006)),
            &trade,
            &tree_provider,
        );
        let fourier = fourier_value(
            &fourier_pricer(BondPricingAnchor::OAS(0.006)),
            &trade,
            &fourier_provider,
        );
        assert!((tree - fourier).abs() < 0.12, "{tree} vs {fourier}");
    }

    #[test]
    fn engines_agree_for_positional_mixed_coupons_and_amortization() {
        let evaluation_date = Date::new(2025, 1, 1);
        let instrument = MakeSteppedCouponBond::<DualFwd>::default()
            .with_identifier("MIXED-STEP-BOND".into())
            .with_start_date(evaluation_date)
            .with_maturity_date(Date::new(2031, 1, 1))
            .with_units(FACE)
            .with_notional(FACE)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Semiannual)
            .with_coupons(vec![
                CashflowType::fixed(0.045),
                CashflowType::floating(MarketIndex::SOFR, 0.006, Some(0.0), Some(0.06)),
            ])
            .with_notional_percentages(vec![1.0, 0.9, 0.75])
            .with_call_schedule(vec![
                CallScheduleEntry::new(Date::new(2028, 1, 1), 101.0),
                CallScheduleEntry::new(Date::new(2029, 1, 1), 100.0),
            ])
            .build()
            .unwrap_or_else(|error| panic!("mixed stepped bond: {error}"));
        let trade = SteppedCouponBondTrade::new(
            instrument,
            evaluation_date,
            FACE,
            crate::core::trade::Side::LongReceive,
        );
        let (tree_provider, _) = provider(evaluation_date, 96);
        let (fourier_provider, _) = fourier_provider(evaluation_date, 96);
        let tree = value(
            &tree_pricer(BondPricingAnchor::OAS(0.0075)),
            &trade,
            &tree_provider,
        );
        let fourier = fourier_value(
            &fourier_pricer(BondPricingAnchor::OAS(0.0075)),
            &trade,
            &fourier_provider,
        );
        assert!(tree.is_finite() && fourier.is_finite());
        assert!((tree - fourier).abs() < 0.15, "{tree} vs {fourier}");
    }

    #[test]
    fn stepped_coupon_bond_rejects_yield_analytics() {
        let evaluation_date = Date::new(2025, 1, 1);
        let (provider, _) = provider(evaluation_date, 24);
        let trade = bond(
            evaluation_date,
            Date::new(2030, 1, 1),
            0.05,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(Date::new(2027, 1, 1), 100.0)],
        );
        let pricer = tree_pricer(BondPricingAnchor::OAS(0.0));
        for request in [Request::YieldToMaturity, Request::ModifiedDuration] {
            let error = match pricer.evaluate(&trade, &[request], &provider) {
                Ok(_) => panic!("stepped bond yield analytics must be rejected"),
                Err(error) => error,
            };
            assert!(error
                .to_string()
                .contains("not defined for SteppedCouponBond"));
        }
    }

    #[test]
    fn backward_fourier_oas_sensitivity_is_in_common_table() {
        let evaluation_date = Date::new(2025, 1, 1);
        let (provider, _) = fourier_provider(evaluation_date, 24);
        let trade = bond(
            evaluation_date,
            Date::new(2028, 1, 1),
            0.04,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(Date::new(2026, 7, 1), 100.0)],
        );
        let oas = 0.005;
        let results = fourier_pricer(BondPricingAnchor::OAS(oas))
            .evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)
            .unwrap_or_else(|error| panic!("Fourier sensitivities: {error}"));
        let oas_risk = results
            .sensitivities()
            .unwrap_or_else(|| panic!("Fourier sensitivity result"))
            .instrument_keys()
            .iter()
            .zip(
                results
                    .sensitivities()
                    .unwrap_or_else(|| panic!("Fourier sensitivity result"))
                    .exposure(),
            )
            .find_map(|(label, exposure)| (label == "TEST-BOND.OAS").then_some(*exposure))
            .unwrap_or_else(|| panic!("Fourier OAS sensitivity"));
        let bump = 1.0e-5;
        let finite_difference = (fourier_value(
            &fourier_pricer(BondPricingAnchor::OAS(oas + bump)),
            &trade,
            &provider,
        ) - fourier_value(
            &fourier_pricer(BondPricingAnchor::OAS(oas - bump)),
            &trade,
            &provider,
        )) / (2.0 * bump);
        assert!((oas_risk - finite_difference).abs() < 0.05);
    }

    #[test]
    fn lattice_ladder_converges_towards_closed_form() {
        let (coarse, benchmark) = callable_zero_coupon_value(12);
        let (fine, _) = callable_zero_coupon_value(192);
        assert!((fine - benchmark).abs() < (coarse - benchmark).abs());
        assert!((fine - benchmark).abs() < 0.15);
    }

    #[test]
    fn call_boundaries_and_extreme_oas_are_monotone() {
        let evaluation_date = Date::new(2025, 1, 1);
        let call_date = Date::new(2027, 1, 1);
        let maturity_date = Date::new(2030, 1, 1);
        let (provider, _) = provider(evaluation_date, 48);
        let low_call = bond(
            evaluation_date,
            maturity_date,
            0.05,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(call_date, 70.0)],
        );
        let high_call = bond(
            evaluation_date,
            maturity_date,
            0.05,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(call_date, 200.0)],
        );
        let low_call_value = value(
            &tree_pricer(BondPricingAnchor::OAS(0.0)),
            &low_call,
            &provider,
        );
        let high_call_value = value(
            &tree_pricer(BondPricingAnchor::OAS(0.0)),
            &high_call,
            &provider,
        );
        assert!(low_call_value < high_call_value);

        let negative = value(
            &tree_pricer(BondPricingAnchor::OAS(-0.05)),
            &high_call,
            &provider,
        );
        let zero = high_call_value;
        let positive = value(
            &tree_pricer(BondPricingAnchor::OAS(0.20)),
            &high_call,
            &provider,
        );
        assert!(negative.is_finite() && zero.is_finite() && positive.is_finite());
        assert!(negative > zero && zero > positive);
    }

    #[test]
    fn oas_price_anchor_round_trip_and_sensitivity_share_the_risk_table() {
        let evaluation_date = Date::new(2025, 1, 1);
        let (provider, _) = provider(evaluation_date, 24);
        let trade = bond(
            evaluation_date,
            Date::new(2030, 1, 1),
            0.05,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(Date::new(2027, 1, 1), 100.0)],
        );
        let oas = 0.0075;
        let direct = tree_pricer(BondPricingAnchor::OAS(oas));
        let results = direct
            .evaluate(
                &trade,
                &[Request::Value, Request::OAS, Request::Sensitivities],
                &provider,
            )
            .unwrap_or_else(|error| panic!("callable bond: {error}"));
        assert_eq!(results.oas(), Some(oas));
        let direct_value = results.price().unwrap_or_default();
        let implied = tree_pricer(BondPricingAnchor::DirtyPrice(direct_value));
        let implied_results = implied
            .evaluate(&trade, &[Request::OAS], &provider)
            .unwrap_or_else(|error| panic!("OAS solve: {error}"));
        assert!((implied_results.oas().unwrap_or_default() - oas).abs() < 1.0e-9);

        let risk = results
            .sensitivities()
            .unwrap_or_else(|| panic!("callable sensitivity result"));
        let oas_risk = risk
            .instrument_keys()
            .iter()
            .zip(risk.exposure())
            .find_map(|(label, exposure)| (label == "TEST-BOND.OAS").then_some(*exposure))
            .unwrap_or_else(|| panic!("OAS sensitivity"));
        let bump = 1.0e-5;
        let finite_difference = (value(
            &tree_pricer(BondPricingAnchor::OAS(oas + bump)),
            &trade,
            &provider,
        ) - value(
            &tree_pricer(BondPricingAnchor::OAS(oas - bump)),
            &trade,
            &provider,
        )) / (2.0 * bump);
        assert!((oas_risk - finite_difference).abs() < 0.02);
    }

    #[test]
    fn rejects_mismatched_spread_family_and_bad_call_terms() {
        let evaluation_date = Date::new(2025, 1, 1);
        let (provider, _) = provider(evaluation_date, 24);
        let straight = bond(
            evaluation_date,
            Date::new(2028, 1, 1),
            0.04,
            PaymentStructure::Bullet,
            Vec::new(),
        );
        assert!(tree_pricer(BondPricingAnchor::OAS(0.0))
            .evaluate(&straight, &[Request::Value], &provider)
            .is_err());
        assert!(CallScheduleEntry::new(Date::new(2026, 1, 1), 0.0)
            .validate()
            .is_err());
    }
}

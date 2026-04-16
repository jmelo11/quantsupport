use crate::{
    calibration::{
        calibrationpricer::CalibrationInstrumentPricer, calibrationprocess::CalibrationProcess,
    },
    core::collateral::Discountable,
    instruments::cashflows::{
        cashflow::Cashflow, cashflowtype::CashflowType, coupons::LinearCoupon, leg::Leg,
    },
    quotes::{calibrationinstrument::CalibrationInstrument, quote::CalibrationInstrumentType},
    rates::bootstrapping::{bootstrappedcurve::BootstrappedCurve, bootstrapstep::BootstrapStep},
    utils::errors::{QSError, Result},
};

/// Evaluator that implements [`CalibrationInstrumentPricer`] for a single
/// bootstrap step.
pub struct BootstrapStepEvaluation<'a> {
    step: &'a BootstrapStep<'a>,
}

impl<'a> BootstrapStepEvaluation<'a> {
    /// Creates a new evaluator from a bootstrap step.
    #[must_use]
    pub const fn new(step: &'a BootstrapStep) -> Self {
        Self { step }
    }

    #[allow(clippy::unused_self)]
    fn leg_pv(
        &self,
        leg: &Leg<f64>,
        discount_curve: &BootstrappedCurve,
        forward_curve: Option<&BootstrappedCurve>,
    ) -> Result<f64> {
        let side = leg.side().sign();
        let mut pv = 0.0;
        for cashflow in leg.cashflows() {
            match cashflow {
                CashflowType::Disbursement(disbursement) => {
                    let payment_date = disbursement.payment_date();
                    let df = discount_curve.discount_factor(payment_date)?;
                    pv += -side * disbursement.amount()? * df;
                }
                CashflowType::Redemption(redemption) => {
                    let payment_date = redemption.payment_date();
                    let df = discount_curve.discount_factor(payment_date)?;
                    pv += side * redemption.amount()? * df;
                }
                CashflowType::FixedRateCoupon(fixed_coupon) => {
                    let payment_date = fixed_coupon.payment_date();
                    let df = discount_curve.discount_factor(payment_date)?;
                    pv += side * fixed_coupon.amount()? * df;
                }
                CashflowType::FloatingRateCoupon(floating_coupon) => {
                    let payment_date = floating_coupon.payment_date();
                    let df = discount_curve.discount_factor(payment_date)?;
                    let rate_definition = leg
                        .forward_index()
                        .ok_or_else(|| {
                            QSError::InvalidValueErr(
                            "Floating leg market index is required for forward rate calculation"
                                .into(),
                        )
                        })?
                        .rate_index_details()?
                        .rate_definition();
                    let fixing = forward_curve
                        .ok_or_else(|| QSError::ValueNotSetErr("Missing forward curve".into()))?
                        .forward_rate(
                            floating_coupon.accrual_start_date(),
                            floating_coupon.accrual_end_date(),
                            rate_definition,
                        )?;

                    floating_coupon.set_fixing(fixing);
                    pv += side * floating_coupon.amount()? * df;
                }
                _ => {
                    return Err(QSError::InvalidValueErr(
                        "Unsupported cashflow type for PV calculation".into(),
                    ))
                }
            }
        }
        Ok(pv)
    }
}

impl CalibrationInstrumentPricer for BootstrapStepEvaluation<'_> {
    #[allow(clippy::too_many_lines)]
    fn price(&self, instrument: &CalibrationInstrument) -> Result<f64> {
        match instrument.built() {
            CalibrationInstrumentType::FixedRateDeposit(deposit) => {
                // The deposit rate is the quote; extract start/end dates
                // from the single coupon and compare the curve-implied rate
                // to the market rate.
                let idx = deposit
                    .discount_index()
                    .ok_or_else(|| QSError::NotFoundErr("Deposit has no market index".into()))?;
                let curve = self.step.get(&idx).ok_or_else(|| {
                    QSError::NotFoundErr(format!("Missing curve {idx} for deposit"))
                })?;
                let start = deposit.start_date();
                let end = deposit.maturity_date();
                let rd = deposit
                    .rate()
                    .ok_or_else(|| QSError::ValueNotSetErr("Deposit rate not set".into()))?
                    .rate_definition();
                let implied = curve.forward_rate(start, end, rd)?;
                Ok(implied)
            }
            CalibrationInstrumentType::Swap(swap) => {
                let pv_fixed = {
                    let disc = self.step.discount_curve_for_leg(swap.fixed_leg())?;
                    let fwd = self.step.forward_curve_for_leg(swap.fixed_leg())?;
                    self.leg_pv(swap.fixed_leg(), disc, fwd)?
                };
                let pv_float = {
                    let disc = self.step.discount_curve_for_leg(swap.floating_leg())?;
                    let fwd = self.step.forward_curve_for_leg(swap.floating_leg())?;
                    self.leg_pv(swap.floating_leg(), disc, fwd)?
                };
                Ok(pv_fixed + pv_float)
            }
            CalibrationInstrumentType::BasisSwap(basis_swap) => {
                let pv_pay = {
                    let disc = self.step.discount_curve_for_leg(basis_swap.pay_leg())?;
                    let fwd = self.step.forward_curve_for_leg(basis_swap.pay_leg())?;
                    self.leg_pv(basis_swap.pay_leg(), disc, fwd)?
                };
                let pv_recv = {
                    let disc = self.step.discount_curve_for_leg(basis_swap.receive_leg())?;
                    let fwd = self.step.forward_curve_for_leg(basis_swap.receive_leg())?;
                    self.leg_pv(basis_swap.receive_leg(), disc, fwd)?
                };
                Ok(pv_pay + pv_recv)
            }
            CalibrationInstrumentType::FixFloatCrossCurrencySwap(xccy) => {
                let dom_disc = self.step.discount_curve_for_leg(xccy.domestic_leg())?;
                let dom_fwd = self.step.forward_curve_for_leg(xccy.domestic_leg())?;
                let for_disc = self.step.discount_curve_for_leg(xccy.foreign_leg())?;
                let for_fwd = self.step.forward_curve_for_leg(xccy.foreign_leg())?;
                let fx = self
                    .step
                    .fx_spot(xccy.domestic_currency(), xccy.foreign_currency())?;

                let dom_pv = self.leg_pv(xccy.domestic_leg(), dom_disc, dom_fwd)?;
                let for_pv = self.leg_pv(xccy.foreign_leg(), for_disc, for_fwd)?;
                Ok(dom_pv + for_pv / fx)
            }
            CalibrationInstrumentType::FloatFloatCrossCurrencySwap(xccy) => {
                let dom_disc = self.step.discount_curve_for_leg(xccy.domestic_leg())?;
                let dom_fwd = self.step.forward_curve_for_leg(xccy.domestic_leg())?;
                let for_disc = self.step.discount_curve_for_leg(xccy.foreign_leg())?;
                let for_fwd = self.step.forward_curve_for_leg(xccy.foreign_leg())?;
                let fx = self
                    .step
                    .fx_spot(xccy.domestic_currency(), xccy.foreign_currency())?;

                let dom_pv = self.leg_pv(xccy.domestic_leg(), dom_disc, dom_fwd)?;
                let for_pv = self.leg_pv(xccy.foreign_leg(), for_disc, for_fwd)?;
                Ok(dom_pv + for_pv / fx)
            }
            CalibrationInstrumentType::RateFutures(rf) => {
                let curve = self.step.get(&rf.market_index()).ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Missing curve {} for rate futures",
                        rf.market_index()
                    ))
                })?;
                let implied =
                    curve.forward_rate(rf.start_date(), rf.end_date(), rf.rate_definition())?;
                Ok(implied)
            }
            CalibrationInstrumentType::FxForward(fxf) => {
                let base_ccy = fxf.base_currency();
                let quote_ccy = fxf.quote_currency();
                let spot = self.step.fx_spot(base_ccy, quote_ccy)?;
                let delivery = fxf.delivery_date();

                let policy = self.step.discount_policy();
                let base_index = policy.discount_index_for_currency(base_ccy)?;
                let quote_index = policy.discount_index_for_currency(quote_ccy)?;

                let base_curve = self.step.get(&base_index).ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Missing discount curve {base_index} for FX forward base currency"
                    ))
                })?;
                let quote_curve = self.step.get(&quote_index).ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Missing discount curve {quote_index} for FX forward quote currency"
                    ))
                })?;

                let df_base = base_curve.discount_factor(delivery)?;
                let df_quote = quote_curve.discount_factor(delivery)?;
                let implied_fwd = spot * df_base / df_quote;

                // Handle both outright forward prices and forward points.
                match fxf.forward_price() {
                    Some(_) => Ok(implied_fwd),
                    None => match fxf.forward_points() {
                        Some(_) => Ok(implied_fwd - spot),
                        None => Err(QSError::ValueNotSetErr(
                            "FX forward: neither price nor points set".into(),
                        )),
                    },
                }
            }
            _ => Err(QSError::InvalidValueErr(format!(
                "Calibration Instrumet of type {:?} is not supported for curve bootstrapping.",
                instrument.built()
            ))),
        }
    }

    fn sensitivity(&self, instrument: &CalibrationInstrument) -> Result<f64> {
        match instrument.built() {
            CalibrationInstrumentType::FixedRateDeposit(_)
            | CalibrationInstrumentType::FxForward(_) => Ok(-1.0),
            CalibrationInstrumentType::Swap(swap) => fixed_leg_annuity(swap.fixed_leg(), self.step),
            CalibrationInstrumentType::BasisSwap(bs) => {
                floating_leg_annuity(bs.pay_leg(), self.step)
            }
            CalibrationInstrumentType::FixFloatCrossCurrencySwap(xccy) => {
                fixed_leg_annuity(xccy.domestic_leg(), self.step)
            }
            CalibrationInstrumentType::FloatFloatCrossCurrencySwap(xccy) => {
                floating_leg_annuity(xccy.domestic_leg(), self.step)
            }
            CalibrationInstrumentType::RateFutures(_) => Ok(1.0 / 100.0),
            _ => Err(QSError::InvalidValueErr(
                "Unsupported instrument type for quote sensitivity".into(),
            )),
        }
    }
}

impl CalibrationProcess for BootstrapStepEvaluation<'_> {}

/// Computes the fixed-leg annuity
fn fixed_leg_annuity(leg: &Leg<f64>, curves: &BootstrapStep) -> Result<f64> {
    let disc = curves.discount_curve_for_leg(leg)?;
    let side = leg.side().sign();
    let mut annuity = 0.0;
    for cashflow in leg.cashflows() {
        if let CashflowType::FixedRateCoupon(coupon) = cashflow {
            let df = disc.discount_factor(coupon.payment_date())?;
            let yf = coupon
                .rate()
                .day_counter()
                .year_fraction(coupon.accrual_start_date(), coupon.accrual_end_date());
            annuity += side * yf * coupon.notional() * df;
        }
    }
    Ok(annuity)
}

/// Computes the floating-leg annuity
fn floating_leg_annuity(leg: &Leg<f64>, curves: &BootstrapStep) -> Result<f64> {
    let disc = curves.discount_curve_for_leg(leg)?;
    let side = leg.side().sign();
    let mut annuity = 0.0;
    for cashflow in leg.cashflows() {
        if let CashflowType::FloatingRateCoupon(coupon) = cashflow {
            let df = disc.discount_factor(coupon.payment_date())?;
            let yf = coupon
                .day_counter()
                .year_fraction(coupon.accrual_start_date(), coupon.accrual_end_date());
            annuity += side * yf * coupon.notional() * df;
        }
    }
    Ok(annuity)
}

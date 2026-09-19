use crate::{
    calibration::{
        calibrationpricer::CalibrationInstrumentPricer, calibrationprocess::CalibrationProcess,
    },
    core::collateral::Discountable,
    indices::marketindex::MarketIndex,
    instruments::cashflows::{
        cashflow::Cashflow, cashflowtype::CashflowType, coupons::LinearCoupon, leg::Leg,
    },
    math::interpolation::interpolator::Interpolator,
    quotes::{
        calibrationinstrument::CalibrationInstrument,
        quote::{BondCalibrationStrategy, CalibrationInstrumentType, FxForwardCalibrationStrategy},
    },
    rates::{
        bootstrapping::{bootstrappedcurve::BootstrappedCurve, bootstrapstep::BootstrapStep},
        interestrate::InterestRate,
    },
    time::date::Date,
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
                    pv = (-side * disbursement.amount()?).mul_add(df, pv);
                }
                CashflowType::Redemption(redemption) => {
                    let payment_date = redemption.payment_date();
                    let df = discount_curve.discount_factor(payment_date)?;
                    pv = (side * redemption.amount()?).mul_add(df, pv);
                }
                CashflowType::FixedRateCoupon(fixed_coupon) => {
                    let payment_date = fixed_coupon.payment_date();
                    let df = discount_curve.discount_factor(payment_date)?;
                    pv = (side * fixed_coupon.amount()?).mul_add(df, pv);
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
                    pv = (side * floating_coupon.amount()?).mul_add(df, pv);
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

    /// Converts the existing fixed-leg PV into a quoted price (normally 100
    /// per unit of principal) by adding back the initial disbursement.
    fn fixed_leg_price(
        &self,
        leg: &Leg<f64>,
        units: f64,
        curve: &BootstrappedCurve,
    ) -> Result<f64> {
        let pv = self.leg_pv(leg, curve, None)?;
        let side = leg.side().sign();
        let mut principal = 0.0;
        let mut disbursement_pv = 0.0;
        for cashflow in leg.cashflows() {
            if let CashflowType::Disbursement(disbursement) = cashflow {
                let amount = disbursement.amount()?;
                principal += amount;
                disbursement_pv = amount.mul_add(
                    curve.discount_factor(disbursement.payment_date())?,
                    disbursement_pv,
                );
            }
        }
        if principal.abs() < f64::EPSILON {
            return Err(QSError::InvalidValueErr(
                "Cannot normalize fixed-leg price without an initial disbursement".into(),
            ));
        }
        Ok((pv / side + disbursement_pv) * units / principal)
    }

    fn settlement_date(leg: &Leg<f64>) -> Result<Date> {
        leg.cashflows()
            .iter()
            .filter_map(|cashflow| match cashflow {
                CashflowType::Disbursement(disbursement) => Some(disbursement.payment_date()),
                _ => None,
            })
            .min()
            .ok_or_else(|| {
                QSError::ValueNotSetErr("Initial disbursement date for yield calibration".into())
            })
    }

    fn yield_price(&self, leg: &Leg<f64>, units: f64, yield_rate: f64) -> Result<f64> {
        let settlement_date = Self::settlement_date(leg)?;
        let rate_definition = leg
            .interest_rate()
            .ok_or_else(|| QSError::ValueNotSetErr("Fixed-leg rate definition".into()))?
            .rate_definition();
        let yield_definition = InterestRate::from_rate_definition(yield_rate, rate_definition);
        let day_counter = rate_definition.day_counter();
        let mut dates = Vec::with_capacity(leg.cashflows().len());
        for cashflow in leg.cashflows() {
            let date = match cashflow {
                CashflowType::FixedRateCoupon(coupon) => coupon.payment_date(),
                CashflowType::Redemption(redemption) => redemption.payment_date(),
                CashflowType::Disbursement(disbursement) => disbursement.payment_date(),
                _ => {
                    return Err(QSError::InvalidValueErr(
                        "Yield calibration requires a fixed-rate leg".into(),
                    ))
                }
            };
            dates.push(date);
        }
        dates.sort_unstable();
        dates.dedup();

        let mut times = vec![0.0];
        let mut discount_factors = vec![1.0];
        for date in dates.into_iter().filter(|date| *date > settlement_date) {
            times.push(day_counter.year_fraction(settlement_date, date));
            discount_factors.push(yield_definition.discount_factor(settlement_date, date));
        }
        let yield_curve = BootstrappedCurve::new(
            MarketIndex::Other("fixed-rate-bond-yield".into()),
            settlement_date,
            times,
            discount_factors,
            day_counter,
            Interpolator::LogLinear,
        );
        self.fixed_leg_price(leg, units, &yield_curve)
    }
}

impl CalibrationInstrumentPricer for BootstrapStepEvaluation<'_> {
    #[allow(clippy::too_many_lines)]
    fn price(&self, instrument: &CalibrationInstrument) -> Result<f64> {
        match instrument.built() {
            CalibrationInstrumentType::FixedRateDeposit(deposit) => {
                let idx = deposit
                    .discount_index()
                    .ok_or_else(|| QSError::NotFoundErr("Deposit has no market index".into()))?;
                let curve = self.step.get(&idx).ok_or_else(|| {
                    QSError::NotFoundErr(format!("Missing curve {idx} for deposit"))
                })?;
                let rd = deposit
                    .rate()
                    .ok_or_else(|| QSError::ValueNotSetErr("Deposit rate not set".into()))?
                    .rate_definition();
                let implied =
                    curve.forward_rate(deposit.start_date(), deposit.maturity_date(), rd)?;
                Ok(implied - instrument.quote_value())
            }
            CalibrationInstrumentType::FixedRateBond(bond, strategy) => {
                let idx = bond.discount_index().ok_or_else(|| {
                    QSError::NotFoundErr("Fixed-rate bond has no market index".into())
                })?;
                let curve = self.step.get(&idx).ok_or_else(|| {
                    QSError::NotFoundErr(format!("Missing curve {idx} for fixed-rate bond"))
                })?;
                let model_price = self.fixed_leg_price(bond.leg(), bond.units(), curve)?;
                match strategy {
                    BondCalibrationStrategy::AnchorPrice => {
                        Ok(model_price - instrument.quote_value())
                    }
                    BondCalibrationStrategy::AnchorYield => Ok(model_price
                        - self.yield_price(bond.leg(), bond.units(), instrument.quote_value())?),
                }
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
                Ok(implied - rf.implied_rate())
            }
            CalibrationInstrumentType::FxForward(fxf, strategy) => {
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

                match strategy {
                    FxForwardCalibrationStrategy::AnchorOutrightPrice => {
                        Ok(implied_fwd - instrument.quote_value())
                    }
                    FxForwardCalibrationStrategy::AnchorForwardPoints => {
                        Ok(implied_fwd - spot - instrument.quote_value())
                    }
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
            | CalibrationInstrumentType::FxForward(_, _) => Ok(-1.0),
            CalibrationInstrumentType::FixedRateBond(bond, strategy) => match strategy {
                BondCalibrationStrategy::AnchorPrice => Ok(-1.0),
                BondCalibrationStrategy::AnchorYield => {
                    let bump = 1e-6;
                    let market_yield = instrument.quote_value();
                    let up = self.yield_price(bond.leg(), bond.units(), market_yield + bump)?;
                    let down = self.yield_price(bond.leg(), bond.units(), market_yield - bump)?;
                    Ok(-(up - down) / (2.0 * bump))
                }
            },
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
            annuity = (side * yf * coupon.notional()).mul_add(df, annuity);
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
            annuity = (side * yf * coupon.notional()).mul_add(df, annuity);
        }
    }
    Ok(annuity)
}

use crate::{
    core::collateral::Discountable,
    instruments::cashflows::{
        cashflow::Cashflow, cashflowtype::CashflowType, coupons::LinearCoupon, leg::Leg,
    },
    quotes::quote::{CalibrationInstrumentType, Level, Quote},
    rates::bootstrapping::bootstraputils::{BootstrapCurveSet, SolvedCurve},
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// A calibration instrument: a quote that has been turned into a
/// concrete [`CalibrationInstrumentType`] with a known pillar date and scalar quote
/// value.
#[derive(Clone)]
pub struct CalibrationInstrument {
    quote: Quote,
    level: Level,
    built: CalibrationInstrumentType,
    quote_value: f64,
    pillar_date: Date,
}

impl CalibrationInstrument {
    /// Creates a calibration instrument.
    #[must_use]
    pub const fn new(
        quote: Quote,
        level: Level,
        built: CalibrationInstrumentType,
        quote_value: f64,
        pillar_date: Date,
    ) -> Self {
        Self {
            quote,
            level,
            built,
            quote_value,
            pillar_date,
        }
    }

    /// Returns the source quote.
    #[must_use]
    pub const fn quote(&self) -> &Quote {
        &self.quote
    }

    /// Returns the quote level.
    #[must_use]
    pub const fn level(&self) -> Level {
        self.level
    }

    /// Returns the built instrument.
    #[must_use]
    pub const fn built(&self) -> &CalibrationInstrumentType {
        &self.built
    }

    /// Returns the market input value.
    #[must_use]
    pub const fn quote_value(&self) -> f64 {
        self.quote_value
    }

    /// Returns the pillar date.
    #[must_use]
    pub const fn pillar_date(&self) -> Date {
        self.pillar_date
    }

    /// Returns the reporting label associated with this calibration input.
    #[must_use]
    pub fn pillar_label(&self) -> String {
        self.quote.details().identifier()
    }
}

impl CalibrationInstrument {
    /// Returns the market index associated with this instrument, if any.
    /// Computes the pricing residual using the given set of curves.
    ///
    /// For leg-based instruments (deposits, swaps, basis swaps, cross-currency
    /// swaps), the discount and forward curves are resolved per-leg via the
    /// discount policy embedded in the curve set.  For rate futures and FX
    /// forwards, specialised residual formulas are used.
    ///
    /// # Errors
    /// Returns an error if the instrument type is unsupported for residual
    /// calculation or if required market data (e.g., forward rates for floating coupons,
    /// discount factors for cashflow dates) is missing from
    #[allow(clippy::too_many_lines)]
    pub fn residual(&self, curves: &BootstrapCurveSet) -> Result<f64> {
        match self.built() {
            CalibrationInstrumentType::FixedRateDeposit(deposit) => {
                // The deposit rate is the quote; extract start/end dates
                // from the single coupon and compare the curve-implied rate
                // to the market rate.
                let idx = deposit
                    .discount_index()
                    .ok_or_else(|| QSError::NotFoundErr("Deposit has no market index".into()))?;
                let curve = curves.get(&idx).ok_or_else(|| {
                    QSError::NotFoundErr(format!("Missing curve {idx} for deposit"))
                })?;
                let start = deposit.start_date();
                let end = deposit.maturity_date();
                let rd = deposit
                    .rate()
                    .ok_or_else(|| QSError::ValueNotSetErr("Deposit rate not set".into()))?
                    .rate_definition();
                let implied = curve.forward_rate(start, end, rd)?;
                Ok(implied - self.quote_value)
            }
            CalibrationInstrumentType::Swap(swap) => {
                let pv_fixed = {
                    let disc = curves.discount_curve_for_leg(swap.fixed_leg())?;
                    let fwd = curves.forward_curve_for_leg(swap.fixed_leg())?;
                    self.leg_pv(swap.fixed_leg(), disc, fwd)?
                };
                let pv_float = {
                    let disc = curves.discount_curve_for_leg(swap.floating_leg())?;
                    let fwd = curves.forward_curve_for_leg(swap.floating_leg())?;
                    self.leg_pv(swap.floating_leg(), disc, fwd)?
                };
                Ok(pv_fixed + pv_float)
            }
            CalibrationInstrumentType::BasisSwap(basis_swap) => {
                let pv_pay = {
                    let disc = curves.discount_curve_for_leg(basis_swap.pay_leg())?;
                    let fwd = curves.forward_curve_for_leg(basis_swap.pay_leg())?;
                    self.leg_pv(basis_swap.pay_leg(), disc, fwd)?
                };
                let pv_recv = {
                    let disc = curves.discount_curve_for_leg(basis_swap.receive_leg())?;
                    let fwd = curves.forward_curve_for_leg(basis_swap.receive_leg())?;
                    self.leg_pv(basis_swap.receive_leg(), disc, fwd)?
                };
                Ok(pv_pay + pv_recv)
            }
            CalibrationInstrumentType::FixFloatCrossCurrencySwap(xccy) => {
                let dom_disc = curves.discount_curve_for_leg(xccy.domestic_leg())?;
                let dom_fwd = curves.forward_curve_for_leg(xccy.domestic_leg())?;
                let for_disc = curves.discount_curve_for_leg(xccy.foreign_leg())?;
                let for_fwd = curves.forward_curve_for_leg(xccy.foreign_leg())?;
                let fx = curves.fx_spot(xccy.domestic_currency(), xccy.foreign_currency())?;

                let dom_pv = self.leg_pv(xccy.domestic_leg(), dom_disc, dom_fwd)?;
                let for_pv = self.leg_pv(xccy.foreign_leg(), for_disc, for_fwd)?;
                Ok(dom_pv + for_pv / fx)
            }
            CalibrationInstrumentType::FloatFloatCrossCurrencySwap(xccy) => {
                let dom_disc = curves.discount_curve_for_leg(xccy.domestic_leg())?;
                let dom_fwd = curves.forward_curve_for_leg(xccy.domestic_leg())?;
                let for_disc = curves.discount_curve_for_leg(xccy.foreign_leg())?;
                let for_fwd = curves.forward_curve_for_leg(xccy.foreign_leg())?;
                let fx = curves.fx_spot(xccy.domestic_currency(), xccy.foreign_currency())?;

                let dom_pv = self.leg_pv(xccy.domestic_leg(), dom_disc, dom_fwd)?;
                let for_pv = self.leg_pv(xccy.foreign_leg(), for_disc, for_fwd)?;
                Ok(dom_pv + for_pv / fx)
            }
            CalibrationInstrumentType::RateFutures(rf) => {
                let curve = curves.get(&rf.market_index()).ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Missing curve {} for rate futures",
                        rf.market_index()
                    ))
                })?;
                let implied =
                    curve.forward_rate(rf.start_date(), rf.end_date(), rf.rate_definition())?;
                Ok(implied - rf.implied_rate())
            }

            // an fx forward should have two legs, one per currency.
            CalibrationInstrumentType::FxForward(fxf) => {
                let base_ccy = fxf.base_currency();
                let quote_ccy = fxf.quote_currency();
                let spot = curves.fx_spot(base_ccy, quote_ccy)?;
                let delivery = fxf.delivery_date();

                let policy = curves.discount_policy();
                let base_index = policy.discount_index_for_currency(base_ccy)?;
                let quote_index = policy.discount_index_for_currency(quote_ccy)?;

                let base_curve = curves.get(&base_index).ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Missing discount curve {base_index} for FX forward base currency"
                    ))
                })?;
                let quote_curve = curves.get(&quote_index).ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Missing discount curve {quote_index} for FX forward quote currency"
                    ))
                })?;

                let df_base = base_curve.discount_factor(delivery)?;
                let df_quote = quote_curve.discount_factor(delivery)?;
                let implied_fwd = spot * df_base / df_quote;

                // Handle both outright forward prices and forward points.
                let market_fwd = if let Some(price) = fxf.forward_price() {
                    price
                } else if let Some(points) = fxf.forward_points() {
                    spot + points
                } else {
                    return Err(QSError::ValueNotSetErr(
                        "FX forward: neither price nor points set".into(),
                    ));
                };
                Ok(implied_fwd - market_fwd)
            }
            _ => Err(QSError::InvalidValueErr(
                "Unsupported instrument type for bootstrap residual".into(),
            )),
        }
    }

    /// Returns the analytical `∂F/∂q` for this instrument.
    ///
    /// Because each quote enters its own residual linearly, the derivative
    /// is a closed-form scalar that depends only on the instrument type:
    ///
    /// | Type              | `∂F/∂q`                            |
    /// |-------------------|----------------------------------|
    /// | `Deposit`           | −1                               |
    /// | `Swap`              | fixed-leg annuity                |
    /// | `BasisSwap`         | pay-leg (spread-leg) annuity     |
    /// | `FixFloatXCcy`      | domestic fixed-leg annuity       |
    /// | `FloatFloatXCcy`    | domestic floating-leg annuity    |
    /// | `RateFutures`       | 1/100                            |
    /// | `FxForward`         | −1                               |
    ///
    /// # Errors
    /// Returns an error if the instrument type is unsupported for quote sensitivity
    /// calculation or if required market data (e.g., discount factors for cashflow dates) is
    /// missing from the provided curves for leg-based instruments.
    pub fn quote_sensitivity(&self, curves: &BootstrapCurveSet) -> Result<f64> {
        match self.built() {
            CalibrationInstrumentType::FixedRateDeposit(_)
            | CalibrationInstrumentType::FxForward(_) => Ok(-1.0),
            CalibrationInstrumentType::Swap(swap) => fixed_leg_annuity(swap.fixed_leg(), curves),
            CalibrationInstrumentType::BasisSwap(bs) => floating_leg_annuity(bs.pay_leg(), curves),
            CalibrationInstrumentType::FixFloatCrossCurrencySwap(xccy) => {
                fixed_leg_annuity(xccy.domestic_leg(), curves)
            }
            CalibrationInstrumentType::FloatFloatCrossCurrencySwap(xccy) => {
                floating_leg_annuity(xccy.domestic_leg(), curves)
            }
            CalibrationInstrumentType::RateFutures(_) => Ok(1.0 / 100.0),
            _ => Err(QSError::InvalidValueErr(
                "Unsupported instrument type for quote sensitivity".into(),
            )),
        }
    }

    /// Computes the present value of a single leg using the given discount and forward curves.
    ///
    /// # Errors
    /// Returns an error if any cashflow in the leg is of an unsupported type or if required market data (e.g., forward rates for floating coupons) is missing from the provided curves.
    pub fn leg_pv(
        &self,
        leg: &Leg<f64>,
        discount_curve: &SolvedCurve,
        forward_curve: Option<&SolvedCurve>,
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
                CashflowType::OptionEmbeddedCoupon(_) => {
                    return Err(QSError::InvalidValueErr(
                        "Unsupported cashflow type for PV calculation".into(),
                    ))
                }
            }
        }
        Ok(pv)
    }
}

/// Computes the fixed-leg annuity
fn fixed_leg_annuity(leg: &Leg<f64>, curves: &BootstrapCurveSet) -> Result<f64> {
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
fn floating_leg_annuity(leg: &Leg<f64>, curves: &BootstrapCurveSet) -> Result<f64> {
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

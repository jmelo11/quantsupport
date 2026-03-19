use crate::{
    core::collateral::Discountable,
    instruments::cashflows::{
        cashflow::Cashflow, cashflowtype::CashflowType, coupons::LinearCoupon, leg::Leg,
    },
    quotes::quote::{BuiltInstrument, Level, Quote},
    rates::bootstrapping::bootstraputils::{BootstrapCurveSet, SolvedCurve},
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// A resolved calibration instrument: a quote that has been turned into a
/// concrete [`BuiltInstrument`] with a known pillar date and scalar quote
/// value.
#[derive(Clone)]
pub struct ResolvedInstrument {
    quote: Quote,
    level: Level,
    built: BuiltInstrument,
    quote_value: f64,
    pillar_date: Date,
}

impl ResolvedInstrument {
    /// Creates a resolved calibration instrument.
    #[must_use]
    pub const fn new(
        quote: Quote,
        level: Level,
        built: BuiltInstrument,
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
    pub const fn built(&self) -> &BuiltInstrument {
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

impl ResolvedInstrument {
    /// Returns the market index associated with this instrument, if any.
    /// Computes the pricing residual using the given set of curves.
    ///
    /// For leg-based instruments (deposits, swaps, basis swaps, cross-currency
    /// swaps), the discount and forward curves are resolved per-leg via the
    /// discount policy embedded in the curve set.  For rate futures and FX
    /// forwards, specialised residual formulas are used.
    pub fn residual(&self, curves: &BootstrapCurveSet) -> Result<f64> {
        match self.built() {
            BuiltInstrument::FixedRateDeposit(deposit) => {
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
            BuiltInstrument::Swap(swap) => {
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
            BuiltInstrument::BasisSwap(basis_swap) => {
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
            BuiltInstrument::FixFloatCrossCurrencySwap(xccy) => {
                let dom_disc = curves.discount_curve_for_leg(xccy.domestic_leg())?;
                let dom_fwd = curves.forward_curve_for_leg(xccy.domestic_leg())?;
                let for_disc = curves.discount_curve_for_leg(xccy.foreign_leg())?;
                let for_fwd = curves.forward_curve_for_leg(xccy.foreign_leg())?;
                let fx = curves.fx_spot(xccy.domestic_currency(), xccy.foreign_currency())?;

                let dom_pv = self.leg_pv(xccy.domestic_leg(), dom_disc, dom_fwd)?;
                let for_pv = self.leg_pv(xccy.foreign_leg(), for_disc, for_fwd)?;
                Ok(dom_pv + for_pv / fx)
            }
            BuiltInstrument::FloatFloatCrossCurrencySwap(xccy) => {
                let dom_disc = curves.discount_curve_for_leg(xccy.domestic_leg())?;
                let dom_fwd = curves.forward_curve_for_leg(xccy.domestic_leg())?;
                let for_disc = curves.discount_curve_for_leg(xccy.foreign_leg())?;
                let for_fwd = curves.forward_curve_for_leg(xccy.foreign_leg())?;
                let fx = curves.fx_spot(xccy.domestic_currency(), xccy.foreign_currency())?;

                let dom_pv = self.leg_pv(xccy.domestic_leg(), dom_disc, dom_fwd)?;
                let for_pv = self.leg_pv(xccy.foreign_leg(), for_disc, for_fwd)?;
                Ok(dom_pv + for_pv / fx)
            }
            BuiltInstrument::RateFutures(rf) => {
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
            BuiltInstrument::FxForward(fxf) => {
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

    /// Returns the analytical ∂F/∂q for this instrument.
    ///
    /// Because each quote enters its own residual linearly, the derivative
    /// is a closed-form scalar that depends only on the instrument type:
    ///
    /// | Type              | ∂F/∂q                            |
    /// |-------------------|----------------------------------|
    /// | Deposit           | −1                               |
    /// | Swap              | fixed-leg annuity                |
    /// | BasisSwap         | pay-leg (spread-leg) annuity     |
    /// | FixFloatXCcy      | domestic fixed-leg annuity       |
    /// | FloatFloatXCcy    | domestic floating-leg annuity    |
    /// | RateFutures       | 1/100                            |
    /// | FxForward         | −1                               |
    pub fn quote_sensitivity(&self, curves: &BootstrapCurveSet) -> Result<f64> {
        match self.built() {
            BuiltInstrument::FixedRateDeposit(_) => Ok(-1.0),
            BuiltInstrument::Swap(swap) => self.fixed_leg_annuity(swap.fixed_leg(), curves),
            BuiltInstrument::BasisSwap(bs) => self.floating_leg_annuity(bs.pay_leg(), curves),
            BuiltInstrument::FixFloatCrossCurrencySwap(xccy) => {
                self.fixed_leg_annuity(xccy.domestic_leg(), curves)
            }
            BuiltInstrument::FloatFloatCrossCurrencySwap(xccy) => {
                self.floating_leg_annuity(xccy.domestic_leg(), curves)
            }
            BuiltInstrument::RateFutures(_) => Ok(1.0 / 100.0),
            BuiltInstrument::FxForward(_) => Ok(-1.0),
            _ => Err(QSError::InvalidValueErr(
                "Unsupported instrument type for quote sensitivity".into(),
            )),
        }
    }

    /// Computes the present value of a single leg using the given discount and forward curves.
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
                    pv += side * disbursement.amount()? * df;
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

    /// Computes the fixed-leg annuity: Σ (side × τ × notional × DF).
    /// This gives ∂NPV_fixed/∂rate and is used for IFT quote sensitivities.
    fn fixed_leg_annuity(&self, leg: &Leg<f64>, curves: &BootstrapCurveSet) -> Result<f64> {
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

    /// Computes the floating-leg annuity (DV01 of the spread):
    /// Σ (side × τ × notional × DF). Used for basis/xccy swap
    /// quote sensitivities where the quote is the spread.
    fn floating_leg_annuity(&self, leg: &Leg<f64>, curves: &BootstrapCurveSet) -> Result<f64> {
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
}

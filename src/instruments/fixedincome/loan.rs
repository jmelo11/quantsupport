//! Loans own contractual cashflows; market observations and optional prepayment
//! are evaluated by the pricer.

use std::sync::Arc;

use crate::{
    ad::scalar::Scalar,
    core::{
        collateral::Discountable,
        instrument::{AssetClass, Instrument},
        marketdatahandling::forwardraterequest::ForwardRateRequest,
        request::LegsProvider,
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::{cashflow::Cashflow, cashflowtype::CashflowType, leg::Leg},
    models::prepaymentmodel::PrepaymentModel,
    time::date::Date,
    utils::errors::{QSError, Result},
};

use super::loanrepaymentstructure::LoanRepaymentStructure;

/// A generic loan contract assembled from the library's existing cashflows.
///
/// Coupon notionals and redemptions are contractual `f64` amounts. Prepayment
/// is a separate function evaluated by the pricer; it never modifies this leg.
/// Both the leg and the prepayment model use the scalar type `T`.
/// Schedule generation and repayment defaults belong to [`super::makeloan::MakeLoan`].
#[derive(Clone)]
pub struct Loan<T: Scalar = f64> {
    identifier: String,
    notional: f64,
    leg: Leg<T>,
    observations: Vec<Option<ForwardRateRequest>>,
    prepayment: Arc<dyn PrepaymentModel<T>>,
    repayment: Option<LoanRepaymentStructure>,
    credit_index: Option<MarketIndex>,
    recovery: f64,
    initial_amortization_rate: Option<f64>,
}

impl<T: Scalar> Loan<T> {
    /// Creates a contract from an existing leg and one observation per coupon.
    /// `None` observations correspond to fixed coupons. Repayment metadata is
    /// optional and is used only when a fair-rate solve rebuilds amortization.
    ///
    /// # Errors
    /// Rejects invalid principal, inconsistent coupon/redemption schedules,
    /// missing floating observations, invalid credit/recovery terms, or a
    /// discount curve stored on the supplied leg.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn new(
        identifier: String,
        notional: f64,
        leg: Leg<T>,
        observations: Vec<Option<ForwardRateRequest>>,
        prepayment: Arc<dyn PrepaymentModel<T>>,
        repayment: Option<LoanRepaymentStructure>,
        credit_index: Option<MarketIndex>,
        recovery: f64,
    ) -> Result<Self> {
        if leg.discount_index().is_some() {
            return Err(QSError::InvalidValueErr(
                "Loan discount curves belong to the pricing policy".into(),
            ));
        }
        if identifier.trim().is_empty()
            || !notional.is_finite()
            || notional <= 0.0
            || !recovery.is_finite()
            || !(0.0..=1.0).contains(&recovery)
            || credit_index
                .as_ref()
                .is_some_and(|index| !matches!(index, MarketIndex::Credit(_)))
        {
            return Err(QSError::InvalidValueErr(
                "Invalid loan principal or credit terms".into(),
            ));
        }
        let mut balance = notional;
        let mut coupon_count = 0;
        let mut previous_end = Some(leg.first_payment_date());
        let mut redemption_date = None;
        let mut disbursements = 0;
        for cashflow in leg.cashflows() {
            match cashflow {
                CashflowType::FixedRateCoupon(coupon) => {
                    cashflow.validate_coupon()?;
                    let observation = observations.get(coupon_count).ok_or_else(|| {
                        QSError::InvalidValueErr("Missing loan coupon observation entry".into())
                    })?;
                    if observation.is_some() {
                        return Err(QSError::InvalidValueErr(
                            "Fixed coupons cannot require index observations".into(),
                        ));
                    }
                    validate_coupon_balance_and_accrual_dates(
                        balance,
                        coupon.notional(),
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        previous_end,
                    )?;
                    previous_end = Some(coupon.accrual_end_date());
                    redemption_date = Some(coupon.payment_date());
                    coupon_count += 1;
                }
                CashflowType::FloatingRateCoupon(coupon) => {
                    cashflow.validate_coupon()?;
                    let request = observations
                        .get(coupon_count)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| {
                            QSError::InvalidValueErr(
                                "Floating loan coupon requires an observation request".into(),
                            )
                        })?;
                    if request.market_index() != *coupon.market_index()
                        || request
                            .start_date()
                            .zip(request.end_date())
                            .is_none_or(|(start, end)| end <= start)
                    {
                        return Err(QSError::InvalidValueErr(
                            "Invalid loan index observation".into(),
                        ));
                    }
                    validate_coupon_balance_and_accrual_dates(
                        balance,
                        coupon.notional(),
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        previous_end,
                    )?;
                    previous_end = Some(coupon.accrual_end_date());
                    redemption_date = Some(coupon.payment_date());
                    coupon_count += 1;
                }
                CashflowType::OptionEmbeddedCoupon(coupon) => {
                    cashflow.validate_coupon()?;
                    let request = observations
                        .get(coupon_count)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| {
                            QSError::InvalidValueErr(
                                "Bounded floating loan coupon requires an observation request"
                                    .into(),
                            )
                        })?;
                    if request.market_index() != *coupon.market_index()
                        || request
                            .start_date()
                            .zip(request.end_date())
                            .is_none_or(|(start, end)| end <= start)
                    {
                        return Err(QSError::InvalidValueErr(
                            "Invalid loan index observation".into(),
                        ));
                    }
                    validate_coupon_balance_and_accrual_dates(
                        balance,
                        coupon.notional(),
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        previous_end,
                    )?;
                    previous_end = Some(coupon.accrual_end_date());
                    redemption_date = Some(coupon.payment_date());
                    coupon_count += 1;
                }
                CashflowType::Redemption(redemption) => {
                    let amount = redemption.amount()?;
                    if redemption_date != Some(redemption.payment_date())
                        || !amount.is_finite()
                        || amount < 0.0
                        || amount > notional.mul_add(1e-10, balance)
                    {
                        return Err(QSError::InvalidValueErr(
                            "Invalid contractual loan redemption".into(),
                        ));
                    }
                    balance = (balance - amount).max(0.0);
                }
                CashflowType::Disbursement(advance) => {
                    if coupon_count != 0
                        || disbursements != 0
                        || advance.payment_date() != leg.first_payment_date()
                        || (advance.amount()? - notional).abs() > notional * 1e-10
                    {
                        return Err(QSError::InvalidValueErr(
                            "Loan leg requires one initial principal advance".into(),
                        ));
                    }
                    disbursements += 1;
                }
                _ => {
                    return Err(QSError::InvalidValueErr(
                        "Loan legs require coupons, redemptions and an initial advance".into(),
                    ));
                }
            }
        }
        if disbursements != 1
            || coupon_count == 0
            || coupon_count != observations.len()
            || balance > notional * 1e-10
        {
            return Err(QSError::InvalidValueErr(
                "Loan cashflows must repay the full original principal".into(),
            ));
        }
        Ok(Self {
            identifier,
            notional,
            leg: leg.with_side(Side::LongReceive),
            observations,
            prepayment,
            repayment,
            credit_index,
            recovery,
            initial_amortization_rate: None,
        })
    }

    /// Initial annual rate used to freeze builder-generated amortization.
    /// Directly assembled contracts may have no associated construction rate.
    #[must_use]
    pub const fn initial_amortization_rate(&self) -> Option<f64> {
        self.initial_amortization_rate
    }

    /// Records the construction rate associated with the existing principal schedule.
    #[must_use]
    pub const fn with_initial_amortization_rate(mut self, rate: f64) -> Self {
        self.initial_amortization_rate = Some(rate);
        self
    }

    /// Original contractual principal.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
    /// Contractual cashflows, unaffected by prepayment or valuation.
    #[must_use]
    pub const fn contractual_leg(&self) -> &Leg<T> {
        &self.leg
    }
    /// Coupon observations; consecutive coupons may share an annual reset.
    #[must_use]
    pub fn coupon_rate_observations(&self) -> &[Option<ForwardRateRequest>] {
        &self.observations
    }
    /// Prepayment function evaluated on the pricer's market and balance state.
    #[must_use]
    pub fn prepayment_model(&self) -> &dyn PrepaymentModel<T> {
        self.prepayment.as_ref()
    }

    /// Converts contractual cashflow scalars and installs a target-scalar prepayment model.
    ///
    /// An arbitrary prepayment model cannot be converted between scalar types.
    /// Supply an equivalent model for `U`, such as the same portable prepayment
    /// configuration or a custom model that retains the required AD dependencies.
    /// All other contract terms are preserved.
    #[must_use]
    pub fn into_scalar<U: Scalar>(self, prepayment: Arc<dyn PrepaymentModel<U>>) -> Loan<U>
    where
        Leg<U>: From<Leg<T>>,
    {
        Loan {
            identifier: self.identifier,
            notional: self.notional,
            leg: self.leg.into(),
            observations: self.observations,
            prepayment,
            repayment: self.repayment,
            credit_index: self.credit_index,
            recovery: self.recovery,
            initial_amortization_rate: self.initial_amortization_rate,
        }
    }
    /// Builder-provided structure for optional fair-rate schedule rebuilding.
    #[must_use]
    pub const fn repayment_structure(&self) -> Option<&LoanRepaymentStructure> {
        self.repayment.as_ref()
    }
    /// Optional survival curve.
    #[must_use]
    pub const fn credit_curve_index(&self) -> Option<&MarketIndex> {
        self.credit_index.as_ref()
    }
    /// Recovery fraction on default.
    #[must_use]
    pub const fn recovery_fraction(&self) -> f64 {
        self.recovery
    }
    /// Initial principal advance date.
    #[must_use]
    pub const fn start_date(&self) -> Date {
        self.leg.first_payment_date()
    }
    /// Last contractual payment date.
    #[must_use]
    pub const fn maturity_date(&self) -> Date {
        self.leg.last_payment_date()
    }
    /// Payment currency.
    #[must_use]
    pub fn currency(&self) -> Currency {
        self.leg.currency()
    }
}

/// Checks that a coupon uses the contractual opening balance and contiguous dates.
///
/// # Errors
/// Rejects nonpositive/nonfinite coupon notionals, a balance mismatch beyond
/// relative tolerance 1e-10, empty/reversed accrual intervals, or a gap/overlap.
fn validate_coupon_balance_and_accrual_dates(
    balance: f64,
    notional: f64,
    start: Date,
    end: Date,
    previous_end: Option<Date>,
) -> Result<()> {
    if !notional.is_finite()
        || notional <= 0.0
        || (balance - notional).abs() > balance.max(1.0) * 1e-10
        || start >= end
        || previous_end.is_some_and(|previous| previous != start)
    {
        return Err(QSError::InvalidValueErr("Loan coupons must use the contractual outstanding principal and contiguous accrual periods".into()));
    }
    Ok(())
}

impl<T: Scalar> Instrument for Loan<T> {
    /// Returns an owned copy of the contract identifier.
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}
impl<T: Scalar> LegsProvider<T> for Loan<T> {
    /// Returns the single contractual leg, including the initial advance.
    fn legs(&self) -> &[Leg<T>] {
        std::slice::from_ref(&self.leg)
    }
}
impl<T: Scalar> Discountable for Loan<T> {
    /// Classifies the loan as fixed income for discount-policy selection.
    fn asset_class(&self) -> AssetClass {
        AssetClass::FixedIncome
    }
    /// Returns the payment currency of the contractual leg.
    fn currency(&self) -> Currency {
        self.leg.currency()
    }
}

/// A signed position in a loan contract.
#[derive(Clone)]
pub struct LoanTrade<T: Scalar = f64> {
    instrument: Loan<T>,
    trade_date: Date,
    side: Side,
}
impl<T: Scalar> LoanTrade<T> {
    /// Creates a loan position.
    ///
    /// `trade_date` records the position's execution date; the market provider
    /// supplies the valuation date. `side` signs projected receipts without
    /// changing the contract's principal or cashflows.
    #[must_use]
    pub const fn new(instrument: Loan<T>, trade_date: Date, side: Side) -> Self {
        Self {
            instrument,
            trade_date,
            side,
        }
    }

    /// Converts the loan using an explicit target-scalar prepayment model.
    ///
    /// Preserves the trade date and side. See [`Loan::into_scalar`] for model
    /// conversion requirements.
    #[must_use]
    pub fn into_scalar<U: Scalar>(self, prepayment: Arc<dyn PrepaymentModel<U>>) -> LoanTrade<U>
    where
        Leg<U>: From<Leg<T>>,
    {
        LoanTrade::new(
            self.instrument.into_scalar(prepayment),
            self.trade_date,
            self.side,
        )
    }
}
impl<T: Scalar> Trade<Loan<T>> for LoanTrade<T> {
    /// Returns the underlying loan contract.
    fn instrument(&self) -> &Loan<T> {
        &self.instrument
    }
    /// Returns the position's execution date.
    fn trade_date(&self) -> Date {
        self.trade_date
    }
    /// Returns the direction used to sign the loan's projected receipts.
    fn side(&self) -> Side {
        self.side
    }
}
impl<T: Scalar> LegsProvider<T> for LoanTrade<T> {
    /// Exposes contractual cashflows; projected receipts belong to pricing state.
    fn legs(&self) -> &[Leg<T>] {
        self.instrument.legs()
    }
}

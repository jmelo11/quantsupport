//! Loans own contractual cashflows; market observations and optional prepayment
//! are evaluated by the pricer.

use std::sync::Arc;

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
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
/// Schedule generation and repayment defaults belong to [`super::makeloan::MakeLoan`].
#[derive(Clone)]
pub struct Loan<T: Scalar = f64> {
    identifier: String,
    notional: f64,
    leg: Leg<T>,
    observations: Vec<Option<ForwardRateRequest>>,
    prepayment: Arc<dyn PrepaymentModel<DualFwd>>,
    repayment: Option<LoanRepaymentStructure>,
    credit_index: Option<MarketIndex>,
    recovery: f64,
}

impl<T: Scalar> Loan<T> {
    /// Creates a contract from an existing leg and one observation per coupon.
    /// `None` observations correspond to fixed coupons. Repayment metadata is
    /// optional and is used only when a fair-rate solve rebuilds amortization.
    ///
    /// # Errors
    /// Rejects invalid principal, inconsistent coupon/redemption schedules,
    /// missing floating observations, or invalid credit/recovery terms.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn new(
        identifier: String,
        notional: f64,
        leg: Leg<T>,
        observations: Vec<Option<ForwardRateRequest>>,
        prepayment: Arc<dyn PrepaymentModel<DualFwd>>,
        repayment: Option<LoanRepaymentStructure>,
        credit_index: Option<MarketIndex>,
        recovery: f64,
    ) -> Result<Self> {
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
                    validate_coupon(
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
                    validate_coupon(
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
        })
    }

    /// Original contractual principal.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
    /// Contractual cashflows, unaffected by prepayment or valuation.
    #[must_use]
    pub const fn leg(&self) -> &Leg<T> {
        &self.leg
    }
    /// Coupon observations; consecutive coupons may share an annual reset.
    #[must_use]
    pub fn observations(&self) -> &[Option<ForwardRateRequest>] {
        &self.observations
    }
    /// Prepayment function evaluated on the pricer's market and balance state.
    #[must_use]
    pub fn prepayment(&self) -> &dyn PrepaymentModel<DualFwd> {
        self.prepayment.as_ref()
    }
    /// Builder-provided structure for optional fair-rate schedule rebuilding.
    #[must_use]
    pub const fn repayment(&self) -> Option<&LoanRepaymentStructure> {
        self.repayment.as_ref()
    }
    /// Optional survival curve.
    #[must_use]
    pub const fn credit_index(&self) -> Option<&MarketIndex> {
        self.credit_index.as_ref()
    }
    /// Recovery fraction on default.
    #[must_use]
    pub const fn recovery(&self) -> f64 {
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

fn validate_coupon(
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
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}
impl<T: Scalar> LegsProvider<T> for Loan<T> {
    fn legs(&self) -> &[Leg<T>] {
        std::slice::from_ref(&self.leg)
    }
}
impl<T: Scalar> Discountable for Loan<T> {
    fn asset_class(&self) -> AssetClass {
        AssetClass::FixedIncome
    }
    fn discount_index(&self) -> Option<MarketIndex> {
        self.leg.discount_index()
    }
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
    #[must_use]
    pub const fn new(instrument: Loan<T>, trade_date: Date, side: Side) -> Self {
        Self {
            instrument,
            trade_date,
            side,
        }
    }
}
impl<T: Scalar> Trade<Loan<T>> for LoanTrade<T> {
    fn instrument(&self) -> &Loan<T> {
        &self.instrument
    }
    fn trade_date(&self) -> Date {
        self.trade_date
    }
    fn side(&self) -> Side {
        self.side
    }
}
impl<T: Scalar> LegsProvider<T> for LoanTrade<T> {
    fn legs(&self) -> &[Leg<T>] {
        self.instrument.legs()
    }
}

macro_rules! convert_loan {
    ($from:ty, $to:ty) => {
        impl From<Loan<$from>> for Loan<$to> {
            fn from(value: Loan<$from>) -> Self {
                Self {
                    identifier: value.identifier,
                    notional: value.notional,
                    leg: value.leg.into(),
                    observations: value.observations,
                    prepayment: value.prepayment,
                    repayment: value.repayment,
                    credit_index: value.credit_index,
                    recovery: value.recovery,
                }
            }
        }
        impl From<LoanTrade<$from>> for LoanTrade<$to> {
            fn from(value: LoanTrade<$from>) -> Self {
                Self::new(value.instrument.into(), value.trade_date, value.side)
            }
        }
    };
}
convert_loan!(f64, DualFwd);
convert_loan!(DualFwd, f64);

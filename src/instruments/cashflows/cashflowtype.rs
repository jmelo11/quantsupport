use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    instruments::cashflows::{
        cashflow::SimpleCashflow, fixedratecoupon::FixedRateCoupon,
        floatingratecoupon::FloatingRateCoupon, optionembeddedcashflow::OptionEmbeddedCashflow,
        optionembeddedcoupon::OptionEmbeddedCoupon,
    },
};

/// An enumeration representing different types of cash flows that can occur in financial instruments.
#[derive(Clone)]
pub enum CashflowType<T: Scalar> {
    /// A fixed rate coupon, where the cash flow is determined by a fixed interest rate applied to the notional amount.
    FixedRateCoupon(FixedRateCoupon<T>),
    /// A floating rate coupon, where the cash flow is determined by a variable interest rate (often linked to an index) applied to the notional amount.
    FloatingRateCoupon(FloatingRateCoupon<T>),
    /// An option-embedded coupon, where the cash flow is determined by a payoff function that may include optionality features (e.g., caps, floors).
    OptionEmbeddedCoupon(OptionEmbeddedCoupon<T>),
    /// A simple cash flow representing a redemption of the notional amount at a given time.
    Redemption(SimpleCashflow<f64>),
    /// A simple cash flow representing a disbursement of the notional amount at the start of the instrument.
    Disbursement(SimpleCashflow<f64>),
    /// Constant amount
    ConstantAmount(SimpleCashflow<f64>),
    /// An option-embedded cash flow, where the cash flow is determined by a payoff function that may include optionality features (e.g., caps, floors).
    OptionEmbeddedCashflow(OptionEmbeddedCashflow<T>),
}

impl From<CashflowType<f64>> for CashflowType<DualFwd> {
    fn from(value: CashflowType<f64>) -> Self {
        match value {
            CashflowType::FixedRateCoupon(coupon) => Self::FixedRateCoupon(coupon.into()),
            CashflowType::FloatingRateCoupon(coupon) => Self::FloatingRateCoupon(coupon.into()),
            CashflowType::OptionEmbeddedCoupon(coupon) => Self::OptionEmbeddedCoupon(coupon.into()),
            CashflowType::Redemption(cashflow) => Self::Redemption(cashflow),
            CashflowType::Disbursement(cashflow) => Self::Disbursement(cashflow),
            CashflowType::ConstantAmount(cashflow) => Self::ConstantAmount(cashflow),
            CashflowType::OptionEmbeddedCashflow(cashflow) => {
                Self::OptionEmbeddedCashflow(cashflow.into())
            }
        }
    }
}

impl From<CashflowType<DualFwd>> for CashflowType<f64> {
    fn from(value: CashflowType<DualFwd>) -> Self {
        match value {
            CashflowType::FixedRateCoupon(coupon) => Self::FixedRateCoupon(coupon.into()),
            CashflowType::FloatingRateCoupon(coupon) => Self::FloatingRateCoupon(coupon.into()),
            CashflowType::OptionEmbeddedCoupon(coupon) => Self::OptionEmbeddedCoupon(coupon.into()),
            CashflowType::Redemption(cashflow) => Self::Redemption(cashflow),
            CashflowType::Disbursement(cashflow) => Self::Disbursement(cashflow),
            CashflowType::ConstantAmount(cashflow) => Self::ConstantAmount(cashflow),
            CashflowType::OptionEmbeddedCashflow(cashflow) => {
                Self::OptionEmbeddedCashflow(cashflow.into())
            }
        }
    }
}

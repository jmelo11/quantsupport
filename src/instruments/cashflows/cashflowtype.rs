use crate::{
    ad::adreal::{ADReal, IsReal},
    instruments::cashflows::{
        cashflow::SimpleCashflow, fixedratecoupon::FixedRateCoupon,
        floatingratecoupon::FloatingRateCoupon, optionembeddedcoupon::OptionEmbeddedCoupon,
    },
};

/// An enumeration representing different types of cash flows that can occur in financial instruments.
pub enum CashflowType<T: IsReal> {
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
}

impl From<CashflowType<f64>> for CashflowType<ADReal> {
    fn from(value: CashflowType<f64>) -> Self {
        match value {
            CashflowType::FixedRateCoupon(coupon) => Self::FixedRateCoupon(coupon.into()),
            CashflowType::FloatingRateCoupon(coupon) => Self::FloatingRateCoupon(coupon.into()),
            CashflowType::OptionEmbeddedCoupon(coupon) => Self::OptionEmbeddedCoupon(coupon.into()),
            CashflowType::Redemption(cashflow) => Self::Redemption(cashflow),
            CashflowType::Disbursement(cashflow) => Self::Disbursement(cashflow),
        }
    }
}

impl From<CashflowType<ADReal>> for CashflowType<f64> {
    fn from(value: CashflowType<ADReal>) -> Self {
        match value {
            CashflowType::FixedRateCoupon(coupon) => Self::FixedRateCoupon(coupon.into()),
            CashflowType::FloatingRateCoupon(coupon) => Self::FloatingRateCoupon(coupon.into()),
            CashflowType::OptionEmbeddedCoupon(coupon) => Self::OptionEmbeddedCoupon(coupon.into()),
            CashflowType::Redemption(cashflow) => Self::Redemption(cashflow),
            CashflowType::Disbursement(cashflow) => Self::Disbursement(cashflow),
        }
    }
}

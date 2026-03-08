use crate::{
    ad::adreal::ADReal,
    instruments::cashflows::{
        cashflow::SimpleCashflow, fixedratecoupon::FixedRateCoupon,
        floatingratecoupon::FloatingRateCoupon, optionembeddedcoupon::OptionEmbeddedCoupon,
    },
};

/// An enumeration representing different types of cash flows that can occur in financial instruments.
pub enum CashflowType {
    /// A fixed rate coupon, where the cash flow is determined by a fixed interest rate applied to the notional amount.
    FixedRateCoupon(FixedRateCoupon<ADReal>),
    /// A floating rate coupon, where the cash flow is determined by a variable interest rate (often linked to an index) applied to the notional amount.
    FloatingRateCoupon(FloatingRateCoupon<ADReal>),
    /// An option-embedded coupon, where the cash flow is determined by a payoff function that may include optionality features (e.g., caps, floors).
    OptionEmbeddedCoupon(OptionEmbeddedCoupon<ADReal>),
    /// A simple cash flow representing a redemption of the notional amount at a given time.
    Redemption(SimpleCashflow<f64>),
    /// A simple cash flow representing a disbursement of the notional amount at the start of the instrument.
    Disbursement(SimpleCashflow<f64>),
}

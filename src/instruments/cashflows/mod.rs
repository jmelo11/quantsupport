/// Defines dated payments, redemptions and the common cashflow interface.
pub mod cashflow;
/// Represents the supported coupon and payment variants.
pub mod cashflowtype;
/// Constructs cashflows from reusable coupon terms.
pub mod coupons;
/// Calculates fixed coupon payments from their rate and accrual conventions.
pub mod fixedratecoupon;
/// Calculates floating coupons from index observations and contractual spreads.
pub mod floatingratecoupon;
/// Groups related cashflows into a contractual payment leg.
pub mod leg;
/// Builds payment legs from schedules and coupon specifications.
pub mod makeleg;
/// Calculates option payoffs embedded in dated cashflows.
pub mod optionembeddedcashflow;
/// Represents coupon terms with embedded optionality.
pub mod optionembeddedcoupon;
/// Defines the arithmetic operations used to evaluate cashflow payoffs.
pub mod payoffops;

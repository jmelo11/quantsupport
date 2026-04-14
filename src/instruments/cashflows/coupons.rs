use crate::{
    ad::scalar::Scalar, instruments::cashflows::{cashflow::Cashflow, payoffops::PayoffOps}, time::date::Date,
    utils::errors::Result,
};

/// A [`LinearCoupon`] is a type of [`Cashflow`] that represents a periodic payment.
///
/// In addition to the amount and payment date, a coupon also has an accrual period defined by a start and end date,
/// and can calculate the accrued amount for a given period.
pub trait LinearCoupon<T>: Cashflow<T>
where
    T: Scalar,
{
    /// Returns the accrued amount between two dates.
    ///
    /// ## Errors
    /// Returns an error if the accrued amount cannot be calculated.
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<T>;

    /// Returns the accrual start date.
    fn accrual_start_date(&self) -> Date;

    /// Returns the accrual end date.
    fn accrual_end_date(&self) -> Date;

    /// Returns the notional amount associated with this coupon.
    fn notional(&self) -> f64;
}

/// A [`NonLinearCoupon`] is a coupon that contains some optionality.
pub trait NonLinearCoupon<T> {
    /// Returns the payoff computation tree.
    fn payoff_description(&self) -> PayoffOps;

    /// Returns the accrued amount between two dates.
    ///
    /// # Errors
    /// Returns an error if the accrued amount cannot be calculated.
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<T>;

    /// Returns the accrual start date.
    fn accrual_start_date(&self) -> Date;

    /// Returns the accrual end date.
    fn accrual_end_date(&self) -> Date;

    /// Returns the notional amount associated with this coupon.
    fn notional(&self) -> f64;

    /// Returns the payment date of the coupon.
    fn payment_date(&self) -> Date;
}

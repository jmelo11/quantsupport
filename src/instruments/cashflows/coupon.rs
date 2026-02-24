use crate::{
    ad::adreal::IsReal, instruments::cashflows::cashflow::Cashflow, time::date::Date,
    utils::errors::Result,
};

/// # `Coupon`
///
/// A `Coupon` is a type of `Cashflow` that represents a periodic payment, typically associated with bonds.
/// In addition to the amount and payment date, a coupon also has an accrual period defined by a start and end date,
/// and can calculate the accrued amount for a given period.
pub trait Coupon<T>: Cashflow<T>
where
    T: IsReal,
{
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
}

use crate::{ad::adreal::IsReal, time::date::Date, utils::errors::Result};

/// # `Cashflow`
///
/// A `Cashflow` represents a single payment that occurs at a specific date. It has an amount and a payment date.
pub trait Cashflow<T>
where
    T: IsReal,
{
    /// Returns the amount of the cashflow.
    ///
    /// # Errors
    /// Returns an error if the amount cannot be calculated.
    fn amount(&self) -> Result<T>;

    /// Returns the payment date of the cashflow.
    fn payment_date(&self) -> Date;
}


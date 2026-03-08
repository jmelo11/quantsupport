use crate::{ad::adreal::IsReal, time::date::Date, utils::errors::Result};

/// A [`Cashflow`] represents a single payment that occurs at a specific date. It has an amount and a payment date.
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

/// A [`SimpleCashflow`] is the most basic representation of a payable cashflow.
#[derive(Clone)]
pub struct SimpleCashflow<T>
where
    T: IsReal,
{
    amount: T,
    payment_date: Date,
}

impl<T> SimpleCashflow<T>
where
    T: IsReal,
{
    /// Creates a new [`SimpleCashflow`] with the given amount and payment date.
    pub const fn new(amount: T, payment_date: Date) -> Self {
        Self {
            amount,
            payment_date,
        }
    }
}

impl<T> Cashflow<T> for SimpleCashflow<T>
where
    T: IsReal,
{
    fn amount(&self) -> Result<T> {
        Ok(self.amount)
    }

    fn payment_date(&self) -> Date {
        self.payment_date
    }
}

use std::{cell::RefCell, rc::Rc};

use crate::{
    ad::scalar::Scalar,
    rates::compounding::Compounding,
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::Result,
};

/// Base trait for rate term structures.
///
/// This trait defines the common interface for all interest rate term structures, including methods
/// to get the reference date, calculate discount factors, and
/// compute forward rates. Specific types of term structures (e.g., flat forward,
/// zero curve) will implement this trait with their own logic for these calculations.
pub trait InterestRatesTermStructure<T>
where
    T: Scalar,
{
    /// Returns the reference date for the given curve.
    fn reference_date(&self) -> Date;
    /// Calculates the discount factor for the given date.
    ///
    /// # Errors
    /// Returns an error if the discount factor cannot be computed for the date.
    fn discount_factor(&self, date: Date) -> Result<T>;
    /// Calculates the forward rate between two dates with the specified compounding and frequency.
    ///
    /// # Errors
    /// Returns an error if the forward rate cannot be computed for the date range.
    fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<T>;

    /// Returns the nodes of the term structure, if available.
    fn nodes(&self) -> Option<Vec<(Date, T)>>;

    /// Returns the day count convention used by the term structure, if available.
    fn day_counter(&self) -> Option<DayCounter>;

    /// Calculates the discount factor for a given year fraction from the reference date.
    ///
    /// Times must use the curve's day-count convention.
    ///
    /// # Errors
    /// Returns an error if the discount factor cannot be computed for the given time.
    fn discount_factor_from_time(&self, t: f64) -> Result<T>;

    /// Calculates the forward rate between two year fractions.
    ///
    /// Times use the curve's reference date and day count. The supplied curve
    /// implementations return the simple annualized rate
    /// `(DF(start) / DF(end) - 1) / (end - start)`, using a short finite
    /// interval when the endpoints coincide; no compounding convention is selected here.
    ///
    /// # Errors
    /// Returns an error if the forward rate cannot be computed for the given time interval.
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<T>;
}

impl<T: Scalar, C: InterestRatesTermStructure<T> + ?Sized> InterestRatesTermStructure<T>
    for Rc<RefCell<C>>
{
    fn reference_date(&self) -> Date {
        self.borrow().reference_date()
    }
    fn discount_factor(&self, date: Date) -> Result<T> {
        self.borrow().discount_factor(date)
    }
    fn forward_rate(
        &self,
        start: Date,
        end: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<T> {
        self.borrow().forward_rate(start, end, comp, freq)
    }
    fn nodes(&self) -> Option<Vec<(Date, T)>> {
        self.borrow().nodes()
    }
    fn day_counter(&self) -> Option<DayCounter> {
        self.borrow().day_counter()
    }
    fn discount_factor_from_time(&self, time: f64) -> Result<T> {
        self.borrow().discount_factor_from_time(time)
    }
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<T> {
        self.borrow().forward_rate_from_time(start, end)
    }
}

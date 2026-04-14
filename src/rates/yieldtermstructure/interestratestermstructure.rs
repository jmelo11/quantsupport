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
    /// # Errors
    /// Returns an error if the discount factor cannot be computed for the given time.
    fn discount_factor_from_time(&self, t: f64) -> Result<T>;

    /// Calculates the forward rate between two year fractions.
    ///
    /// # Errors
    /// Returns an error if the forward rate cannot be computed for the given time interval.
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<T>;
}

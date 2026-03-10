use crate::{
    ad::adreal::IsReal,
    rates::compounding::Compounding,
    time::{date::Date, enums::Frequency},
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
    T: IsReal,
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
}

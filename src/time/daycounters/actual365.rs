use super::daycount::DayCount;
use crate::time::date::Date;

/// # `Actual365` (Fixed)
/// 
/// Actual/365 day count convention.
/// 
/// ## Example
/// ```
/// use quantsupport::time::date::Date;
/// use quantsupport::time::daycounters::actual365::Actual365;
/// use quantsupport::time::daycounters::daycount::DayCount;
///
/// let start = Date::new(2020, 1, 1);
/// let end = Date::new(2020, 2, 1);
/// assert_eq!(Actual365::day_count(start, end), 31);
/// assert_eq!(Actual365::year_fraction(start, end), 31.0 / 365.0);
/// ```
pub struct Actual365;

impl DayCount for Actual365 {
    fn day_count(start: Date, end: Date) -> i64 {
        end - start
    }

    fn year_fraction(start: Date, end: Date) -> f64 {
        let days = i32::try_from(Self::day_count(start, end))
            .unwrap_or_else(|_| panic!("day count should fit in i32"));
        f64::from(days) / 365.0
    }
}

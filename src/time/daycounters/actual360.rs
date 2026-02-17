use super::daycount::DayCount;
use crate::time::date::Date;

/// # `Actual360`
///
///
/// Day count convention.
/// 
/// ## Example
/// ```
/// use quantsupport::time::date::Date;
/// use quantsupport::time::daycounters::actual360::Actual360;
/// use quantsupport::time::daycounters::daycount::DayCount;
///
/// let start = Date::new(2020, 1, 1);
/// let end = Date::new(2020, 2, 1);
/// assert_eq!(Actual360::day_count(start, end), 31);
/// assert_eq!(Actual360::year_fraction(start, end), 31.0 / 360.0);
/// ```
pub struct Actual360;

impl DayCount for Actual360 {
    fn day_count(start: Date, end: Date) -> i64 {
        end - start
    }

    fn year_fraction(start: Date, end: Date) -> f64 {
        let days = i32::try_from(Self::day_count(start, end))
            .unwrap_or_else(|_| panic!("day count should fit in i32"));
        f64::from(days) / 360.0
    }
}

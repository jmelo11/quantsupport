use super::daycount::DayCount;
use crate::time::calendars::brazil::Market;
use crate::time::{
    calendar::Calendar,
    calendars::{brazil::Brazil, traits::ImplCalendar},
    date::Date,
};

/// # `Business252`
/// 
/// Business/252 day count convention.
/// Calculates the number of business days between two dates.
/// 
/// ## Example
/// ```
/// use quantsupport::time::date::Date;
/// use quantsupport::time::daycounters::business252::Business252;
/// use quantsupport::time::daycounters::daycount::DayCount;
///
/// let start = Date::new(2020, 1, 1);
/// let end = Date::new(2020, 2, 1);
/// assert_eq!(Business252::day_count(start, end), 22);
/// assert_eq!(Business252::year_fraction(start, end), 22.0 / 252.0);
/// ```
pub struct Business252;

impl DayCount for Business252 {
    fn day_count(start: Date, end: Date) -> i64 {
        let calendar = Calendar::Brazil(Brazil::new(Market::Settlement));

        let count = i64::try_from(calendar.business_day_list(start, end).len())
            .unwrap_or_else(|_| panic!("business day count should fit in i64"));
        if end < start {
            -count
        } else {
            count
        }
    }

    fn year_fraction(start: Date, end: Date) -> f64 {
        let days = i32::try_from(Self::day_count(start, end))
            .unwrap_or_else(|_| panic!("day count should fit in i32"));
        f64::from(days) / 252.0
    }
}

#[cfg(test)]
mod test {
    use crate::time::date::Date;
    use crate::time::daycounters::business252::Business252;
    use crate::time::daycounters::daycount::DayCount;

    #[test]
    fn test_business252() {
        let start = Date::new(2020, 1, 1);
        let end = Date::new(2020, 2, 1);
        assert_eq!(Business252::day_count(start, end), 22);
        let yf = Business252::year_fraction(start, end);
        assert!((yf - 22.0 / 252.0).abs() < 1e-12);
    }
}

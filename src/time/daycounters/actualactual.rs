use std::cmp::Ordering;

use super::daycount::DayCount;
use crate::time::date::Date;

/// # `ActualActual`
///
/// Actual/Actual day count convention.
///
/// ## Example
/// ```
/// use quantsupport::time::date::Date;
/// use quantsupport::time::daycounters::actualactual::ActualActual;
/// use quantsupport::time::daycounters::daycount::DayCount;
///
/// let start = Date::new(2020, 1, 1);
/// let end = Date::new(2020, 2, 1);
/// assert_eq!(ActualActual::day_count(start, end), 31);
/// assert_eq!(ActualActual::year_fraction(start, end), 31.0 / 366.0);
/// ```
pub struct ActualActual;

const fn days_in_year(year: i32) -> i32 {
    if Date::is_leap_year(year) {
        366
    } else {
        365
    }
}

impl DayCount for ActualActual {
    fn day_count(start: Date, end: Date) -> i64 {
        end - start
    }

    fn year_fraction(start: Date, end: Date) -> f64 {
        let days = Self::day_count(start, end);

        let y1 = start.year();
        let y2 = end.year();

        match y1.cmp(&y2) {
            Ordering::Equal => {
                let days =
                    i32::try_from(days).unwrap_or_else(|_| panic!("day count should fit in i32"));
                f64::from(days) / f64::from(days_in_year(y1))
            }
            Ordering::Less => {
                let mut sum = 0.0;
                let start_days = i32::try_from(Date::new(y1 + 1, 1, 1) - start)
                    .unwrap_or_else(|_| panic!("day count should fit in i32"));
                sum += f64::from(start_days) / f64::from(days_in_year(y1));
                for _year in y1 + 1..y2 - 1 {
                    sum += 1.0;
                }
                let end_days = i32::try_from(end - Date::new(y2, 1, 1))
                    .unwrap_or_else(|_| panic!("day count should fit in i32"));
                sum += f64::from(end_days) / f64::from(days_in_year(y2));

                sum
            }
            Ordering::Greater => {
                let mut sum = 0.0;
                let end_days = i32::try_from(Date::new(y2 + 1, 1, 1) - end)
                    .unwrap_or_else(|_| panic!("day count should fit in i32"));
                sum -= f64::from(end_days) / f64::from(days_in_year(y2));
                for _year in y2 + 1..y1 - 1 {
                    sum -= 1.0;
                }
                let start_days = i32::try_from(start - Date::new(y1, 1, 1))
                    .unwrap_or_else(|_| panic!("day count should fit in i32"));
                sum -= f64::from(start_days) / f64::from(days_in_year(y1));
                sum
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::ActualActual;
    use crate::time::{date::Date, daycounters::daycount::DayCount};

    #[test]
    fn test_actualactual_day_count() {
        let start = Date::new(2020, 1, 1);
        let end = Date::new(2020, 2, 1);
        assert_eq!(ActualActual::day_count(start, end), 31);
    }

    #[test]
    fn test_actualactual_year_fraction() {
        let start = Date::new(2020, 1, 1);
        let end = Date::new(2020, 2, 1);
        let yf = ActualActual::year_fraction(start, end);
        assert!((yf - 31.0 / 366.0).abs() < 1e-12);
    }

    #[test]
    fn test_actualactual_year_fraction2() {
        let start = Date::new(2020, 1, 1);
        let end = Date::new(2021, 1, 1);
        let yf = ActualActual::year_fraction(start, end);
        assert!((yf - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_actualactual_year_fraction3() {
        let start = Date::new(2021, 1, 1);
        let end = Date::new(2020, 1, 1);
        let yf = ActualActual::year_fraction(start, end);
        assert!((yf + 1.0).abs() < 1e-12);
    }
}

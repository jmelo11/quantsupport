use crate::time::{date::Date, daycounters::daycount::DayCount};

/// # `Thirty360` (ISMA)
/// 
/// Convention: if the starting date is the 31st of a
/// month, it becomes equal to the 30th of the same month.
/// 
/// If the ending date is the 31st of a month and the starting
/// date is the 30th or 31th of a month, the ending date
/// also becomes equal to the 30th of the month.
/// 
/// ## Example
/// ```
/// use quantsupport::time::date::Date;
/// use quantsupport::time::daycounters::thirty360::Thirty360;
/// use quantsupport::time::daycounters::daycount::DayCount;
///
/// let start = Date::new(2020, 1, 1);
/// let end = Date::new(2020, 2, 1);
/// assert_eq!(Thirty360::day_count(start, end), 30);
/// assert_eq!(Thirty360::year_fraction(start, end), 30.0 / 360.0);
/// ```
pub struct Thirty360;

impl DayCount for Thirty360 {
    fn day_count(start: Date, end: Date) -> i64 {
        let d1 = i64::from(start.day());
        let d2 = i64::from(end.day());
        let m1 = i64::from(start.month());
        let m2 = i64::from(end.month());
        let y1 = i64::from(start.year());
        let y2 = i64::from(end.year());

        let dd1 = if d1 == 31 { 30 } else { d1 };
        let dd2 = if d2 == 31 && dd1 == 30 { 30 } else { d2 };

        360 * (y2 - y1) + 30 * (m2 - m1) + (dd2 - dd1)
    }

    fn year_fraction(start: Date, end: Date) -> f64 {
        let days = i32::try_from(Self::day_count(start, end))
            .unwrap_or_else(|_| panic!("day count should fit in i32"));
        f64::from(days) / 360.0
    }
}

/// # `Thirty360US`
/// 
/// convention: if the starting date is the 31st of a month or
/// the last day of February, it becomes equal to the 30th of the
/// same month.  If the ending date is the 31st of a month and the
/// starting date is the 30th or 31th of a month, the ending date
/// becomes equal to the 30th.  If the ending date is the last of
/// February and the starting date is also the last of February,
/// the ending date becomes equal to the 30th.
/// Also known as "30/360" or "360/360".
/// 
/// ## Example
/// ```
/// use quantsupport::time::date::Date;
/// use quantsupport::time::daycounters::thirty360::Thirty360US;
/// use quantsupport::time::daycounters::daycount::DayCount;
///
/// let start = Date::new(2020, 1, 1);
/// let end = Date::new(2020, 2, 1);
/// assert_eq!(Thirty360US::day_count(start, end), 30);
/// assert_eq!(Thirty360US::year_fraction(start, end), 30.0 / 360.0);
/// ```
pub struct Thirty360US;

const fn is_last_of_february(d: i64, m: i64, y: i32) -> bool {
    if Date::is_leap_year(y) {
        m == 2 && d == 28 + 1
    } else {
        m == 2 && d == 28
    }
}

impl DayCount for Thirty360US {
    fn day_count(start: Date, end: Date) -> i64 {
        let d1 = i64::from(start.day());
        let d2 = i64::from(end.day());
        let m1 = i64::from(start.month());
        let m2 = i64::from(end.month());
        let y1 = start.year();
        let y2 = end.year();

        let dd1 = if d1 == 31 { 30 } else { d1 };
        let dd2 = if d2 == 31 && dd1 >= 30 { 30 } else { d2 };

        let dd1 = if is_last_of_february(dd1, m1, y1) {
            30
        } else {
            dd1
        };
        let dd2 = if is_last_of_february(dd2, m2, y2) {
            30
        } else {
            dd2
        };

        360 * (i64::from(y2) - i64::from(y1)) + 30 * (m2 - m1) + (dd2 - dd1)
    }

    fn year_fraction(start: Date, end: Date) -> f64 {
        let days = i32::try_from(Self::day_count(start, end))
            .unwrap_or_else(|_| panic!("day count should fit in i32"));
        f64::from(days) / 360.0
    }
}

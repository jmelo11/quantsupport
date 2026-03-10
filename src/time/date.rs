use super::enums::{TimeUnit, Weekday};
use super::period::Period;
use crate::utils::errors::Result;
use chrono::{Datelike, Duration, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Extends the [`NaiveDate`] struct from the chrono rustatlas.
/// # Examples
/// ```
/// use quantsupport::time::date::*;
/// use quantsupport::time::enums::TimeUnit;
/// use chrono::NaiveDate;
///
/// let date = NaiveDate::from_ymd_opt(2020, 2, 15).unwrap();
/// assert_eq!(date.days_in_month(), 29);
/// let date = NaiveDate::from_ymd_opt(2020, 5, 15).unwrap();
/// assert_eq!(date.days_in_year(), 366);
/// let date = NaiveDate::from_ymd_opt(2020, 5, 15).unwrap();
/// assert!(date.date_has_leap_year());
/// let date = NaiveDate::from_ymd_opt(2020, 1, 15).unwrap();
/// assert_eq!(date.advance(15, TimeUnit::Days), NaiveDate::from_ymd_opt(2020, 1, 30).unwrap());
/// ```
pub trait NaiveDateExt {
    /// Returns the number of days in the month of this date.
    fn days_in_month(&self) -> i32;
    /// Returns the number of days in the year of this date.
    fn days_in_year(&self) -> i32;
    /// Returns the day of year (1-366) for this date.
    fn day_of_year(&self) -> i32;
    /// Returns whether this date falls in a leap year.
    fn date_has_leap_year(&self) -> bool;
    /// Advances the date by `n` units of the specified `TimeUnit`.
    fn advance(&self, n: i32, units: TimeUnit) -> NaiveDate;
    /// Returns the last day of the month for the given date.
    fn end_of_month(date: NaiveDate) -> NaiveDate;
}

impl NaiveDateExt for NaiveDate {
    fn days_in_month(&self) -> i32 {
        let month = self.month();
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if self.date_has_leap_year() {
                    29
                } else {
                    28
                }
            }
            _ => panic!("Invalid month: {month}"),
        }
    }

    fn days_in_year(&self) -> i32 {
        if self.date_has_leap_year() {
            366
        } else {
            365
        }
    }

    fn day_of_year(&self) -> i32 {
        let mut day = 0;
        for m in 1..self.month() {
            day += Self::from_ymd_opt(self.year(), m, 1)
                .unwrap_or_else(|| panic!("valid date for month start"))
                .days_in_month();
        }
        let day_i32 = i32::try_from(self.day()).unwrap_or_else(|_| panic!("day should fit in i32"));
        day + day_i32
    }

    fn date_has_leap_year(&self) -> bool {
        let year = self.year();
        year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
    }

    fn advance(&self, n: i32, units: TimeUnit) -> NaiveDate {
        let date = *self;
        let flag = n >= 0;
        match units {
            TimeUnit::Days => {
                date + Duration::try_days(i64::from(n)).unwrap_or_else(|| panic!("valid day count"))
            }
            TimeUnit::Weeks => {
                date + Duration::try_days(i64::from(7 * n))
                    .unwrap_or_else(|| panic!("valid day count"))
            }
            TimeUnit::Months => {
                if flag {
                    date + Months::new(
                        u32::try_from(n).unwrap_or_else(|_| panic!("valid month count")),
                    )
                } else {
                    date - Months::new(
                        u32::try_from(-n).unwrap_or_else(|_| panic!("valid month count")),
                    )
                }
            }
            TimeUnit::Years => {
                if flag {
                    date + Months::new(
                        u32::try_from(12 * n).unwrap_or_else(|_| panic!("valid year count")),
                    )
                } else {
                    date - Months::new(
                        u32::try_from(-12 * n).unwrap_or_else(|_| panic!("valid year count")),
                    )
                }
            }
        }
    }

    fn end_of_month(date: NaiveDate) -> NaiveDate {
        let month = date.month();
        let year = date.year();
        let mut end_of_month = Self::from_ymd_opt(year, month, 1)
            .unwrap_or_else(|| panic!("valid date for month start"));
        end_of_month = end_of_month + Months::new(1);
        end_of_month -= Duration::try_days(1).unwrap_or_else(|| panic!("valid day count"));
        end_of_month
    }
}

/// # Implementing [`Add<Period>`] for [`NaiveDate`]
/// Adds a [`Period`] to a [`NaiveDate`].
/// # Examples
/// ```
/// use chrono::NaiveDate;
/// use quantsupport::time::date::Date;
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let date = NaiveDate::from_ymd_opt(2020, 1, 15).unwrap();
/// let period = Period::new(15, TimeUnit::Days);
/// assert_eq!(date + period, NaiveDate::from_ymd_opt(2020, 1, 30).unwrap());
/// ```
impl Add<Period> for NaiveDate {
    type Output = Self;

    fn add(self, rhs: Period) -> Self::Output {
        let n = rhs.length();
        let units = rhs.units();
        self.advance(n, units)
    }
}

/// # Implementing [`Sub<Period>`] for [`NaiveDate`]
/// Subtracts a [`Period`] from a [`NaiveDate`].
/// # Examples
/// ```
/// use chrono::NaiveDate;
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
/// let date = NaiveDate::from_ymd_opt(2020, 1, 15).unwrap();
/// let period = Period::new(15, TimeUnit::Days);
/// assert_eq!(date - period, NaiveDate::from_ymd_opt(2019, 12, 31).unwrap());
/// ```
impl Sub<Period> for NaiveDate {
    type Output = Self;

    fn sub(self, rhs: Period) -> Self::Output {
        let n = rhs.length();
        let units = rhs.units();
        self.advance(-n, units)
    }
}

/// Wrapper around the [`NaiveDate`] struct from the chrono rustatlas.
/// # Examples
/// ```
/// use quantsupport::time::date::Date;
///
/// let date = Date::new(2020, 2, 15);
/// assert_eq!(date.day(), 15);
/// assert_eq!(date.month(), 2);
/// assert_eq!(date.year(), 2020);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Date {
    base_date: NaiveDate,
}

impl From<NaiveDate> for Date {
    fn from(base_date: NaiveDate) -> Self {
        Self { base_date }
    }
}

impl Serialize for Date {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s, "%Y-%m-%d").map_err(serde::de::Error::custom)
    }
}

impl Date {
    /// Creates a new [`Date`] from the given year, month, and day.
    #[must_use]
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        let base_date = NaiveDate::from_ymd_opt(year, month, day);
        base_date.map_or_else(|| panic!("Invalid date: {year}-{month}-{day}"), Self::from)
    }

    /// Parses a date string using the specified format.
    ///
    /// # Errors
    /// Returns an error if the provided string cannot be parsed into a [`Date`]
    /// using the specified format.
    pub fn from_str(date: &str, fmt: &str) -> Result<Self> {
        let base_date = NaiveDate::parse_from_str(date, fmt)?;
        Ok(Self::from(base_date))
    }

    /// Formats this date as a string using the specified format.
    #[must_use]
    pub fn to_str(&self, fmt: &str) -> String {
        self.base_date.format(fmt).to_string()
    }

    /// Returns the underlying `NaiveDate`.
    #[must_use]
    pub const fn base_date(&self) -> NaiveDate {
        self.base_date
    }

    /// Returns the day of the month (1-31).
    #[must_use]
    pub fn day(&self) -> u32 {
        self.base_date.day()
    }

    /// Returns the month of the year (1-12).
    #[must_use]
    pub fn month(&self) -> u32 {
        self.base_date.month()
    }

    /// Returns the year.
    #[must_use]
    pub fn year(&self) -> i32 {
        self.base_date.year()
    }

    /// Returns the number of days in the month of this date.
    #[must_use]
    pub fn days_in_month(&self) -> i32 {
        self.base_date.days_in_month()
    }

    /// Returns the day of year (1-366) for this date.
    #[must_use]
    pub fn day_of_year(&self) -> i32 {
        self.base_date.day_of_year()
    }

    /// Returns whether this date falls in a leap year.
    #[must_use]
    pub fn date_has_leap_year(&self) -> bool {
        self.base_date.date_has_leap_year()
    }

    /// Returns whether the given year is a leap year.
    #[must_use]
    pub const fn is_leap_year(year: i32) -> bool {
        year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
    }

    /// Advances this date by `n` units of the specified [`TimeUnit`].
    #[must_use]
    pub fn advance(&self, n: i32, units: TimeUnit) -> Self {
        let base_date = self.base_date.advance(n, units);
        Self::from(base_date)
    }

    /// Adds a [`Period`] to this date.
    #[must_use]
    pub fn add_period(&self, period: Period) -> Self {
        let base_date = self.base_date + period;
        Self::from(base_date)
    }

    /// Returns the last day of the month for the given date.
    #[must_use]
    pub fn end_of_month(date: Self) -> Self {
        let base_date = NaiveDate::end_of_month(date.base_date);
        Self::from(base_date)
    }

    /// Returns the nth occurrence of the specified weekday in the given month and year.
    #[must_use]
    pub fn nth_weekday(n: i32, day_of_week: Weekday, month: u32, year: i32) -> Self {
        let base_date = Self::new(year, month, 1);
        let first = base_date.weekday();
        let skip = n - i32::from(day_of_week >= first);
        let day = 1 + day_of_week + skip * 7 - first;
        let base_date = NaiveDate::from_ymd_opt(
            year,
            month,
            u32::try_from(day).unwrap_or_else(|_| panic!("valid day for nth weekday")),
        )
        .unwrap_or_else(|| panic!("valid date for nth weekday"));
        Self::from(base_date)
    }

    /// Returns the next occurrence of the specified weekday after the given date.
    #[must_use]
    pub fn next_weekday(date: Self, weekday: Weekday) -> Self {
        let wd = date.weekday();
        date + i64::from((if wd > weekday { 7 } else { 0 }) - wd + weekday)
    }

    /// Returns the day of the week for this date.
    #[must_use]
    pub fn weekday(&self) -> Weekday {
        match self.base_date.weekday() {
            chrono::Weekday::Mon => Weekday::Monday,
            chrono::Weekday::Tue => Weekday::Tuesday,
            chrono::Weekday::Wed => Weekday::Wednesday,
            chrono::Weekday::Thu => Weekday::Thursday,
            chrono::Weekday::Fri => Weekday::Friday,
            chrono::Weekday::Sat => Weekday::Saturday,
            chrono::Weekday::Sun => Weekday::Sunday,
        }
    }

    /// Returns the minimum representable date.
    #[must_use]
    pub fn empty() -> Self {
        //min
        Self::from(NaiveDate::MIN)
    }
}

/// # Sub for [`Date`]
/// Subtracts two Dates and returns the difference in days.
/// # Examples
/// ```
/// use quantsupport::time::date::Date;
/// let date1 = Date::new(2020, 2, 15);
/// let date2 = Date::new(2020, 2, 10);
/// assert_eq!(date1 - date2, 5);
/// ```
impl Sub for Date {
    type Output = i64;

    fn sub(self, rhs: Self) -> Self::Output {
        let base_date = self.base_date;
        let rhs_base_date = rhs.base_date;
        (base_date - rhs_base_date).num_days()
    }
}

/// # [`Add<Period>`] for [`Date`]
/// Adds a [`Period`] to [`Date`].
/// # Examples
/// ```
/// use quantsupport::time::date::Date;
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let date = Date::new(2020, 1, 15);
/// let period = Period::new(15, TimeUnit::Days);
/// assert_eq!(date + period, Date::new(2020, 1, 30));
/// ```
impl Add<Period> for Date {
    type Output = Self;

    fn add(self, rhs: Period) -> Self::Output {
        let base_date: NaiveDate = self.base_date + rhs;
        Self::from(base_date)
    }
}

/// # [`Sub<Period>`] for [`Date`]
/// Subtracts a [`Period`] from a [`Date`].
/// # Examples
/// ```
/// use quantsupport::time::date::Date;
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let date = Date::new(2020, 1, 15);
/// let period = Period::new(15, TimeUnit::Days);
/// assert_eq!(date - period, Date::new(2019, 12, 31));
/// ```
impl Sub<Period> for Date {
    type Output = Self;

    fn sub(self, rhs: Period) -> Self::Output {
        let base_date: NaiveDate = self.base_date - rhs;
        Self::from(base_date)
    }
}

/// # [`Add<i64>`] for [`Date`]
/// Adds an [`i64`] to a [`Date`].
/// # Examples
/// ```
/// use quantsupport::time::date::Date;
///
/// let date = Date::new(2020, 1, 15);
/// assert_eq!(date + 15, Date::new(2020, 1, 30));
/// ```
impl Add<i64> for Date {
    type Output = Self;

    fn add(self, rhs: i64) -> Self::Output {
        let base_date: NaiveDate =
            self.base_date + Duration::try_days(rhs).unwrap_or_else(|| panic!("valid day count"));
        Self::from(base_date)
    }
}

/// # Implementing `AddAssign<i64>` for `Date`
/// Adds an i64 to a Date.
/// # Examples
/// ```
/// use quantsupport::time::date::Date;
///
/// let mut date = Date::new(2020, 1, 15);
/// date += 15;
/// assert_eq!(date, Date::new(2020, 1, 30));
/// ```
impl AddAssign<i64> for Date {
    fn add_assign(&mut self, rhs: i64) {
        self.base_date =
            self.base_date + Duration::try_days(rhs).unwrap_or_else(|| panic!("valid day count"));
    }
}

/// # Sub`<i64>` for Date
/// Subtracts an i64 from a Date.
/// # Examples
/// ```
/// use quantsupport::time::date::Date;
///
/// let date = Date::new(2020, 1, 30);
/// assert_eq!(date - 15, Date::new(2020, 1, 15));
/// ```
impl Sub<i64> for Date {
    type Output = Self;

    fn sub(self, rhs: i64) -> Self::Output {
        let base_date: NaiveDate =
            self.base_date - Duration::try_days(rhs).unwrap_or_else(|| panic!("valid day count"));
        Self::from(base_date)
    }
}

/// # Implementing `SubAssign<i64>` for `Date`
/// Subtracts an i64 from a Date.
/// # Examples
/// ```
/// use quantsupport::time::date::Date;
///
/// let mut date = Date::new(2020, 1, 30);
/// date -= 15;
/// assert_eq!(date, Date::new(2020, 1, 15));
/// ```
impl SubAssign<i64> for Date {
    fn sub_assign(&mut self, rhs: i64) {
        self.base_date =
            self.base_date - Duration::try_days(rhs).unwrap_or_else(|| panic!("valid day count"));
    }
}

/// # Display for Date
/// Formats a Date as a string.
/// # Examples
/// ```
/// use quantsupport::time::date::Date;
///
/// let date = Date::new(2020, 1, 15);
/// assert_eq!(date.to_string(), "2020-01-15");
/// ```
impl Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let base_date = self.base_date;
        write!(f, "{}", base_date.format("%Y-%m-%d"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_days_in_month() {
        let date =
            NaiveDate::from_ymd_opt(2020, 2, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert_eq!(date.days_in_month(), 29);

        let date =
            NaiveDate::from_ymd_opt(2021, 2, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert_eq!(date.days_in_month(), 28);

        let date =
            NaiveDate::from_ymd_opt(2021, 4, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert_eq!(date.days_in_month(), 30);

        let date =
            NaiveDate::from_ymd_opt(2021, 7, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert_eq!(date.days_in_month(), 31);
    }

    #[test]
    fn test_days_in_year() {
        let date =
            NaiveDate::from_ymd_opt(2020, 5, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert_eq!(date.days_in_year(), 366);

        let date =
            NaiveDate::from_ymd_opt(2021, 5, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert_eq!(date.days_in_year(), 365);
    }

    #[test]
    fn test_date_has_leap_year() {
        let date =
            NaiveDate::from_ymd_opt(2020, 5, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert!(date.date_has_leap_year());

        let date =
            NaiveDate::from_ymd_opt(2021, 5, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert!(!date.date_has_leap_year());
    }

    #[test]
    fn test_advance() {
        let date =
            NaiveDate::from_ymd_opt(2020, 1, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert_eq!(
            date.advance(15, TimeUnit::Days),
            NaiveDate::from_ymd_opt(2020, 1, 30).unwrap_or_else(|| panic!("date should be valid")),
        );

        let date =
            NaiveDate::from_ymd_opt(2020, 1, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert_eq!(
            date.advance(3, TimeUnit::Weeks),
            NaiveDate::from_ymd_opt(2020, 2, 5).unwrap_or_else(|| panic!("date should be valid")),
        );

        let date =
            NaiveDate::from_ymd_opt(2020, 1, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert_eq!(
            date.advance(2, TimeUnit::Months),
            NaiveDate::from_ymd_opt(2020, 3, 15).unwrap_or_else(|| panic!("date should be valid")),
        );

        let date =
            NaiveDate::from_ymd_opt(2020, 1, 15).unwrap_or_else(|| panic!("date should be valid"));
        assert_eq!(
            date.advance(2, TimeUnit::Years),
            NaiveDate::from_ymd_opt(2022, 1, 15).unwrap_or_else(|| panic!("date should be valid")),
        );
    }

    #[test]
    fn test_addition_with_period() {
        let date =
            NaiveDate::from_ymd_opt(2020, 1, 15).unwrap_or_else(|| panic!("date should be valid"));
        let period = Period::new(15, TimeUnit::Days);
        assert_eq!(
            date + period,
            NaiveDate::from_ymd_opt(2020, 1, 30).unwrap_or_else(|| panic!("date should be valid")),
        );

        let date =
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap_or_else(|| panic!("date should be valid"));
        let period = Period::new(6, TimeUnit::Months);
        assert_eq!(
            date + period,
            NaiveDate::from_ymd_opt(2020, 7, 1).unwrap_or_else(|| panic!("date should be valid")),
        );
    }

    #[test]
    fn test_end_of_month() {
        let date = Date::new(2023, 8, 15);
        let end_date = Date::end_of_month(date);
        assert_eq!(end_date.day(), 31);
    }

    #[test]
    fn test_nth_weekday() {
        let date = Date::nth_weekday(1, Weekday::Monday, 8, 2023);
        assert_eq!(date.day(), 7); // 1st Monday of August 2023 should be on 7th
        assert_eq!(date.month(), 8);
        assert_eq!(date.year(), 2023);

        let date = Date::nth_weekday(3, Weekday::Saturday, 1, 2023);
        assert_eq!(date.day(), 21);
        assert_eq!(date.month(), 1);
        assert_eq!(date.year(), 2023);
    }

    #[test]
    fn test_next_weekday() {
        let date = Date::new(2023, 1, 1);
        let next_wed = Date::next_weekday(date, Weekday::Wednesday);
        assert_eq!(next_wed.day(), 4);
        assert_eq!(next_wed.month(), 1);
        assert_eq!(next_wed.year(), 2023);

        let date = Date::new(2023, 2, 28);
        let next_mon = Date::next_weekday(date, Weekday::Monday);
        assert_eq!(next_mon.day(), 6);
        assert_eq!(next_mon.month(), 3);
        assert_eq!(next_mon.year(), 2023);
    }

    #[test]
    fn test_empty() {
        let date = Date::empty();
        assert_eq!(date, Date::from(NaiveDate::MIN));
    }

    #[test]
    fn test_deserialize() {
        let date = Date::from_str("2020-01-15", "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("date should deserialize: {e}"));
        assert_eq!(date, Date::new(2020, 1, 15));
    }
}

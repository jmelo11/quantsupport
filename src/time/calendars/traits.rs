use crate::{
    time::date::Date,
    time::enums::{BusinessDayConvention, TimeUnit, Weekday},
    time::period::Period,
};

use std::{cmp::Ordering, collections::HashSet};

/// Returns the day of year for Easter Monday for a given year.
///
/// # Arguments
///
/// * `y` - The year for which to calculate Easter Monday
///
/// # Returns
///
/// The day of year (1-366) for Easter Monday in the given year
///
/// # Panics
/// Panics if `y` is outside 1–9999, the supported Gregorian computus range.
#[must_use]
pub fn easter_monday(y: i32) -> i32 {
    assert!((1..=9999).contains(&y), "Easter year must be in 1..=9999");
    // Gregorian computus: calculate the paschal full moon and the following Sunday.
    let century = y / 100;
    let golden_number = y % 19;
    let correction = (century - (century + 8) / 25 + 1) / 3;
    let epact = (19 * golden_number + century - century / 4 - correction + 15) % 30;
    let weekday_correction = (32 + 2 * (century % 4) + 2 * ((y % 100) / 4) - epact - y % 4) % 7;
    let exception = (golden_number + 11 * epact + 22 * weekday_correction) / 451;
    let encoded = epact + weekday_correction - 7 * exception + 114;
    let month = u32::try_from(encoded / 31).unwrap_or_else(|_| panic!("invalid Easter year"));
    let day = u32::try_from(encoded % 31 + 1).unwrap_or_else(|_| panic!("invalid Easter year"));
    (Date::new(y, month, day) + 1).day_of_year()
}

/// Weekend observance for a fixed-date holiday.
#[derive(Clone, Copy)]
pub enum Observance {
    /// Observe the holiday on its calendar date.
    Exact,
    /// Also observe a weekend holiday on the following Monday.
    NextMonday,
    /// Also observe a Sunday holiday on the following Monday.
    SundayToMonday,
    /// Move each weekend date in a consecutive holiday pair forward two days.
    Paired,
}

/// A recurring holiday date rule.
#[derive(Clone, Copy)]
pub enum Holiday {
    /// A month, day and weekend-observance rule.
    Fixed(u32, u32, Observance),
    /// The first given weekday on or after a month and day.
    WeekdayOnOrAfter(u32, u32, Weekday),
    /// An offset from Easter Sunday, e.g. Good Friday = -2, Whit Monday = 50.
    Easter(i32),
}

impl Holiday {
    /// Returns whether this holiday rule includes the date.
    ///
    /// # Panics
    /// Panics for invalid rule date components or an Easter year outside 1–9999.
    #[must_use]
    pub fn contains(self, date: Date) -> bool {
        match self {
            Self::Fixed(month, day, observance) => {
                let actual = Date::new(date.year(), month, day);
                let shift = match (observance, actual.weekday()) {
                    (Observance::NextMonday | Observance::Paired, Weekday::Saturday)
                    | (Observance::Paired, Weekday::Sunday) => 2,
                    (Observance::NextMonday | Observance::SundayToMonday, Weekday::Sunday) => 1,
                    _ => 0,
                };
                date == actual || date == actual + shift
            }
            Self::WeekdayOnOrAfter(month, day, weekday) => {
                let mut holiday = Date::new(date.year(), month, day);
                while holiday.weekday() != weekday {
                    holiday += 1;
                }
                date == holiday
            }
            Self::Easter(offset) => date.day_of_year() == easter_monday(date.year()) - 1 + offset,
        }
    }
}

/// Returns whether the date falls on Monday through Friday.
#[must_use]
pub fn weekday(date: Date) -> bool {
    !matches!(date.weekday(), Weekday::Saturday | Weekday::Sunday)
}

/// Returns whether the date is a weekday outside the supplied holiday rules.
///
/// # Panics
/// Panics for invalid rule date components or an Easter year outside 1–9999.
#[must_use]
pub fn business_day(date: Date, holidays: &[Holiday]) -> bool {
    weekday(date) && !holidays.iter().any(|holiday| holiday.contains(date))
}

/// Checks the date against a sorted slice of YYYYMMDD values.
///
/// # Panics
/// Panics if the year cannot be encoded as an `i32` YYYYMMDD value.
#[must_use]
pub fn dated_holiday(date: Date, dates: &[i32]) -> bool {
    let key = date.year() * 10_000
        + i32::try_from(date.month() * 100 + date.day())
            .unwrap_or_else(|_| unreachable!("month and day fit in i32"));
    dates.binary_search(&key).is_ok()
}

/// Trait defining the implementation interface for calendar operations.
pub trait ImplCalendar {
    /// Returns the name of the calendar implementation.
    fn impl_name(&self) -> String;

    /// Returns the set of holidays that have been added to this calendar.
    fn added_holidays(&self) -> HashSet<Date>;

    /// Checks if a given date is a business day according to the calendar's implementation.
    fn impl_is_business_day(&self, date: &Date) -> bool;

    /// Returns the set of holidays that have been removed from this calendar.
    fn removed_holidays(&self) -> HashSet<Date>;

    /// Adds a holiday to the calendar.
    fn add_holiday(&mut self, date: Date);

    /// Removes a holiday from the calendar.
    fn remove_holiday(&mut self, date: Date);

    /// Returns holidays between two dates, including both endpoints.
    fn holiday_list(&self, from: Date, to: Date, include_weekends: bool) -> Vec<Date> {
        let mut dates = Vec::new();
        let mut date = from;
        while date <= to {
            let holiday = self.added_holidays().contains(&date)
                || (!self.removed_holidays().contains(&date) && !self.impl_is_business_day(&date));
            if holiday && (include_weekends || !self.is_weekend(&date.weekday())) {
                dates.push(date);
            }
            if date == to {
                break;
            }
            date += 1;
        }
        dates
    }

    /// Returns business days between two dates, including both endpoints.
    fn business_day_list(&self, from: Date, to: Date) -> Vec<Date> {
        let holidays: HashSet<_> = self.holiday_list(from, to, true).into_iter().collect();
        let mut dates = Vec::new();
        let mut date = from;
        while date <= to {
            if !holidays.contains(&date) {
                dates.push(date);
            }
            if date == to {
                break;
            }
            date += 1;
        }
        dates
    }

    /// Checks if a given weekday is considered a weekend.
    fn is_weekend(&self, weekday: &Weekday) -> bool {
        weekday == &Weekday::Saturday || weekday == &Weekday::Sunday
    }

    /// Returns the day of year for Easter Monday in the given year.
    fn easter_monday(&self, year: i32) -> i32 {
        easter_monday(year)
    }
}

/// Trait providing the public calendar interface for business day calculations.
pub trait IsCalendar: ImplCalendar {
    /// Returns the name of the calendar.
    fn name(&self) -> String {
        self.impl_name()
    }

    /// Checks if a given date is a business day, considering added and removed holidays.
    fn is_business_day(&self, date: &Date) -> bool {
        if !self.added_holidays().is_empty() && self.added_holidays().contains(date) {
            return false;
        }
        if !self.removed_holidays().is_empty() && self.removed_holidays().contains(date) {
            return true;
        }
        self.impl_is_business_day(date)
    }

    /// Returns the last business day of the month containing the given date.
    fn end_of_month(&self, date: Date) -> Date {
        self.adjust(
            Date::end_of_month(date),
            Some(BusinessDayConvention::Preceding),
        )
    }

    /// Checks whether following adjustment of the next calendar day enters
    /// another month. This also includes non-business days after the month's
    /// last business day.
    fn is_end_of_month(&self, date: &Date) -> bool {
        let d1 = self.adjust(*date + 1, None);
        d1.month() != date.month()
    }

    /// Checks if a given date is a holiday.
    fn is_holiday(&self, date: &Date) -> bool {
        !self.is_business_day(date)
    }

    /// Counts the number of business days between two dates.
    ///
    /// Endpoint flags apply to `from` and `to` respectively. Reversing dates
    /// reverses the sign and swaps the endpoint flags. Equal endpoints count
    /// one only when both flags are true and the date is a business day.
    fn business_days_between(
        &self,
        from: Date,
        to: Date,
        include_first: bool,
        include_last: bool,
    ) -> i64 {
        match from.cmp(&to) {
            Ordering::Less => self.impl_days_between(from, to, include_first, include_last),
            Ordering::Greater => -self.impl_days_between(to, from, include_last, include_first),
            Ordering::Equal => {
                i64::from(include_first && include_last && self.is_business_day(&from))
            }
        }
    }

    /// Adjusts a date to a business day according to the given convention.
    ///
    /// `None` means Following. Nearest resolves equal-distance ties forward.
    ///
    /// # Panics
    /// Panics for an empty date.
    fn adjust(&self, date: Date, convention: Option<BusinessDayConvention>) -> Date {
        assert!(date != Date::empty(), "null date");

        let conv = convention.unwrap_or(BusinessDayConvention::Following);

        let mut d1 = date;
        match conv {
            BusinessDayConvention::Unadjusted => return date,
            BusinessDayConvention::Following
            | BusinessDayConvention::ModifiedFollowing
            | BusinessDayConvention::HalfMonthModifiedFollowing => {
                while self.is_holiday(&d1) {
                    d1 += 1;
                }
                if let BusinessDayConvention::ModifiedFollowing
                | BusinessDayConvention::HalfMonthModifiedFollowing = conv
                {
                    if d1.month() != date.month() {
                        return self.adjust(date, Some(BusinessDayConvention::Preceding));
                    }
                    if conv == BusinessDayConvention::HalfMonthModifiedFollowing
                        && date.day() <= 15
                        && d1.day() > 15
                    {
                        return self.adjust(date, Some(BusinessDayConvention::Preceding));
                    }
                }
            }
            BusinessDayConvention::Preceding | BusinessDayConvention::ModifiedPreceding => {
                while self.is_holiday(&d1) {
                    d1 -= 1;
                }
                if conv == BusinessDayConvention::ModifiedPreceding && d1.month() != date.month() {
                    return self.adjust(date, Some(BusinessDayConvention::Following));
                }
            }
            BusinessDayConvention::Nearest => {
                let mut d2 = date;
                while self.is_holiday(&d1) && self.is_holiday(&d2) {
                    d1 += 1;
                    d2 -= 1;
                }
                if self.is_holiday(&d1) {
                    return d2;
                }
                return d1;
            }
        }
        d1
    }

    /// Internal method to count business days between two dates.
    fn impl_days_between(
        &self,
        from: Date,
        to: Date,
        include_first: bool,
        include_last: bool,
    ) -> i64 {
        let mut res = i64::from(include_last && self.is_business_day(&to));
        let mut d = if include_first { from } else { from + 1 };
        while d < to {
            if self.is_business_day(&d) {
                res += 1;
            }
            d += 1;
        }
        res
    }

    /// Advances a date by a given period, adjusting for business days.
    ///
    /// Days count business days, unlike `Date` arithmetic; zero days leaves
    /// the date unchanged. Other units advance calendar periods and then apply
    /// the convention (`None` means Following), with optional month-end handling.
    ///
    /// # Panics
    /// Panics for an empty date.
    fn advance(
        &self,
        date: Date,
        period: Period,
        convention: Option<BusinessDayConvention>,
        end_of_month: bool,
    ) -> Date {
        assert!(date != Date::empty(), "null date");

        let mut d1 = date;
        match period.units() {
            TimeUnit::Days => {
                let mut n = period.length();
                if n > 0 {
                    while n > 0 {
                        d1 += 1;
                        while self.is_holiday(&d1) {
                            d1 += 1;
                        }
                        n -= 1;
                    }
                } else {
                    while n < 0 {
                        d1 -= 1;
                        while self.is_holiday(&d1) {
                            d1 -= 1;
                        }
                        n += 1;
                    }
                }
            }
            TimeUnit::Weeks => {
                d1 = d1 + period;
                d1 = self.adjust(d1, convention);
            }
            _ => {
                d1 = d1 + period;
                if end_of_month && self.is_end_of_month(&date) {
                    return self.end_of_month(d1);
                }
                d1 = self.adjust(d1, convention);
            }
        }
        d1
    }
}

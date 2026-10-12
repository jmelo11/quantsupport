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
#[must_use]
pub fn easter_monday(y: i32) -> i32 {
    let easter_monday = vec![
        98, 90, 103, 95, 114, 106, 91, 111, 102, // 1901-1909
        87, 107, 99, 83, 103, 95, 115, 99, 91, 111, // 1910-1919
        96, 87, 107, 92, 112, 103, 95, 108, 100, 91, // 1920-1929
        111, 96, 88, 107, 92, 112, 104, 88, 108, 100, // 1930-1939
        85, 104, 96, 116, 101, 92, 112, 97, 89, 108, // 1940-1949
        100, 85, 105, 96, 109, 101, 93, 112, 97, 89, // 1950-1959
        109, 93, 113, 105, 90, 109, 101, 86, 106, 97, // 1960-1969
        89, 102, 94, 113, 105, 90, 110, 101, 86, 106, // 1970-1979
        98, 110, 102, 94, 114, 98, 90, 110, 95, 86, // 1980-1989
        106, 91, 111, 102, 94, 107, 99, 90, 103, 95, // 1990-1999
        115, 106, 91, 111, 103, 87, 107, 99, 84, 103, // 2000-2009
        95, 115, 100, 91, 111, 96, 88, 107, 92, 112, // 2010-2019
        104, 95, 108, 100, 92, 111, 96, 88, 108, 92, // 2020-2029
        112, 104, 89, 108, 100, 85, 105, 96, 116, 101, // 2030-2039
        93, 112, 97, 89, 109, 100, 85, 105, 97, 109, // 2040-2049
        101, 93, 113, 97, 89, 109, 94, 113, 105, 90, // 2050-2059
        110, 101, 86, 106, 98, 89, 102, 94, 114, 105, // 2060-2069
        90, 110, 102, 86, 106, 98, 111, 102, 94, 114, // 2070-2079
        99, 90, 110, 95, 87, 106, 91, 111, 103, 94, // 2080-2089
        107, 99, 91, 103, 95, 115, 107, 91, 111, 103, // 2090-2099
        88, 108, 100, 85, 105, 96, 109, 101, 93, 112, // 2100-2109
        97, 89, 109, 93, 113, 105, 90, 109, 101, 86, // 2110-2119
        106, 97, 89, 102, 94, 113, 105, 90, 110, 101, // 2120-2129
        86, 106, 98, 110, 102, 94, 114, 98, 90, 110, // 2130-2139
        95, 86, 106, 91, 111, 102, 94, 107, 99, 90, // 2140-2149
        103, 95, 115, 106, 91, 111, 103, 87, 107, 99, // 2150-2159
        84, 103, 95, 115, 100, 91, 111, 96, 88, 107, // 2160-2169
        92, 112, 104, 95, 108, 100, 92, 111, 96, 88, // 2170-2179
        108, 92, 112, 104, 89, 108, 100, 85, 105, 96, // 2180-2189
        116, 101, 93, 112, 97, 89, 109, 100, 85, 105, // 2190-2199
    ];
    let index = usize::try_from(y - 1901).unwrap_or_else(|_| panic!("valid easter index"));
    easter_monday[index]
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

    /// Returns a list of holidays between two dates.
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

    /// Returns a list of business days between two dates.
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

    /// Checks if the given date is the last business day of its month.
    fn is_end_of_month(&self, date: &Date) -> bool {
        let d1 = self.adjust(*date + 1, None);
        d1.month() != date.month()
    }

    /// Checks if a given date is a holiday.
    fn is_holiday(&self, date: &Date) -> bool {
        !self.is_business_day(date)
    }

    /// Counts the number of business days between two dates.
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

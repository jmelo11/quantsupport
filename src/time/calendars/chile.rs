use std::collections::HashSet;

use chrono::{Datelike, NaiveDate, Weekday};

use super::traits::{easter_monday, ImplCalendar, IsCalendar};
use crate::time::date::Date;

/// # Chile
/// A calendar for Chile
///

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Market {
    /// Santiago Stock Exchange
    SSE,
}

/// Chile calendar for business day calculations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chile {
    market: Market,
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl Chile {
    /// Creates a new Chile calendar instance for the specified market.
    #[must_use]
    pub fn new(market: Market) -> Self {
        Self {
            market,
            added_holidays: HashSet::new(),
            removed_holidays: HashSet::new(),
        }
    }

    fn is_weekend(day: Weekday) -> bool {
        day == Weekday::Sat || day == Weekday::Sun
    }

    fn is_new_years_day(day: u32, month: u32, year: i32) -> bool {
        let w = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap_or_else(|| panic!("valid date for New Year's Day rules"))
            .weekday();
        (day == 1 && month == 1) || (day == 2 && month == 1 && w == Weekday::Mon && year >= 2016)
    }

    fn is_good_friday(day: u32, month: u32, year: i32) -> bool {
        let easter_friday = easter_monday(year) - 3;
        let dd = Date::new(year, month, day).day_of_year();
        dd == easter_friday
    }

    fn is_easter_saturday(day: u32, month: u32, year: i32) -> bool {
        let easter_saturday = easter_monday(year) - 2;
        let dd = Date::new(year, month, day).day_of_year();
        dd == easter_saturday
    }

    const fn is_labour_day(day: u32, month: u32) -> bool {
        day == 1 && month == 5
    }

    const fn is_navy_day(day: u32, month: u32) -> bool {
        day == 21 && month == 5
    }

    const fn is_aboriginal_peoples_day(day: u32, month: u32, year: i32) -> bool {
        day == 21 && month == 6 && year >= 2021
    }

    fn is_saint_peter_and_saint_paul_day(day: u32, month: u32) -> bool {
        let w = NaiveDate::from_ymd_opt(2001, month, day)
            .unwrap_or_else(|| panic!("valid date for Saint Peter and Saint Paul day rules"))
            .weekday();
        (26..=29).contains(&day) && month == 6 && w == Weekday::Mon
            || day == 2 && month == 7 && w == Weekday::Mon
    }

    const fn is_our_lady_of_mount_carmel_day(day: u32, month: u32) -> bool {
        day == 16 && month == 7
    }

    const fn is_assumption_day(day: u32, month: u32) -> bool {
        day == 15 && month == 8
    }

    fn is_independence_day(day: u32, month: u32, year: i32) -> bool {
        let w = NaiveDate::from_ymd_opt(1810, month, day)
            .unwrap_or_else(|| panic!("valid date for Independence Day rules"))
            .weekday();
        (day == 17
            && month == 9
            && ((w == Weekday::Mon && year >= 2007) || (w == Weekday::Fri && year >= 2016)))
            || (day == 18 && month == 9)
    }

    fn is_army_day(day: u32, month: u32, year: i32) -> bool {
        let w = NaiveDate::from_ymd_opt(1810, month, day)
            .unwrap_or_else(|| panic!("valid date for Army Day rules"))
            .weekday();
        (day == 19 && month == 9) || (day == 20 && month == 9 && w == Weekday::Fri && year >= 2007)
    }

    fn is_discovery_of_two_worlds(day: u32, month: u32) -> bool {
        let w = NaiveDate::from_ymd_opt(1492, month, day)
            .unwrap_or_else(|| panic!("valid date for Discovery of Two Worlds rules"))
            .weekday();
        !(month != 10 || w != Weekday::Mon || !(9..=12).contains(&day) && day != 15)
    }

    fn is_reformation_day(day: u32, month: u32, year: i32) -> bool {
        let w = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap_or_else(|| panic!("valid date for Reformation Day rules"))
            .weekday();
        ((day == 27 && month == 10 && w == Weekday::Fri)
            || (day == 31 && month == 10 && w != Weekday::Tue && w != Weekday::Wed)
            || (day == 2 && month == 11 && w == Weekday::Fri))
            && year >= 2008
    }

    const fn is_all_saints_day(day: u32, month: u32) -> bool {
        day == 1 && month == 11
    }

    const fn is_immaculate_conception(day: u32, month: u32) -> bool {
        day == 8 && month == 12
    }

    const fn is_christmas_day(day: u32, month: u32) -> bool {
        day == 25 && month == 12
    }

    const fn is_bank_holiday(day: u32, month: u32) -> bool {
        day == 31 && month == 12
    }

    /// Determines if a given date is a business day in the Chilean market.
    #[must_use]
    pub fn is_business_day(&self, date: NaiveDate) -> bool {
        let weekday = date.weekday();
        let day = date.day();
        let month = date.month();
        let year = date.year();
        if Self::is_weekend(weekday) {
            return false;
        }

        match self.market {
            Market::SSE => {
                !(Self::is_new_years_day(day, month, year)
                    || Self::is_good_friday(day, month, year)
                    || Self::is_easter_saturday(day, month, year)
                    || Self::is_labour_day(day, month)
                    || Self::is_navy_day(day, month)
                    || Self::is_aboriginal_peoples_day(day, month, year)
                    || Self::is_saint_peter_and_saint_paul_day(day, month)
                    || Self::is_our_lady_of_mount_carmel_day(day, month)
                    || Self::is_assumption_day(day, month)
                    || Self::is_independence_day(day, month, year)
                    || Self::is_army_day(day, month, year)
                    || Self::is_discovery_of_two_worlds(day, month)
                    || Self::is_reformation_day(day, month, year)
                    || Self::is_all_saints_day(day, month)
                    || Self::is_immaculate_conception(day, month)
                    || Self::is_christmas_day(day, month)
                    || Self::is_bank_holiday(day, month))
            }
        }
    }
}

impl ImplCalendar for Chile {
    fn impl_is_business_day(&self, date: &Date) -> bool {
        self.is_business_day(date.base_date())
    }

    fn impl_name(&self) -> String {
        format!("Chile({:?})", self.market)
    }

    fn added_holidays(&self) -> HashSet<Date> {
        self.added_holidays.clone()
    }

    fn removed_holidays(&self) -> HashSet<Date> {
        self.removed_holidays.clone()
    }

    fn add_holiday(&mut self, date: Date) {
        self.added_holidays.insert(date);
    }

    fn remove_holiday(&mut self, date: Date) {
        self.removed_holidays.insert(date);
    }

    fn holiday_list(&self, from: Date, to: Date, include_weekends: bool) -> Vec<Date> {
        let mut holidays = vec![];
        let mut d = from;
        while d <= to {
            if self.is_holiday(&d) {
                holidays.push(d);
            }
            d += 1;
        }
        if include_weekends {
            holidays
        } else {
            holidays
                .into_iter()
                .filter(|d| !self.is_weekend(&d.weekday()))
                .collect()
        }
    }

    fn business_day_list(&self, from: Date, to: Date) -> Vec<Date> {
        let mut business_days = vec![];
        let mut d = from;
        while d <= to {
            if self.is_business_day(d.base_date()) {
                business_days.push(d);
            }
            d += 1;
        }
        business_days
    }
}

impl IsCalendar for Chile {}

impl Default for Chile {
    fn default() -> Self {
        Self::new(Market::SSE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::date::Date;

    #[test]
    fn test_chile_settlement() {
        let cal = Chile::new(Market::SSE);
        let expected_hol = vec![
            Date::new(2024, 1, 1),
            Date::new(2024, 3, 29),
            Date::new(2024, 5, 1),
            Date::new(2024, 5, 21),
            Date::new(2024, 6, 21),
            Date::new(2024, 7, 16),
            Date::new(2024, 8, 15),
            Date::new(2024, 9, 18),
            Date::new(2024, 9, 19),
            Date::new(2024, 10, 31),
            Date::new(2024, 11, 1),
            Date::new(2024, 12, 25),
            Date::new(2024, 12, 31),
        ];
        for d in expected_hol {
            assert!(!cal.is_business_day(d.base_date()));
        }
    }
}

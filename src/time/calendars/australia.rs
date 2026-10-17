// Adapted from QuantLib ql/time/calendars/australia.cpp at 5f4811e7e03bbd1a3d6d7df31b751fecd5daef5f.
// Copyright (C) 2000, 2001, 2002, 2003 RiskMap srl
// Distributed under the QuantLib license; see LICENSE-QuantLib.txt.

use super::traits::{ImplCalendar, IsCalendar};
use crate::time::{date::Date, enums::Weekday};
use std::collections::HashSet;

/// Calendar market or regional variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Market {
    /// Settlement calendar.
    #[default]
    Settlement,
    /// ASX calendar.
    ASX,
}

/// Australia business-day calendar, following `QuantLib` 5f4811e7e03b.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Australia {
    market: Market,
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl Australia {
    /// Creates the calendar with empty holiday overrides.
    #[must_use]
    pub fn new(market: Market) -> Self {
        Self {
            market,
            ..Self::default()
        }
    }

    fn settlement_business_day(date: Date) -> bool {
        let w = date.weekday();
        let d = date.day();
        let m = date.month();
        let y = date.year();
        let dd = date.day_of_year();
        let em = super::traits::easter_monday(date.year());
        !(matches!(w, Weekday::Saturday | Weekday::Sunday)
            || ((d == 1 || ((d == 2 || d == 3) && w == Weekday::Monday)) && m == 1)
            || ((d == 26 || ((d == 27 || d == 28) && w == Weekday::Monday)) && m == 1)
            || (dd == em - 3)
            || (dd == em)
            || (d == 25 && m == 4)
            || ((d > 7 && d <= 14) && w == Weekday::Monday && m == 6)
            || (d <= 7 && w == Weekday::Monday && m == 8)
            || (d <= 7 && w == Weekday::Monday && m == 10)
            || ((d == 25 || (d == 27 && (w == Weekday::Monday || w == Weekday::Tuesday)))
                && m == 12)
            || ((d == 26 || (d == 28 && (w == Weekday::Monday || w == Weekday::Tuesday)))
                && m == 12)
            || (d == 22 && m == 9 && y == 2022))
    }
    fn asx_business_day(date: Date) -> bool {
        let w = date.weekday();
        let d = date.day();
        let m = date.month();
        let y = date.year();
        let dd = date.day_of_year();
        let em = super::traits::easter_monday(date.year());
        !(matches!(w, Weekday::Saturday | Weekday::Sunday)
            || ((d == 1 || ((d == 2 || d == 3) && w == Weekday::Monday)) && m == 1)
            || ((d == 26 || ((d == 27 || d == 28) && w == Weekday::Monday)) && m == 1)
            || (dd == em - 3)
            || (dd == em)
            || (d == 25 && m == 4)
            || ((d > 7 && d <= 14) && w == Weekday::Monday && m == 6)
            || ((d == 25 || (d == 27 && (w == Weekday::Monday || w == Weekday::Tuesday)))
                && m == 12)
            || ((d == 26 || (d == 28 && (w == Weekday::Monday || w == Weekday::Tuesday)))
                && m == 12)
            || (d == 22 && m == 9 && y == 2022))
    }
}

impl ImplCalendar for Australia {
    fn impl_name(&self) -> String {
        format!("Australia({:?})", self.market)
    }
    fn impl_is_business_day(&self, date: &Date) -> bool {
        match self.market {
            Market::Settlement => Self::settlement_business_day(*date),
            Market::ASX => Self::asx_business_day(*date),
        }
    }
    fn added_holidays(&self) -> HashSet<Date> {
        self.added_holidays.clone()
    }
    fn removed_holidays(&self) -> HashSet<Date> {
        self.removed_holidays.clone()
    }
    fn add_holiday(&mut self, date: Date) {
        self.removed_holidays.remove(&date);
        self.added_holidays.insert(date);
    }
    fn remove_holiday(&mut self, date: Date) {
        self.added_holidays.remove(&date);
        self.removed_holidays.insert(date);
    }
}
impl IsCalendar for Australia {}

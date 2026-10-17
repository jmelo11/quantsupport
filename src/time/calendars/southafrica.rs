// Adapted from QuantLib ql/time/calendars/southafrica.cpp at 5f4811e7e03bbd1a3d6d7df31b751fecd5daef5f.
// Copyright (C) 2000, 2001, 2002, 2003 RiskMap srl
// Copyright (C) 2023, 2024 Skandinaviska Enskilda Banken AB (publ)
// Distributed under the QuantLib license; see LICENSE-QuantLib.txt.

use super::traits::{ImplCalendar, IsCalendar};
use crate::time::{date::Date, enums::Weekday};
use std::collections::HashSet;

/// `SouthAfrica` business-day calendar, following `QuantLib` 5f4811e7e03b.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SouthAfrica {
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl SouthAfrica {
    /// Creates the calendar with empty holiday overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn business_day(date: Date) -> bool {
        let w = date.weekday();
        let d = date.day();
        let m = date.month();
        let y = date.year();
        let dd = date.day_of_year();
        let em = super::traits::easter_monday(date.year());
        !(matches!(w, Weekday::Saturday | Weekday::Sunday)
            || ((d == 1 || (d == 2 && w == Weekday::Monday)) && m == 1)
            || (dd == em - 3)
            || (dd == em)
            || ((d == 21 || (d == 22 && w == Weekday::Monday)) && m == 3)
            || ((d == 27 || (d == 28 && w == Weekday::Monday)) && m == 4)
            || ((d == 1 || (d == 2 && w == Weekday::Monday)) && m == 5)
            || ((d == 16 || (d == 17 && w == Weekday::Monday)) && m == 6)
            || ((d == 9 || (d == 10 && w == Weekday::Monday)) && m == 8)
            || ((d == 24 || (d == 25 && w == Weekday::Monday)) && m == 9)
            || ((d == 16 || (d == 17 && w == Weekday::Monday)) && m == 12)
            || (d == 25 && m == 12)
            || ((d == 26 || (d == 27 && w == Weekday::Monday)) && m == 12)
            || (d == 14 && m == 4 && y == 2004)
            || (d == 22 && m == 4 && y == 2009)
            || (d == 3 && m == 8 && y == 2016)
            || (d == 1 && m == 11 && y == 2021)
            || (d == 29 && m == 5 && y == 2024)
            || (d == 4 && m == 11 && y == 2026)
            || (d == 27 && m == 12 && y == 2022)
            || (d == 15 && m == 12 && y == 2023))
    }
}

impl ImplCalendar for SouthAfrica {
    fn impl_name(&self) -> String {
        "SouthAfrica".into()
    }
    fn impl_is_business_day(&self, date: &Date) -> bool {
        Self::business_day(*date)
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
impl IsCalendar for SouthAfrica {}

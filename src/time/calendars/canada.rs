// Adapted from QuantLib ql/time/calendars/canada.cpp.

use super::traits::{ImplCalendar, IsCalendar};
use crate::time::{date::Date, enums::Weekday};
use std::collections::HashSet;

/// Calendar market or regional variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Market {
    /// Settlement calendar.
    #[default]
    Settlement,
    /// TSX calendar.
    TSX,
}

/// Canada business-day calendar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Canada {
    market: Market,
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl Canada {
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
            || (((15..=21).contains(&d)) && w == Weekday::Monday && m == 2 && y >= 2008)
            || (dd == em - 3)
            || (d > 17 && d <= 24 && w == Weekday::Monday && m == 5)
            || ((d == 1 || ((d == 2 || d == 3) && w == Weekday::Monday)) && m == 7)
            || (d <= 7 && w == Weekday::Monday && m == 8)
            || (d <= 7 && w == Weekday::Monday && m == 9)
            || (((d == 30 && m == 9) || (d <= 2 && m == 10 && w == Weekday::Monday)) && y >= 2021)
            || (d > 7 && d <= 14 && w == Weekday::Monday && m == 10)
            || ((d == 11 || ((d == 12 || d == 13) && w == Weekday::Monday)) && m == 11)
            || ((d == 25 || (d == 27 && (w == Weekday::Monday || w == Weekday::Tuesday)))
                && m == 12)
            || ((d == 26 || (d == 28 && (w == Weekday::Monday || w == Weekday::Tuesday)))
                && m == 12))
    }
    fn tsx_business_day(date: Date) -> bool {
        let w = date.weekday();
        let d = date.day();
        let m = date.month();
        let y = date.year();
        let dd = date.day_of_year();
        let em = super::traits::easter_monday(date.year());
        !(matches!(w, Weekday::Saturday | Weekday::Sunday)
            || ((d == 1 || ((d == 2 || d == 3) && w == Weekday::Monday)) && m == 1)
            || (((15..=21).contains(&d)) && w == Weekday::Monday && m == 2 && y >= 2008)
            || (dd == em - 3)
            || (d > 17 && d <= 24 && w == Weekday::Monday && m == 5)
            || ((d == 1 || ((d == 2 || d == 3) && w == Weekday::Monday)) && m == 7)
            || (d <= 7 && w == Weekday::Monday && m == 8)
            || (d <= 7 && w == Weekday::Monday && m == 9)
            || (d > 7 && d <= 14 && w == Weekday::Monday && m == 10)
            || ((d == 25 || (d == 27 && (w == Weekday::Monday || w == Weekday::Tuesday)))
                && m == 12)
            || ((d == 26 || (d == 28 && (w == Weekday::Monday || w == Weekday::Tuesday)))
                && m == 12))
    }
}

impl ImplCalendar for Canada {
    fn impl_name(&self) -> String {
        format!("Canada({:?})", self.market)
    }
    fn impl_is_business_day(&self, date: &Date) -> bool {
        match self.market {
            Market::Settlement => Self::settlement_business_day(*date),
            Market::TSX => Self::tsx_business_day(*date),
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
impl IsCalendar for Canada {}

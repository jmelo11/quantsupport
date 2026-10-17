//! Australia holiday calendar.

use super::traits::{
    business_day,
    Holiday::{Easter, Fixed, WeekdayOnOrAfter},
    Observance::{Exact, NextMonday, Paired},
};
use super::traits::{ImplCalendar, IsCalendar};
use crate::time::{date::Date, enums::Weekday};
use std::collections::HashSet;

/// Market or regional calendar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Market {
    /// Settlement calendar.
    #[default]
    Settlement,
    /// ASX calendar.
    ASX,
}

/// Australia business-day calendar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Australia {
    market: Market,
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl Australia {
    /// Creates a calendar with empty holiday overrides.
    #[must_use]
    pub fn new(market: Market) -> Self {
        Self {
            market,
            ..Self::default()
        }
    }
}

impl ImplCalendar for Australia {
    fn impl_name(&self) -> String {
        format!("Australia({:?})", self.market)
    }

    fn impl_is_business_day(&self, date: &Date) -> bool {
        let holidays = [
            Fixed(1, 1, NextMonday),  // New Year
            Fixed(1, 26, NextMonday), // Australia Day
            Easter(-2),
            Easter(1),
            Fixed(4, 25, Exact),                     // ANZAC Day
            WeekdayOnOrAfter(6, 8, Weekday::Monday), // King's Birthday
            Fixed(12, 25, Paired),
            Fixed(12, 26, Paired),
        ];
        let bank_only = [
            WeekdayOnOrAfter(8, 1, Weekday::Monday),
            WeekdayOnOrAfter(10, 1, Weekday::Monday),
        ];
        business_day(*date, &holidays)
            && !(self.market == Market::Settlement
                && bank_only.iter().any(|rule| rule.contains(*date)))
            && *date != Date::new(2022, 9, 22)
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

//! Canada holiday calendar.

use super::traits::{
    business_day,
    Holiday::{Easter, Fixed, WeekdayOnOrAfter},
    Observance::{NextMonday, Paired},
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
    /// Creates a calendar with empty holiday overrides.
    #[must_use]
    pub fn new(market: Market) -> Self {
        Self {
            market,
            ..Self::default()
        }
    }
}

impl ImplCalendar for Canada {
    fn impl_name(&self) -> String {
        format!("Canada({:?})", self.market)
    }

    fn impl_is_business_day(&self, date: &Date) -> bool {
        let holidays = [
            Fixed(1, 1, NextMonday),
            Easter(-2),
            WeekdayOnOrAfter(5, 18, Weekday::Monday), // Victoria Day
            Fixed(7, 1, NextMonday),
            WeekdayOnOrAfter(8, 1, Weekday::Monday), // Civic Holiday
            WeekdayOnOrAfter(9, 1, Weekday::Monday), // Labour Day
            WeekdayOnOrAfter(10, 8, Weekday::Monday), // Thanksgiving
            Fixed(12, 25, Paired),
            Fixed(12, 26, Paired),
        ];
        let family_day = WeekdayOnOrAfter(2, 15, Weekday::Monday);
        let settlement_holiday = Fixed(11, 11, NextMonday).contains(*date)
            || (date.year() >= 2021 && Fixed(9, 30, NextMonday).contains(*date));
        business_day(*date, &holidays)
            && !(date.year() >= 2008 && family_day.contains(*date))
            && !(self.market == Market::Settlement && settlement_holiday)
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

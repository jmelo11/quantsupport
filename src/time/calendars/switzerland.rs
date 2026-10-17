// Adapted from QuantLib ql/time/calendars/switzerland.cpp.

use super::traits::{ImplCalendar, IsCalendar};
use crate::time::{date::Date, enums::Weekday};
use std::collections::HashSet;

/// Switzerland business-day calendar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Switzerland {
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl Switzerland {
    /// Creates the calendar with empty holiday overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn business_day(date: Date) -> bool {
        let w = date.weekday();
        let d = date.day();
        let m = date.month();
        let dd = date.day_of_year();
        let em = super::traits::easter_monday(date.year());
        !(matches!(w, Weekday::Saturday | Weekday::Sunday)
            || (d == 1 && m == 1)
            || (d == 2 && m == 1)
            || (dd == em - 3)
            || (dd == em)
            || (dd == em + 38)
            || (dd == em + 49)
            || (d == 1 && m == 5)
            || (d == 1 && m == 8)
            || (d == 25 && m == 12)
            || (d == 26 && m == 12))
    }
}

impl ImplCalendar for Switzerland {
    fn impl_name(&self) -> String {
        "Switzerland".into()
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
impl IsCalendar for Switzerland {}

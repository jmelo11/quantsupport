// Adapted from QuantLib ql/time/calendars/norway.cpp.

use super::traits::{ImplCalendar, IsCalendar};
use crate::time::{date::Date, enums::Weekday};
use std::collections::HashSet;

/// Norway business-day calendar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Norway {
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl Norway {
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
            || (dd == em - 4)
            || (dd == em - 3)
            || (dd == em)
            || (dd == em + 38)
            || (dd == em + 49)
            || (d == 1 && m == 1)
            || (d == 1 && m == 5)
            || (d == 17 && m == 5)
            || (d == 24 && m == 12 && y >= 2002)
            || (d == 25 && m == 12)
            || (d == 26 && m == 12))
    }
}

impl ImplCalendar for Norway {
    fn impl_name(&self) -> String {
        "Norway".into()
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
impl IsCalendar for Norway {}

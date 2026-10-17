//! Denmark holiday calendar.

use super::traits::{
    business_day,
    Holiday::{Easter, Fixed},
    Observance::Exact,
};
use super::traits::{ImplCalendar, IsCalendar};
use crate::time::date::Date;
use std::collections::HashSet;

/// Denmark business-day calendar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Denmark {
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl Denmark {
    /// Creates a calendar with empty holiday overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ImplCalendar for Denmark {
    fn impl_name(&self) -> String {
        "Denmark".into()
    }

    fn impl_is_business_day(&self, date: &Date) -> bool {
        business_day(
            *date,
            &[
                Fixed(1, 1, Exact),
                Fixed(6, 5, Exact),
                Easter(-3),
                Easter(-2),
                Easter(1),
                Easter(39),
                Easter(50),
                Fixed(12, 24, Exact),
                Fixed(12, 25, Exact),
                Fixed(12, 26, Exact),
                Fixed(12, 31, Exact),
            ],
        ) && !(date.year() <= 2023 && Easter(26).contains(*date))
            && !(date.year() >= 2009 && Easter(40).contains(*date))
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
impl IsCalendar for Denmark {}

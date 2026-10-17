//! Norway holiday calendar.

use super::traits::{
    business_day,
    Holiday::{Easter, Fixed},
    Observance::Exact,
};
use super::traits::{ImplCalendar, IsCalendar};
use crate::time::date::Date;
use std::collections::HashSet;

/// Norway business-day calendar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Norway {
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl Norway {
    /// Creates a calendar with empty holiday overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ImplCalendar for Norway {
    fn impl_name(&self) -> String {
        "Norway".into()
    }

    fn impl_is_business_day(&self, date: &Date) -> bool {
        business_day(
            *date,
            &[
                Fixed(1, 1, Exact),
                Fixed(5, 1, Exact),
                Fixed(5, 17, Exact),
                Easter(-3),
                Easter(-2),
                Easter(1),
                Easter(39),
                Easter(50),
                Fixed(12, 25, Exact),
                Fixed(12, 26, Exact),
            ],
        ) && !(date.year() >= 2002 && Fixed(12, 24, Exact).contains(*date))
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

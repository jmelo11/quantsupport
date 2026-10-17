//! Sweden holiday calendar.

use super::traits::{
    business_day,
    Holiday::{Easter, Fixed, WeekdayOnOrAfter},
    Observance::Exact,
};
use super::traits::{ImplCalendar, IsCalendar};
use crate::time::{date::Date, enums::Weekday};
use std::collections::HashSet;

/// Sweden business-day calendar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sweden {
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl Sweden {
    /// Creates a calendar with empty holiday overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ImplCalendar for Sweden {
    fn impl_name(&self) -> String {
        "Sweden".into()
    }

    fn impl_is_business_day(&self, date: &Date) -> bool {
        business_day(
            *date,
            &[
                Fixed(1, 1, Exact),
                Fixed(1, 6, Exact),
                Fixed(5, 1, Exact),
                Easter(-2),
                Easter(1),
                Easter(39),
                WeekdayOnOrAfter(6, 19, Weekday::Friday), // Midsummer Eve
                Fixed(12, 24, Exact),
                Fixed(12, 25, Exact),
                Fixed(12, 26, Exact),
                Fixed(12, 31, Exact),
            ],
        ) && !if date.year() >= 2005 {
            Fixed(6, 6, Exact).contains(*date)
        } else {
            Easter(50).contains(*date)
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
impl IsCalendar for Sweden {}

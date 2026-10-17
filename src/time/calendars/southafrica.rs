//! `SouthAfrica` holiday calendar.

use super::traits::{
    business_day,
    Holiday::{Easter, Fixed},
    Observance::SundayToMonday,
};
use super::traits::{ImplCalendar, IsCalendar};
use crate::time::date::Date;
use std::collections::HashSet;

/// `SouthAfrica` business-day calendar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SouthAfrica {
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl SouthAfrica {
    /// Creates a calendar with empty holiday overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ImplCalendar for SouthAfrica {
    fn impl_name(&self) -> String {
        "SouthAfrica".into()
    }

    fn impl_is_business_day(&self, date: &Date) -> bool {
        business_day(
            *date,
            &[
                Fixed(1, 1, SundayToMonday),
                Fixed(3, 21, SundayToMonday),
                Fixed(4, 27, SundayToMonday),
                Fixed(5, 1, SundayToMonday),
                Fixed(6, 16, SundayToMonday),
                Fixed(8, 9, SundayToMonday),
                Fixed(9, 24, SundayToMonday),
                Fixed(12, 16, SundayToMonday),
                Fixed(12, 25, SundayToMonday),
                Fixed(12, 26, SundayToMonday),
                Easter(-2),
                Easter(1),
            ],
        ) && !super::traits::dated_holiday(*date, SOUTH_AFRICA_SPECIAL)
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

// Dated data from holidays 0.106 (MIT): https://github.com/vacanza/holidays/tree/v0.106
// Upstream copyright and permission notice are retained in LICENSE.
#[rustfmt::skip]
const SOUTH_AFRICA_SPECIAL: &[i32] = &[
    19990602, 19991231,
    20000102,
    20040414,
    20060301,
    20080502,
    20090422,
    20110518, 20111227,
    20140507,
    20160803, 20161227,
    20190508,
    20211101,
    20221227,
    20231215,
    20240529,
    20261104,
];

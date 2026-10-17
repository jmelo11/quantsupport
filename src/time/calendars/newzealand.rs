//! `NewZealand` holiday calendar.

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
    /// Wellington calendar.
    #[default]
    Wellington,
    /// Auckland calendar.
    Auckland,
}

/// `NewZealand` business-day calendar.
/// Matariki dates are published through 2052; later dates require new announcements.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewZealand {
    market: Market,
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl NewZealand {
    /// Creates a calendar with empty holiday overrides.
    #[must_use]
    pub fn new(market: Market) -> Self {
        Self {
            market,
            ..Self::default()
        }
    }
}

impl ImplCalendar for NewZealand {
    fn impl_name(&self) -> String {
        format!("NewZealand({:?})", self.market)
    }

    fn impl_is_business_day(&self, date: &Date) -> bool {
        let holidays = [
            Fixed(1, 1, Paired),
            Fixed(1, 2, Paired),
            Easter(-2),
            Easter(1),
            WeekdayOnOrAfter(6, 1, Weekday::Monday),
            WeekdayOnOrAfter(10, 22, Weekday::Monday),
            Fixed(12, 25, Paired),
            Fixed(12, 26, Paired),
        ];
        let observance = if date.year() >= 2014 {
            NextMonday
        } else {
            Exact
        };
        let anniversary = match self.market {
            Market::Wellington => WeekdayOnOrAfter(1, 19, Weekday::Monday),
            Market::Auckland => WeekdayOnOrAfter(1, 26, Weekday::Monday),
        };
        business_day(*date, &holidays)
            && !Fixed(2, 6, observance).contains(*date)
            && !Fixed(4, 25, observance).contains(*date)
            && !anniversary.contains(*date)
            && !super::traits::dated_holiday(*date, MATARIKI)
            && *date != Date::new(2022, 9, 26)
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
impl IsCalendar for NewZealand {}

// Published Matariki dates: https://www.tepapa.govt.nz/digital-museum/explore-digital-museum/matariki-maori-new-year/dates-for-matariki-public-holiday
#[rustfmt::skip]
const MATARIKI: &[i32] = &[
    20220624,
    20230714,
    20240628,
    20250620,
    20260710,
    20270625,
    20280714,
    20290706,
    20300621,
    20310711,
    20320702,
    20330624,
    20340707,
    20350629,
    20360718,
    20370710,
    20380625,
    20390715,
    20400706,
    20410719,
    20420711,
    20430703,
    20440624,
    20450707,
    20460629,
    20470719,
    20480703,
    20490625,
    20500715,
    20510630,
    20520621,
];

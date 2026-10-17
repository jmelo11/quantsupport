// Adapted from QuantLib ql/time/calendars/newzealand.cpp.

use super::traits::{ImplCalendar, IsCalendar};
use crate::time::{date::Date, enums::Weekday};
use std::collections::HashSet;

/// Calendar market or regional variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Market {
    /// Wellington calendar.
    #[default]
    Wellington,
    /// Auckland calendar.
    Auckland,
}

/// `NewZealand` business-day calendar.
/// Matariki dates are tabulated through 2052; later dates require updated announcements.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewZealand {
    market: Market,
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl NewZealand {
    /// Creates the calendar with empty holiday overrides.
    #[must_use]
    pub fn new(market: Market) -> Self {
        Self {
            market,
            ..Self::default()
        }
    }

    fn common_business_day(date: Date) -> bool {
        let w = date.weekday();
        let d = date.day();
        let m = date.month();
        let y = date.year();
        let dd = date.day_of_year();
        let em = super::traits::easter_monday(date.year());
        !(matches!(w, Weekday::Saturday | Weekday::Sunday)
            || ((d == 1 || (d == 3 && (w == Weekday::Monday || w == Weekday::Tuesday))) && m == 1)
            || ((d == 2 || (d == 4 && (w == Weekday::Monday || w == Weekday::Tuesday))) && m == 1)
            || (d == 6 && m == 2)
            || ((d == 7 || d == 8) && w == Weekday::Monday && m == 2 && y > 2013)
            || (dd == em - 3)
            || (dd == em)
            || (d == 25 && m == 4)
            || ((d == 26 || d == 27) && w == Weekday::Monday && m == 4 && y > 2013)
            || (d <= 7 && w == Weekday::Monday && m == 6)
            || (((22..=28).contains(&d)) && w == Weekday::Monday && m == 10)
            || ((d == 25 || (d == 27 && (w == Weekday::Monday || w == Weekday::Tuesday)))
                && m == 12)
            || ((d == 26 || (d == 28 && (w == Weekday::Monday || w == Weekday::Tuesday)))
                && m == 12)
            || (d == 20 && m == 6 && y == 2025)
            || (d == 21 && m == 6 && (y == 2030 || y == 2052))
            || (d == 24 && m == 6 && (y == 2022 || y == 2033 || y == 2044))
            || (d == 25 && m == 6 && (y == 2027 || y == 2038 || y == 2049))
            || (d == 28 && m == 6 && y == 2024)
            || (d == 29 && m == 6 && (y == 2035 || y == 2046))
            || (d == 30 && m == 6 && y == 2051)
            || (d == 2 && m == 7 && y == 2032)
            || (d == 3 && m == 7 && (y == 2043 || y == 2048))
            || (d == 6 && m == 7 && (y == 2029 || y == 2040))
            || (d == 7 && m == 7 && (y == 2034 || y == 2045))
            || (d == 10 && m == 7 && (y == 2026 || y == 2037))
            || (d == 11 && m == 7 && (y == 2031 || y == 2042))
            || (d == 14 && m == 7 && (y == 2023 || y == 2028))
            || (d == 15 && m == 7 && (y == 2039 || y == 2050))
            || (d == 18 && m == 7 && y == 2036)
            || (d == 19 && m == 7 && (y == 2041 || y == 2047))
            || (d == 26 && m == 9 && y == 2022))
    }
    fn wellington_business_day(date: Date) -> bool {
        let w = date.weekday();
        let d = date.day();
        let m = date.month();
        Self::common_business_day(date)
            && !(((19..=25).contains(&d)) && w == Weekday::Monday && m == 1)
    }
    fn auckland_business_day(date: Date) -> bool {
        let w = date.weekday();
        let d = date.day();
        let m = date.month();
        Self::common_business_day(date)
            && !((d >= 26 && w == Weekday::Monday && m == 1)
                || (d == 1 && w == Weekday::Monday && m == 2))
    }
}

impl ImplCalendar for NewZealand {
    fn impl_name(&self) -> String {
        format!("NewZealand({:?})", self.market)
    }
    fn impl_is_business_day(&self, date: &Date) -> bool {
        match self.market {
            Market::Wellington => Self::wellington_business_day(*date),
            Market::Auckland => Self::auckland_business_day(*date),
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
impl IsCalendar for NewZealand {}

use std::collections::HashSet;

use chrono::{Datelike, NaiveDate, Weekday};

use crate::time::date::Date;

use super::traits::{ImplCalendar, IsCalendar};

/// # Market
/// Defines the relevant market for the United States calendar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Market {
    /// Settlement market.
    Settlement,
    /// Libor Impact market.
    LiborImpact,
    /// New York Stock Exchange market.
    Nyse,
    /// Government Bond market.
    GovernmentBond,
    /// North American Electric Reliability Corporation market.
    Nerc,
    /// Federal Reserve market.
    FederalReserve,
    /// Secured Overnight Financing Rate market.
    Sofr,
}

/// # `UnitedStates`
/// A calendar for the United States.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitedStates {
    market: Market,
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl UnitedStates {
    /// Creates a new `UnitedStates` calendar for the specified market.
    #[must_use]
    pub fn new(market: Market) -> Self {
        Self {
            market,
            added_holidays: HashSet::new(),
            removed_holidays: HashSet::new(),
        }
    }

    fn is_weekend(day: Weekday) -> bool {
        day == Weekday::Sat || day == Weekday::Sun
    }

    fn is_washington_birthday(day: u32, month: u32, year: i32, weekday: Weekday) -> bool {
        match year {
            y if y >= 1971 => (15..=21).contains(&day) && weekday == Weekday::Mon && month == 2,
            _ => {
                (day == 22
                    || (day == 23 && weekday == Weekday::Mon)
                    || (day == 21 && weekday == Weekday::Fri))
                    && month == 2
            }
        }
    }

    fn is_memorial_day(day: u32, month: u32, year: i32, weekday: Weekday) -> bool {
        match year {
            y if y >= 1971 => day >= 25 && weekday == Weekday::Mon && month == 5,
            _ => {
                (day == 30
                    || (day == 31 && weekday == Weekday::Mon)
                    || (day == 29 && weekday == Weekday::Fri))
                    && month == 5
            }
        }
    }

    fn is_independence_day(day: u32, month: u32, weekday: Weekday) -> bool {
        (day == 4 || (day == 5 && weekday == Weekday::Mon) || (day == 3 && weekday == Weekday::Fri))
            && month == 7
    }

    fn is_thanksgiving(day: u32, month: u32, weekday: Weekday) -> bool {
        (22..=28).contains(&day) && weekday == Weekday::Thu && month == 11
    }

    fn is_christmas(day: u32, month: u32, weekday: Weekday) -> bool {
        (day == 25
            || (day == 26 && weekday == Weekday::Mon)
            || (day == 24 && weekday == Weekday::Fri))
            && month == 12
    }

    /// Determines if the given date is a business day for this calendar's market.
    #[must_use]
    pub fn is_business_day(&self, date: NaiveDate) -> bool {
        let weekday = date.weekday();
        let day = date.day();
        let month = date.month();
        let year = date.year();

        if Self::is_weekend(weekday) {
            return false;
        }

        if self.market == Market::Sofr {
            let extra_holiday = ((day == 1 || (day == 2 && weekday == Weekday::Mon)) && month == 1)
                || ((15..=21).contains(&day)
                    && weekday == Weekday::Mon
                    && month == 1
                    && year >= 1983)
                || i64::from(date.ordinal()) == i64::from(super::traits::easter_monday(year) - 3)
                || ((day == 19
                    || (day == 20 && weekday == Weekday::Mon)
                    || (day == 18 && weekday == Weekday::Fri))
                    && month == 6
                    && year >= 2022)
                || (day <= 7 && weekday == Weekday::Mon && month == 9)
                || ((8..=14).contains(&day)
                    && weekday == Weekday::Mon
                    && month == 10
                    && year >= 1971)
                || (if year <= 1970 || year >= 1978 {
                    (day == 11 || (day == 12 && weekday == Weekday::Mon)) && month == 11
                } else {
                    (22..=28).contains(&day) && weekday == Weekday::Mon && month == 10
                })
                || matches!(
                    (year, month, day),
                    (2018, 12, 5) | (2012, 10, 30) | (2004, 6, 11)
                );
            if extra_holiday {
                return false;
            }
        }

        match self.market {
            Market::Nyse
            | Market::Settlement
            | Market::LiborImpact
            | Market::GovernmentBond
            | Market::Nerc
            | Market::FederalReserve
            | Market::Sofr => {
                !(Self::is_washington_birthday(day, month, year, weekday)
                    || Self::is_memorial_day(day, month, year, weekday)
                    || Self::is_independence_day(day, month, weekday)
                    || Self::is_thanksgiving(day, month, weekday)
                    || Self::is_christmas(day, month, weekday))
            }
        }
    }
}

impl ImplCalendar for UnitedStates {
    fn impl_is_business_day(&self, date: &Date) -> bool {
        self.is_business_day(date.base_date())
    }
    fn impl_name(&self) -> String {
        format!("UnitedStates({:?})", self.market)
    }

    fn added_holidays(&self) -> HashSet<Date> {
        self.added_holidays.clone()
    }

    fn removed_holidays(&self) -> HashSet<Date> {
        self.removed_holidays.clone()
    }

    fn add_holiday(&mut self, date: Date) {
        self.added_holidays.insert(date);
    }

    fn remove_holiday(&mut self, date: Date) {
        self.removed_holidays.insert(date);
    }

    fn holiday_list(&self, from: Date, to: Date, include_weekends: bool) -> Vec<Date> {
        let mut holidays = vec![];
        let mut d = from;
        while d <= to {
            if self.is_holiday(&d) {
                holidays.push(d);
            }
            d += 1;
        }
        if include_weekends {
            holidays
        } else {
            holidays
                .into_iter()
                .filter(|d| !self.is_weekend(&d.weekday()))
                .collect()
        }
    }

    fn business_day_list(&self, from: Date, to: Date) -> Vec<Date> {
        let mut business_days = vec![];
        let mut d = from;
        while d <= to {
            if self.is_business_day(d.base_date()) {
                business_days.push(d);
            }
            d += 1;
        }
        business_days
    }
}

impl IsCalendar for UnitedStates {}

impl Default for UnitedStates {
    fn default() -> Self {
        Self::new(Market::Sofr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sofr_calendar_matches_holidays_and_observances() {
        let calendar = UnitedStates::new(Market::Sofr);
        for (year, month, day) in [
            (2026, 1, 1),
            (2026, 1, 19),
            (2026, 4, 3),
            (2026, 6, 19),
            (2026, 7, 3),
            (2026, 9, 7),
            (2026, 10, 12),
            (2026, 11, 11),
            (2026, 11, 26),
            (2026, 12, 25),
            (2027, 6, 18),
        ] {
            assert!(!calendar.is_business_day(Date::new(year, month, day).base_date()));
        }
        for (year, month, day) in [
            (2026, 1, 2),
            (2026, 10, 8),
            (2026, 10, 9),
            (2026, 10, 13),
            (2026, 11, 12),
            (2027, 12, 31),
        ] {
            assert!(calendar.is_business_day(Date::new(year, month, day).base_date()));
        }
    }
}

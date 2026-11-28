use std::collections::HashSet;

use chrono::{Datelike, Days, NaiveDate, Weekday};

use super::traits::{easter_monday, ImplCalendar, IsCalendar};
use crate::time::date::Date;

/// # Colombia
/// A calendar for Colombia.
///
/// Holidays follow Ley Emiliani (Law 51 of 1983): several holidays that do not
/// fall on a Monday are observed on the following Monday.
///
/// Fixed holidays (never moved):
/// * New Year's Day (January 1st)
/// * Labour Day (May 1st)
/// * Independence Day (July 20th)
/// * Battle of Boyacá (August 7th)
/// * Immaculate Conception (December 8th)
/// * Christmas Day (December 25th)
///
/// Holidays moved to the following Monday:
/// * Epiphany (January 6th)
/// * Saint Joseph's Day (March 19th)
/// * Saint Peter and Saint Paul (June 29th)
/// * Assumption Day (August 15th)
/// * Columbus Day (October 12th)
/// * All Saints' Day (November 1st)
/// * Independence of Cartagena (November 11th)
///
/// Easter-based holidays:
/// * Maundy Thursday and Good Friday (not moved)
/// * Ascension, Corpus Christi and Sacred Heart (observed on the following Monday)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Market {
    /// Bolsa de Valores de Colombia (Bogotá).
    BVC,
}

/// Colombia calendar for business day calculations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Colombia {
    market: Market,
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

/// Holidays (month, day) observed on their calendar date.
const FIXED_HOLIDAYS: [(u32, u32); 6] = [(1, 1), (5, 1), (7, 20), (8, 7), (12, 8), (12, 25)];

/// Holidays (month, day) moved to the following Monday under Ley Emiliani.
const EMILIANI_HOLIDAYS: [(u32, u32); 7] = [
    (1, 6),
    (3, 19),
    (6, 29),
    (8, 15),
    (10, 12),
    (11, 1),
    (11, 11),
];

impl Colombia {
    /// Creates a new Colombia calendar instance for the specified market.
    #[must_use]
    pub fn new(market: Market) -> Self {
        Self {
            market,
            added_holidays: HashSet::new(),
            removed_holidays: HashSet::new(),
        }
    }

    /// Checks whether the weekday is Saturday or Sunday.
    fn is_weekend(day: Weekday) -> bool {
        day == Weekday::Sat || day == Weekday::Sun
    }

    /// Checks the date against the fixed (never moved) holidays.
    fn is_fixed_holiday(day: u32, month: u32) -> bool {
        FIXED_HOLIDAYS.contains(&(month, day))
    }

    /// Returns the date itself if it is a Monday, otherwise the following Monday.
    fn next_monday(date: NaiveDate) -> Option<NaiveDate> {
        let offset = (7 - date.weekday().num_days_from_monday()) % 7;
        date.checked_add_days(Days::new(u64::from(offset)))
    }

    /// Checks the date against the holidays moved to Monday by Ley Emiliani.
    fn is_emiliani_holiday(date: NaiveDate) -> bool {
        EMILIANI_HOLIDAYS.iter().any(|&(month, day)| {
            NaiveDate::from_ymd_opt(date.year(), month, day)
                .and_then(Self::next_monday)
                .is_some_and(|observed| observed == date)
        })
    }

    /// Checks the date against the Easter-based holidays.
    ///
    /// Maundy Thursday and Good Friday are not moved; Ascension (Easter + 39),
    /// Corpus Christi (Easter + 60) and Sacred Heart (Easter + 68) are observed
    /// on the following Monday (Easter + 43, + 64 and + 71).
    fn is_easter_holiday(day: u32, month: u32, year: i32) -> bool {
        let easter_sunday = easter_monday(year) - 1;
        let dd = Date::new(year, month, day).day_of_year();
        [-3, -2, 43, 64, 71]
            .iter()
            .any(|offset| dd == easter_sunday + offset)
    }

    /// Determines if a given date is a business day in the Colombian market.
    #[must_use]
    pub fn is_business_day(&self, date: NaiveDate) -> bool {
        let weekday = date.weekday();
        let day = date.day();
        let month = date.month();
        let year = date.year();
        if Self::is_weekend(weekday) {
            return false;
        }

        match self.market {
            Market::BVC => {
                !(Self::is_fixed_holiday(day, month)
                    || Self::is_emiliani_holiday(date)
                    || Self::is_easter_holiday(day, month, year))
            }
        }
    }
}

impl ImplCalendar for Colombia {
    fn impl_is_business_day(&self, date: &Date) -> bool {
        self.is_business_day(date.base_date())
    }

    fn impl_name(&self) -> String {
        format!("Colombia({:?})", self.market)
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

impl IsCalendar for Colombia {}

impl Default for Colombia {
    fn default() -> Self {
        Self::new(Market::BVC)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::date::Date;

    #[test]
    fn colombia_2026_holidays() {
        let cal = Colombia::default();
        let expected = vec![
            Date::new(2026, 1, 1),
            Date::new(2026, 1, 12),
            Date::new(2026, 3, 23),
            Date::new(2026, 4, 2),
            Date::new(2026, 4, 3),
            Date::new(2026, 5, 1),
            Date::new(2026, 5, 18),
            Date::new(2026, 6, 8),
            Date::new(2026, 6, 15),
            Date::new(2026, 6, 29),
            Date::new(2026, 7, 20),
            Date::new(2026, 8, 7),
            Date::new(2026, 8, 17),
            Date::new(2026, 10, 12),
            Date::new(2026, 11, 2),
            Date::new(2026, 11, 16),
            Date::new(2026, 12, 8),
            Date::new(2026, 12, 25),
        ];
        let holidays = cal.holiday_list(Date::new(2026, 1, 1), Date::new(2026, 12, 31), false);
        assert_eq!(holidays, expected);
    }

    #[test]
    fn colombia_2025_holidays() {
        let cal = Colombia::default();
        for d in [
            Date::new(2025, 1, 6),
            Date::new(2025, 3, 24),
            Date::new(2025, 4, 17),
            Date::new(2025, 4, 18),
            Date::new(2025, 6, 2),
            Date::new(2025, 6, 23),
            Date::new(2025, 6, 30),
            Date::new(2025, 8, 18),
            Date::new(2025, 10, 13),
            Date::new(2025, 11, 3),
            Date::new(2025, 11, 17),
        ] {
            assert!(
                !cal.is_business_day(d.base_date()),
                "{d} should be a holiday"
            );
        }
    }

    #[test]
    fn colombia_ordinary_days_are_business_days() {
        let cal = Colombia::default();
        for d in [
            Date::new(2026, 1, 6),
            Date::new(2026, 3, 19),
            Date::new(2026, 4, 6),
            Date::new(2026, 5, 14),
            Date::new(2026, 8, 14),
            Date::new(2026, 11, 11),
            Date::new(2025, 6, 27),
            Date::new(2025, 7, 1),
        ] {
            assert!(
                cal.is_business_day(d.base_date()),
                "{d} should be a business day"
            );
        }
        assert!(!cal.is_business_day(Date::new(2026, 1, 10).base_date()));
        assert!(!cal.is_business_day(Date::new(2026, 1, 11).base_date()));
        assert_eq!(cal.impl_name(), "Colombia(BVC)");
    }
}

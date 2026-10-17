use std::collections::HashSet;

use chrono::{Datelike, NaiveDate, Weekday};

use super::traits::{easter_monday, ImplCalendar, IsCalendar};
use crate::time::date::Date;

/// # Brazil     
/// A calendar for Brazil
///

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Market {
    /// Settlement market type.
    Settlement,
    /// Exchange market type.
    Exchange,
}

/// Brazil calendar for business day calculations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Brazil {
    market: Market,
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}

impl Brazil {
    /// Creates a new Brazil calendar with the specified market type.
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

    const fn is_new_years_day(day: u32, month: u32) -> bool {
        day == 1 && month == 1
    }

    const fn is_sao_paulo_city_day(day: u32, month: u32) -> bool {
        day == 25 && month == 1
    }

    const fn is_tiradentes_day(day: u32, month: u32) -> bool {
        day == 21 && month == 4
    }

    const fn is_labor_day(day: u32, month: u32) -> bool {
        day == 1 && month == 5
    }

    const fn is_revolution_day(day: u32, month: u32) -> bool {
        day == 9 && month == 7
    }

    const fn is_independence_day(day: u32, month: u32) -> bool {
        day == 7 && month == 9
    }

    const fn is_nossa_senhora_aparecida_day(day: u32, month: u32) -> bool {
        day == 12 && month == 10
    }

    const fn is_all_souls_day(day: u32, month: u32) -> bool {
        day == 2 && month == 11
    }

    const fn is_republic_day(day: u32, month: u32) -> bool {
        day == 15 && month == 11
    }

    const fn is_black_consciousness_day(day: u32, month: u32, year: i32) -> bool {
        day == 20 && month == 11 && year >= 2007
    }

    const fn is_christmas_eve(day: u32, month: u32) -> bool {
        day == 24 && month == 12
    }

    const fn is_christmas(day: u32, month: u32) -> bool {
        day == 25 && month == 12
    }

    fn is_passion_of_christ(day: u32, month: u32, year: i32) -> bool {
        let em = easter_monday(year);
        let dd = Date::new(year, month, day).day_of_year();
        em - 3 == dd
    }

    fn is_carnival(day: u32, month: u32, year: i32) -> bool {
        let em = easter_monday(year);
        let dd = Date::new(year, month, day).day_of_year();
        em - 49 == dd || em - 48 == dd
    }

    fn is_corpus_christi(day: u32, month: u32, year: i32) -> bool {
        let em = easter_monday(year);
        let dd = Date::new(year, month, day).day_of_year();
        em + 59 == dd
    }

    fn is_last_business_day_of_year(day: u32, month: u32, year: i32) -> bool {
        let w = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap_or_else(|| panic!("valid date for last business day calculation"))
            .weekday();
        month == 12 && (day == 31 || (day >= 29 && w == Weekday::Fri))
    }

    /// Checks if the given date is a business day according to the calendar rules.
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
            Market::Settlement => {
                !(Self::is_new_years_day(day, month)
                    || Self::is_tiradentes_day(day, month)
                    || Self::is_labor_day(day, month)
                    || Self::is_independence_day(day, month)
                    || Self::is_nossa_senhora_aparecida_day(day, month)
                    || Self::is_all_souls_day(day, month)
                    || Self::is_republic_day(day, month)
                    || Self::is_christmas(day, month)
                    // National holiday since Law 14,759/2023, effective in 2024.
                    || (Self::is_black_consciousness_day(day, month, year) && year >= 2024)
                    || Self::is_passion_of_christ(day, month, year)
                    || Self::is_carnival(day, month, year)
                    || Self::is_corpus_christi(day, month, year))
            }
            Market::Exchange => {
                !(Self::is_new_years_day(day, month)
                    || Self::is_sao_paulo_city_day(day, month)
                    || Self::is_tiradentes_day(day, month)
                    || Self::is_labor_day(day, month)
                    || Self::is_revolution_day(day, month)
                    || Self::is_independence_day(day, month)
                    || Self::is_nossa_senhora_aparecida_day(day, month)
                    || Self::is_all_souls_day(day, month)
                    || Self::is_republic_day(day, month)
                    || Self::is_black_consciousness_day(day, month, year)
                    || Self::is_christmas_eve(day, month)
                    || Self::is_christmas(day, month)
                    || Self::is_passion_of_christ(day, month, year)
                    || Self::is_carnival(day, month, year)
                    || Self::is_corpus_christi(day, month, year)
                    || Self::is_last_business_day_of_year(day, month, year))
            }
        }
    }
}

impl ImplCalendar for Brazil {
    fn impl_is_business_day(&self, date: &Date) -> bool {
        self.is_business_day(date.base_date())
    }

    fn impl_name(&self) -> String {
        format!("Brazil({:?})", self.market)
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
            if self.is_business_day(d.base_date()) {
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

impl IsCalendar for Brazil {}

impl Default for Brazil {
    fn default() -> Self {
        Self::new(Market::Settlement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::date::Date;

    #[test]
    fn test_brazil_settlement() {
        let cal = Brazil::new(Market::Settlement);
        let expected_hol = vec![
            Date::new(2005, 2, 7),
            Date::new(2005, 2, 8),
            Date::new(2005, 3, 25),
            Date::new(2005, 4, 21),
            Date::new(2005, 5, 26),
            Date::new(2005, 9, 7),
            Date::new(2005, 10, 12),
            Date::new(2005, 11, 2),
            Date::new(2005, 11, 15),
            Date::new(2006, 2, 27),
            Date::new(2006, 2, 28),
            Date::new(2006, 4, 14),
            Date::new(2006, 4, 21),
            Date::new(2006, 5, 1),
            Date::new(2006, 6, 15),
            Date::new(2006, 9, 7),
            Date::new(2006, 10, 12),
            Date::new(2006, 11, 2),
            Date::new(2006, 11, 15),
        ];

        for d in expected_hol {
            assert!(!cal.is_business_day(d.base_date()));
        }
    }
}

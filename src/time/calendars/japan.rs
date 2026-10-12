use super::traits::{ImplCalendar, IsCalendar};
use crate::time::{date::Date, enums::Weekday};
use std::collections::HashSet;

/// Japan bank calendar, including public, substitute and citizens' holidays.
///
/// Includes bank closures on January 2–3 and December 31, and the 2019 accession
/// and 2020–2021 Olympic changes. Equinox dates after the officially announced
/// year are astronomical estimates; future exceptional holidays require updates.
/// Public-holiday rules cover the regime introduced in July 1948.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Japan {
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}
impl Japan {
    /// Creates the Japan bank calendar.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn equinox(year: i32, spring: bool) -> u32 {
        let base = match (year < 1980, spring) {
            (true, true) => 20.8357,
            (false, true) => 20.8431,
            (true, false) => 23.2588,
            (false, false) => 23.2488,
        };
        let offset = year - 1980;
        let leap_years = year.div_euclid(4) - year.div_euclid(100) + year.div_euclid(400) - 480;
        (0.242194_f64.mul_add(f64::from(offset), base) - f64::from(leap_years)).floor() as u32
    }

    fn national_holiday(date: Date) -> bool {
        let year = date.year();
        let month = date.month();
        let day = date.day();
        if date < Date::new(1948, 7, 20) {
            return false;
        }
        let monday = date.weekday() == Weekday::Monday;
        match month {
            1 => {
                day == 1
                    || if year >= 2000 {
                        monday && (8..=14).contains(&day)
                    } else {
                        day == 15
                    }
            }
            2 => {
                (day == 11 && year >= 1967)
                    || (day == 23 && year >= 2020)
                    || (year == 1989 && day == 24)
            }
            3 => day == Self::equinox(year, true),
            4 => day == 29 || (year == 1959 && day == 10),
            5 => day == 3 || day == 5 || (day == 4 && year >= 2007) || (year == 2019 && day == 1),
            6 => year == 1993 && day == 9,
            7 => match year {
                2020 => day == 23 || day == 24,
                2021 => day == 22 || day == 23,
                2003.. => monday && (15..=21).contains(&day),
                1996..=2002 => day == 20,
                _ => false,
            },
            8 => match year {
                2020 => day == 10,
                2021 => day == 8,
                2016.. => day == 11,
                _ => false,
            },
            9 => {
                day == Self::equinox(year, false)
                    || if year >= 2003 {
                        monday && (15..=21).contains(&day)
                    } else {
                        day == 15 && year >= 1966
                    }
            }
            10 => {
                (year == 2019 && day == 22)
                    || match year {
                        2020 | 2021 => false,
                        2000.. => monday && (8..=14).contains(&day),
                        1966..=1999 => day == 10,
                        _ => false,
                    }
            }
            11 => day == 3 || day == 23 || (year == 1990 && day == 12),
            12 => day == 23 && (1989..=2018).contains(&year),
            _ => false,
        }
    }
}
impl ImplCalendar for Japan {
    fn impl_name(&self) -> String {
        "Japan".into()
    }
    fn impl_is_business_day(&self, date: &Date) -> bool {
        if self.is_weekend(&date.weekday())
            || Self::national_holiday(*date)
            || (date.month() == 1 && date.day() <= 3)
            || (date.month() == 12 && date.day() == 31)
        {
            return false;
        }
        // Since 1986 a weekday between two national holidays is a holiday.
        if date.year() >= 1986
            && Self::national_holiday(*date - 1)
            && Self::national_holiday(*date + 1)
        {
            return false;
        }
        if *date >= Date::new(1973, 4, 12) {
            let mut previous = *date - 1;
            // Since 2007 substitutions roll past consecutive national holidays.
            if date.year() >= 2007 {
                while Self::national_holiday(previous) && previous.weekday() != Weekday::Sunday {
                    previous -= 1;
                }
            }
            if previous.weekday() == Weekday::Sunday && Self::national_holiday(previous) {
                return false;
            }
        }
        true
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
impl IsCalendar for Japan {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn japanese_holidays_and_rule_changes() {
        let cal = Japan::new();
        for (year, month, day) in [
            (2026, 1, 2),
            (2026, 1, 12),
            (2026, 2, 11),
            (2026, 2, 23),
            (2026, 3, 20),
            (2026, 5, 6),
            (2026, 7, 20),
            (2026, 8, 11),
            (2026, 9, 22),
            (2026, 9, 23),
            (2026, 10, 12),
            (2026, 11, 3),
            (2026, 11, 23),
            (2026, 12, 31),
            (2019, 4, 30),
            (2019, 5, 1),
            (2019, 5, 2),
            (2019, 10, 22),
            (2020, 7, 23),
            (2020, 7, 24),
            (2020, 8, 10),
            (2021, 7, 22),
            (2021, 7, 23),
            (2021, 8, 9),
            (2015, 9, 22),
            (2009, 5, 6),
            (1989, 2, 24),
            (1990, 11, 12),
            (1993, 6, 9),
        ] {
            assert!(
                cal.is_holiday(&Date::new(year, month, day)),
                "{year}-{month}-{day}"
            );
        }
        for (year, month, day) in [
            (2019, 12, 23),
            (2020, 7, 20),
            (2020, 8, 11),
            (2020, 10, 12),
            (2021, 7, 19),
            (2021, 8, 11),
            (2021, 10, 11),
            (2026, 9, 24),
        ] {
            assert!(
                cal.is_business_day(&Date::new(year, month, day)),
                "{year}-{month}-{day}"
            );
        }
        assert_eq!(
            cal.adjust(Date::new(2019, 4, 27), None),
            Date::new(2019, 5, 7)
        );
    }
    // Official holiday snapshots: https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv
    #[test]
    fn matches_announced_holidays_for_every_day() {
        let cal = Japan::new();
        for (year, holidays) in [
            (
                2020,
                vec![
                    (1, 1),
                    (1, 13),
                    (2, 11),
                    (2, 23),
                    (2, 24),
                    (3, 20),
                    (4, 29),
                    (5, 3),
                    (5, 4),
                    (5, 5),
                    (5, 6),
                    (7, 23),
                    (7, 24),
                    (8, 10),
                    (9, 21),
                    (9, 22),
                    (11, 3),
                    (11, 23),
                ],
            ),
            (
                2021,
                vec![
                    (1, 1),
                    (1, 11),
                    (2, 11),
                    (2, 23),
                    (3, 20),
                    (4, 29),
                    (5, 3),
                    (5, 4),
                    (5, 5),
                    (7, 22),
                    (7, 23),
                    (8, 8),
                    (8, 9),
                    (9, 20),
                    (9, 23),
                    (11, 3),
                    (11, 23),
                ],
            ),
            (
                2022,
                vec![
                    (1, 1),
                    (1, 10),
                    (2, 11),
                    (2, 23),
                    (3, 21),
                    (4, 29),
                    (5, 3),
                    (5, 4),
                    (5, 5),
                    (7, 18),
                    (8, 11),
                    (9, 19),
                    (9, 23),
                    (10, 10),
                    (11, 3),
                    (11, 23),
                ],
            ),
            (
                2023,
                vec![
                    (1, 1),
                    (1, 2),
                    (1, 9),
                    (2, 11),
                    (2, 23),
                    (3, 21),
                    (4, 29),
                    (5, 3),
                    (5, 4),
                    (5, 5),
                    (7, 17),
                    (8, 11),
                    (9, 18),
                    (9, 23),
                    (10, 9),
                    (11, 3),
                    (11, 23),
                ],
            ),
            (
                2024,
                vec![
                    (1, 1),
                    (1, 8),
                    (2, 11),
                    (2, 12),
                    (2, 23),
                    (3, 20),
                    (4, 29),
                    (5, 3),
                    (5, 4),
                    (5, 5),
                    (5, 6),
                    (7, 15),
                    (8, 11),
                    (8, 12),
                    (9, 16),
                    (9, 22),
                    (9, 23),
                    (10, 14),
                    (11, 3),
                    (11, 4),
                    (11, 23),
                ],
            ),
            (
                2025,
                vec![
                    (1, 1),
                    (1, 13),
                    (2, 11),
                    (2, 23),
                    (2, 24),
                    (3, 20),
                    (4, 29),
                    (5, 3),
                    (5, 4),
                    (5, 5),
                    (5, 6),
                    (7, 21),
                    (8, 11),
                    (9, 15),
                    (9, 23),
                    (10, 13),
                    (11, 3),
                    (11, 23),
                    (11, 24),
                ],
            ),
            (
                2026,
                vec![
                    (1, 1),
                    (1, 12),
                    (2, 11),
                    (2, 23),
                    (3, 20),
                    (4, 29),
                    (5, 3),
                    (5, 4),
                    (5, 5),
                    (5, 6),
                    (7, 20),
                    (8, 11),
                    (9, 21),
                    (9, 22),
                    (9, 23),
                    (10, 12),
                    (11, 3),
                    (11, 23),
                ],
            ),
            (
                2027,
                vec![
                    (1, 1),
                    (1, 11),
                    (2, 11),
                    (2, 23),
                    (3, 21),
                    (3, 22),
                    (4, 29),
                    (5, 3),
                    (5, 4),
                    (5, 5),
                    (7, 19),
                    (8, 11),
                    (9, 20),
                    (9, 23),
                    (10, 11),
                    (11, 3),
                    (11, 23),
                ],
            ),
        ] {
            let mut date = Date::new(year, 1, 1);
            while date.year() == year {
                let expected = cal.is_weekend(&date.weekday())
                    || holidays.contains(&(date.month(), date.day()))
                    || (date.month() == 1 && date.day() <= 3)
                    || (date.month() == 12 && date.day() == 31);
                assert_eq!(cal.is_holiday(&date), expected, "{date}");
                date += 1;
            }
        }
    }
}

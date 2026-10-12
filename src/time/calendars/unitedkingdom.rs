use super::traits::{ImplCalendar, IsCalendar};
use crate::time::{date::Date, enums::Weekday};
use std::collections::HashSet;

/// London settlement calendar (England and Wales bank holidays).
///
/// Includes substitute days and announced exceptional holidays through 2026.
/// Recurring rules assume the modern bank-holiday regime (since 1971).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitedKingdom {
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}
impl UnitedKingdom {
    /// Creates the London settlement calendar.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
impl ImplCalendar for UnitedKingdom {
    fn impl_name(&self) -> String {
        "UnitedKingdom(Settlement)".into()
    }
    #[allow(clippy::cast_sign_loss)]
    fn easter_monday(&self, year: i32) -> i32 {
        // Gregorian computus avoids the shared lookup table's 1901–2199 bounds.
        let golden = year.rem_euclid(19);
        let century = year.div_euclid(100);
        let within_century = year.rem_euclid(100);
        let shift = (century + 8).div_euclid(25);
        let correction = (century - shift + 1).div_euclid(3);
        let epact =
            (19 * golden + century - century.div_euclid(4) - correction + 15).rem_euclid(30);
        let weekday = (32 + 2 * century.rem_euclid(4) + 2 * within_century.div_euclid(4)
            - epact
            - within_century.rem_euclid(4))
        .rem_euclid(7);
        let adjustment = (golden + 11 * epact + 22 * weekday).div_euclid(451);
        let number = epact + weekday - 7 * adjustment + 114;
        let month = number.div_euclid(31) as u32;
        let day = (number.rem_euclid(31) + 1) as u32;
        (Date::new(year, month, day) + 1).day_of_year()
    }
    fn impl_is_business_day(&self, date: &Date) -> bool {
        let year = date.year();
        let month = date.month();
        let day = date.day();
        let monday = date.weekday() == Weekday::Monday;
        let easter = self.easter_monday(year);
        let early_may = month == 5
            && if year == 1995 || year == 2020 {
                day == 8
            } else {
                monday && day <= 7
            };
        let spring = if [2002, 2012, 2022].contains(&year) {
            month == 6
                && match year {
                    2002 => day == 3 || day == 4,
                    2012 => day == 4 || day == 5,
                    _ => day == 2 || day == 3,
                }
        } else {
            month == 5 && monday && day >= 25
        };
        let special = matches!(
            (year, month, day),
            (2011, 4, 29) | (2022, 9, 19) | (2023, 5, 8) | (1999, 12, 31)
        );
        !(self.is_weekend(&date.weekday())
            || (month == 1 && (day == 1 || (monday && day <= 3)))
            || date.day_of_year() == easter - 3
            || date.day_of_year() == easter
            || early_may
            || spring
            || (month == 8 && monday && day >= 25)
            || (month == 12
                && (day == 25
                    || day == 26
                    || ((day == 27 || day == 28)
                        && matches!(date.weekday(), Weekday::Monday | Weekday::Tuesday))))
            || special)
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
impl IsCalendar for UnitedKingdom {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn london_holidays_and_substitutions() {
        let cal = UnitedKingdom::new();
        for (year, month, day) in [
            (2026, 4, 3),
            (2026, 4, 6),
            (2026, 5, 4),
            (2026, 5, 25),
            (2026, 8, 31),
            (2026, 12, 28),
            (2021, 12, 27),
            (2021, 12, 28),
            (2022, 1, 3),
            (2022, 6, 2),
            (2022, 6, 3),
            (2022, 9, 19),
            (2023, 5, 8),
            (2012, 6, 4),
            (2002, 6, 4),
            (2020, 5, 8),
            (1995, 5, 8),
            (2011, 4, 29),
        ] {
            assert!(cal.is_holiday(&Date::new(year, month, day)));
        }
        for (year, month, day) in [(2022, 5, 30), (2020, 5, 4), (2026, 12, 29)] {
            assert!(cal.is_business_day(&Date::new(year, month, day)));
        }
        assert_eq!(
            cal.adjust(Date::new(2026, 4, 3), None),
            Date::new(2026, 4, 7)
        );
    }
    #[test]
    fn lists_respect_overrides_and_boundaries() {
        let mut cal = UnitedKingdom::new();
        let holiday = Date::new(2023, 5, 8);
        cal.remove_holiday(holiday);
        assert!(cal.is_business_day(&holiday));
        cal.add_holiday(holiday);
        assert!(cal.is_holiday(&holiday));
        assert_eq!(
            cal.holiday_list(Date::new(2023, 5, 5), holiday, false),
            vec![holiday]
        );
        assert_eq!(
            cal.business_day_list(holiday, Date::new(2023, 5, 9)),
            vec![Date::new(2023, 5, 9)]
        );
    }
    // Official holiday snapshots: https://www.gov.uk/bank-holidays.json
    #[test]
    fn matches_announced_holidays_for_every_day() {
        let cal = UnitedKingdom::new();
        for (year, holidays) in [
            (
                2020,
                vec![
                    (1, 1),
                    (4, 10),
                    (4, 13),
                    (5, 8),
                    (5, 25),
                    (8, 31),
                    (12, 25),
                    (12, 28),
                ],
            ),
            (
                2021,
                vec![
                    (1, 1),
                    (4, 2),
                    (4, 5),
                    (5, 3),
                    (5, 31),
                    (8, 30),
                    (12, 27),
                    (12, 28),
                ],
            ),
            (
                2022,
                vec![
                    (1, 3),
                    (4, 15),
                    (4, 18),
                    (5, 2),
                    (6, 2),
                    (6, 3),
                    (8, 29),
                    (9, 19),
                    (12, 26),
                    (12, 27),
                ],
            ),
            (
                2023,
                vec![
                    (1, 2),
                    (4, 7),
                    (4, 10),
                    (5, 1),
                    (5, 8),
                    (5, 29),
                    (8, 28),
                    (12, 25),
                    (12, 26),
                ],
            ),
            (
                2024,
                vec![
                    (1, 1),
                    (3, 29),
                    (4, 1),
                    (5, 6),
                    (5, 27),
                    (8, 26),
                    (12, 25),
                    (12, 26),
                ],
            ),
            (
                2025,
                vec![
                    (1, 1),
                    (4, 18),
                    (4, 21),
                    (5, 5),
                    (5, 26),
                    (8, 25),
                    (12, 25),
                    (12, 26),
                ],
            ),
            (
                2026,
                vec![
                    (1, 1),
                    (4, 3),
                    (4, 6),
                    (5, 4),
                    (5, 25),
                    (8, 31),
                    (12, 25),
                    (12, 28),
                ],
            ),
            (
                2027,
                vec![
                    (1, 1),
                    (3, 26),
                    (3, 29),
                    (5, 3),
                    (5, 31),
                    (8, 30),
                    (12, 27),
                    (12, 28),
                ],
            ),
            (
                2028,
                vec![
                    (1, 3),
                    (4, 14),
                    (4, 17),
                    (5, 1),
                    (5, 29),
                    (8, 28),
                    (12, 25),
                    (12, 26),
                ],
            ),
        ] {
            let mut date = Date::new(year, 1, 1);
            while date.year() == year {
                let expected = cal.is_weekend(&date.weekday())
                    || holidays.contains(&(date.month(), date.day()));
                assert_eq!(cal.is_holiday(&date), expected, "{date}");
                date += 1;
            }
        }
    }
    #[test]
    fn easter_matches_existing_dates_and_extends_beyond_lookup_range() {
        let calendar = UnitedKingdom::new();
        for year in 1901..=2199 {
            assert_eq!(
                calendar.easter_monday(year),
                super::super::traits::easter_monday(year)
            );
        }
        assert!(calendar.is_holiday(&Date::new(2200, 4, 7)));
        assert!(calendar.is_business_day(&Date::new(2200, 1, 7)));
    }
}

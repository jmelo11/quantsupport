use super::traits::{ImplCalendar, IsCalendar};
use crate::{
    time::{calendar::Calendar, date::Date, enums::Weekday},
    utils::errors::{QSError, Result},
};
use std::collections::HashSet;

/// How a joint calendar combines its constituent calendars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointCalendarRule {
    /// A business day must be open in every constituent calendar.
    JoinHolidays,
    /// A business day must be open in at least one constituent calendar.
    JoinBusinessDays,
}
/// Calendar combining two or more markets, typically for cross-currency settlement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JointCalendar {
    calendars: Vec<Calendar>,
    rule: JointCalendarRule,
    added_holidays: HashSet<Date>,
    removed_holidays: HashSet<Date>,
}
impl JointCalendar {
    /// Combines calendars using the selected rule.
    /// # Errors
    /// Returns an error when fewer than two calendars are supplied.
    pub fn new(calendars: Vec<Calendar>, rule: JointCalendarRule) -> Result<Self> {
        if calendars.len() < 2 {
            return Err(QSError::InvalidValueErr(
                "JointCalendar requires at least two calendars".into(),
            ));
        }
        Ok(Self {
            calendars,
            rule,
            added_holidays: HashSet::new(),
            removed_holidays: HashSet::new(),
        })
    }
    /// Constituent calendars, including any local holiday overrides.
    #[must_use]
    pub fn calendars(&self) -> &[Calendar] {
        &self.calendars
    }
    /// Combination rule.
    #[must_use]
    pub const fn rule(&self) -> JointCalendarRule {
        self.rule
    }
}
impl ImplCalendar for JointCalendar {
    fn impl_name(&self) -> String {
        let rule = match self.rule {
            JointCalendarRule::JoinHolidays => "JoinHolidays",
            JointCalendarRule::JoinBusinessDays => "JoinBusinessDays",
        };
        let names: Vec<_> = self.calendars.iter().map(ImplCalendar::impl_name).collect();
        format!("JointCalendar({rule},{})", names.join(","))
    }
    fn impl_is_business_day(&self, date: &Date) -> bool {
        match self.rule {
            JointCalendarRule::JoinHolidays => {
                self.calendars.iter().all(|cal| cal.is_business_day(date))
            }
            JointCalendarRule::JoinBusinessDays => {
                self.calendars.iter().any(|cal| cal.is_business_day(date))
            }
        }
    }
    fn is_weekend(&self, weekday: &Weekday) -> bool {
        match self.rule {
            JointCalendarRule::JoinHolidays => {
                self.calendars.iter().any(|cal| cal.is_weekend(weekday))
            }
            JointCalendarRule::JoinBusinessDays => {
                self.calendars.iter().all(|cal| cal.is_weekend(weekday))
            }
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
impl IsCalendar for JointCalendar {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn joint_rules_and_overrides() {
        let uk = Calendar::try_from("UnitedKingdom(Settlement)".to_string()).unwrap();
        let mut jp = Calendar::try_from("Japan".to_string()).unwrap();
        let date = Date::new(2026, 5, 4);
        jp.remove_holiday(date);
        let mut both = JointCalendar::new(
            vec![uk.clone(), jp.clone()],
            JointCalendarRule::JoinHolidays,
        )
        .unwrap();
        let either = JointCalendar::new(vec![uk, jp], JointCalendarRule::JoinBusinessDays).unwrap();
        assert!(both.is_holiday(&date));
        assert!(either.is_business_day(&date));
        both.remove_holiday(date);
        assert!(both.is_business_day(&date));
        both.add_holiday(date);
        assert!(both.is_holiday(&date));
        assert!(JointCalendar::new(vec![], JointCalendarRule::JoinHolidays).is_err());
        assert_eq!(both.holiday_list(date, date, false), vec![date]);
    }
    #[test]
    fn joint_weekends_follow_the_combination_rule() {
        let either =
            Calendar::try_from("JointCalendar(JoinBusinessDays,NullCalendar,Japan)".to_string())
                .unwrap();
        assert!(!either.is_weekend(&Weekday::Saturday));
        assert!(either.is_business_day(&Date::new(2026, 1, 3)));
        let both = Calendar::try_from("JointCalendar(JoinHolidays,NullCalendar,Japan)".to_string())
            .unwrap();
        assert!(both.is_weekend(&Weekday::Saturday));
    }
}

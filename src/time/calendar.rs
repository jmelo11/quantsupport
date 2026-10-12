use serde::Serialize;

use super::calendars::{
    brazil::Brazil,
    chile::Chile,
    japan::Japan,
    jointcalendar::{JointCalendar, JointCalendarRule},
    nullcalendar::NullCalendar,
    target::TARGET,
    traits::{ImplCalendar, IsCalendar},
    unitedkingdom::UnitedKingdom,
    unitedstates::UnitedStates,
    weekendsonly::WeekendsOnly,
};
use crate::{
    time::date::Date,
    utils::errors::{QSError, Result},
};
use std::collections::HashSet;

/// A calendar.
///
/// ## Enums
/// * [`Self::NullCalendar`] - A calendar that considers all days as business days.
/// * [`Self::WeekendsOnly`] - A calendar that considers only weekends as business days.
/// * [`Self::TARGET`] - A calendar that considers only TARGET business days as business days.
/// * [`Self::UnitedStates`] - A calendar for the United States.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Calendar {
    /// A null calendar that considers all days as business days.
    NullCalendar(NullCalendar),
    /// A calendar that considers only weekends as non-business days.
    WeekendsOnly(WeekendsOnly),
    /// TARGET Eurosystem calendar for business days.
    TARGET(TARGET),
    /// A calendar for the United States.
    UnitedStates(UnitedStates),
    /// A calendar for Brazil.
    Brazil(Brazil),
    /// A calendar for Chile.
    Chile(Chile),
    /// London settlement calendar.
    UnitedKingdom(UnitedKingdom),
    /// Japan bank calendar.
    Japan(Japan),
    /// A combination of multiple market calendars.
    JointCalendar(JointCalendar),
}

impl Serialize for Calendar {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            Self::NullCalendar(cal) => cal.impl_name(),
            Self::WeekendsOnly(cal) => cal.impl_name(),
            Self::TARGET(cal) => cal.impl_name(),
            Self::UnitedStates(cal) => cal.impl_name(),
            Self::Brazil(cal) => cal.impl_name(),
            Self::Chile(cal) => cal.impl_name(),
            Self::UnitedKingdom(cal) => cal.impl_name(),
            Self::Japan(cal) => cal.impl_name(),
            Self::JointCalendar(cal) => cal.impl_name(),
        };
        serializer.serialize_str(&s)
    }
}

impl<'de> serde::Deserialize<'de> for Calendar {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<String> for Calendar {
    type Error = QSError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "UnitedKingdom(Settlement)" => Ok(Self::UnitedKingdom(UnitedKingdom::new())),
            "Japan" => Ok(Self::Japan(Japan::new())),
            name if name.starts_with("JointCalendar(") => parse_joint_calendar(name),
            "NullCalendar" => Ok(Self::NullCalendar(NullCalendar::new())),
            "WeekendsOnly" => Ok(Self::WeekendsOnly(WeekendsOnly::new())),
            "TARGET" => Ok(Self::TARGET(TARGET::new())),
            "UnitedStates(Sofr)" => Ok(Self::UnitedStates(UnitedStates::default())),
            "UnitedStates(Settlement)" => Ok(Self::UnitedStates(UnitedStates::new(
                super::calendars::unitedstates::Market::Settlement,
            ))),
            "UnitedStates(LiborImpact)" => Ok(Self::UnitedStates(UnitedStates::new(
                super::calendars::unitedstates::Market::LiborImpact,
            ))),
            "UnitedStates(Nyse)" => Ok(Self::UnitedStates(UnitedStates::new(
                super::calendars::unitedstates::Market::Nyse,
            ))),
            "UnitedStates(GovernmentBond)" => Ok(Self::UnitedStates(UnitedStates::new(
                super::calendars::unitedstates::Market::GovernmentBond,
            ))),
            "UnitedStates(Nerc)" => Ok(Self::UnitedStates(UnitedStates::new(
                super::calendars::unitedstates::Market::Nerc,
            ))),
            "UnitedStates(FederalReserve)" => Ok(Self::UnitedStates(UnitedStates::new(
                super::calendars::unitedstates::Market::FederalReserve,
            ))),
            "Brazil(Settlement)" => Ok(Self::Brazil(Brazil::default())),
            "Brazil(Exchange)" => Ok(Self::Brazil(Brazil::new(
                super::calendars::brazil::Market::Exchange,
            ))),
            "Chile(SSE)" => Ok(Self::Chile(Chile::default())),
            _ => Err(QSError::InvalidValueErr(format!("Invalid calendar: {s}"))),
        }
    }
}

fn parse_joint_calendar(name: &str) -> Result<Calendar> {
    let invalid = || QSError::InvalidValueErr(format!("Invalid joint calendar: {name}"));
    let body = name
        .strip_prefix("JointCalendar(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(invalid)?;
    let mut parts = Vec::new();
    let mut depth = 0_u32;
    let mut start = 0;
    for (index, ch) in body.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1).ok_or_else(invalid)?,
            ',' if depth == 0 => {
                parts.push(&body[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(invalid());
    }
    parts.push(&body[start..]);
    let rule = match parts.first().copied() {
        Some("JoinHolidays") => JointCalendarRule::JoinHolidays,
        Some("JoinBusinessDays") => JointCalendarRule::JoinBusinessDays,
        _ => return Err(invalid()),
    };
    let calendars = parts
        .into_iter()
        .skip(1)
        .map(|s| Calendar::try_from(s.to_string()))
        .collect::<Result<Vec<_>>>()?;
    Ok(Calendar::JointCalendar(JointCalendar::new(
        calendars, rule,
    )?))
}

impl From<Calendar> for String {
    fn from(calendar: Calendar) -> Self {
        calendar.impl_name()
    }
}

impl ImplCalendar for Calendar {
    fn impl_name(&self) -> String {
        match self {
            Self::NullCalendar(cal) => cal.impl_name(),
            Self::WeekendsOnly(cal) => cal.impl_name(),
            Self::TARGET(cal) => cal.impl_name(),
            Self::UnitedStates(cal) => cal.impl_name(),
            Self::Brazil(cal) => cal.impl_name(),
            Self::Chile(cal) => cal.impl_name(),
            Self::UnitedKingdom(cal) => cal.impl_name(),
            Self::Japan(cal) => cal.impl_name(),
            Self::JointCalendar(cal) => cal.impl_name(),
        }
    }

    fn impl_is_business_day(&self, date: &Date) -> bool {
        match self {
            Self::NullCalendar(cal) => cal.impl_is_business_day(date),
            Self::WeekendsOnly(cal) => cal.impl_is_business_day(date),
            Self::TARGET(cal) => cal.impl_is_business_day(date),
            Self::UnitedStates(cal) => cal.impl_is_business_day(date),
            Self::Brazil(cal) => cal.impl_is_business_day(date),
            Self::Chile(cal) => cal.impl_is_business_day(date),
            Self::UnitedKingdom(cal) => cal.impl_is_business_day(date),
            Self::Japan(cal) => cal.impl_is_business_day(date),
            Self::JointCalendar(cal) => cal.impl_is_business_day(date),
        }
    }

    fn is_weekend(&self, weekday: &crate::time::enums::Weekday) -> bool {
        match self {
            Self::NullCalendar(cal) => cal.is_weekend(weekday),
            Self::WeekendsOnly(cal) => cal.is_weekend(weekday),
            Self::TARGET(cal) => cal.is_weekend(weekday),
            Self::UnitedStates(cal) => cal.is_weekend(weekday),
            Self::Brazil(cal) => cal.is_weekend(weekday),
            Self::Chile(cal) => cal.is_weekend(weekday),
            Self::UnitedKingdom(cal) => cal.is_weekend(weekday),
            Self::Japan(cal) => cal.is_weekend(weekday),
            Self::JointCalendar(cal) => cal.is_weekend(weekday),
        }
    }

    fn added_holidays(&self) -> HashSet<Date> {
        match self {
            Self::NullCalendar(cal) => cal.added_holidays(),
            Self::WeekendsOnly(cal) => cal.added_holidays(),
            Self::TARGET(cal) => cal.added_holidays(),
            Self::UnitedStates(cal) => cal.added_holidays(),
            Self::Brazil(cal) => cal.added_holidays(),
            Self::Chile(cal) => cal.added_holidays(),
            Self::UnitedKingdom(cal) => cal.added_holidays(),
            Self::Japan(cal) => cal.added_holidays(),
            Self::JointCalendar(cal) => cal.added_holidays(),
        }
    }

    fn removed_holidays(&self) -> HashSet<Date> {
        match self {
            Self::NullCalendar(cal) => cal.removed_holidays(),
            Self::WeekendsOnly(cal) => cal.removed_holidays(),
            Self::TARGET(cal) => cal.removed_holidays(),
            Self::UnitedStates(cal) => cal.removed_holidays(),
            Self::Brazil(cal) => cal.removed_holidays(),
            Self::Chile(cal) => cal.removed_holidays(),
            Self::UnitedKingdom(cal) => cal.removed_holidays(),
            Self::Japan(cal) => cal.removed_holidays(),
            Self::JointCalendar(cal) => cal.removed_holidays(),
        }
    }

    fn add_holiday(&mut self, date: Date) {
        match self {
            Self::NullCalendar(cal) => cal.add_holiday(date),
            Self::WeekendsOnly(cal) => cal.add_holiday(date),
            Self::TARGET(cal) => cal.add_holiday(date),
            Self::UnitedStates(cal) => cal.add_holiday(date),
            Self::Brazil(cal) => cal.add_holiday(date),
            Self::Chile(cal) => cal.add_holiday(date),
            Self::UnitedKingdom(cal) => cal.add_holiday(date),
            Self::Japan(cal) => cal.add_holiday(date),
            Self::JointCalendar(cal) => cal.add_holiday(date),
        }
    }

    fn remove_holiday(&mut self, date: Date) {
        match self {
            Self::NullCalendar(cal) => cal.remove_holiday(date),
            Self::WeekendsOnly(cal) => cal.remove_holiday(date),
            Self::TARGET(cal) => cal.remove_holiday(date),
            Self::UnitedStates(cal) => cal.remove_holiday(date),
            Self::Brazil(cal) => cal.remove_holiday(date),
            Self::Chile(cal) => cal.remove_holiday(date),
            Self::UnitedKingdom(cal) => cal.remove_holiday(date),
            Self::Japan(cal) => cal.remove_holiday(date),
            Self::JointCalendar(cal) => cal.remove_holiday(date),
        }
    }

    fn holiday_list(&self, from: Date, to: Date, include_weekends: bool) -> Vec<Date> {
        match self {
            Self::NullCalendar(cal) => cal.holiday_list(from, to, include_weekends),
            Self::WeekendsOnly(cal) => cal.holiday_list(from, to, include_weekends),
            Self::TARGET(cal) => cal.holiday_list(from, to, include_weekends),
            Self::UnitedStates(cal) => cal.holiday_list(from, to, include_weekends),
            Self::Brazil(cal) => cal.holiday_list(from, to, include_weekends),
            Self::Chile(cal) => cal.holiday_list(from, to, include_weekends),
            Self::UnitedKingdom(cal) => cal.holiday_list(from, to, include_weekends),
            Self::Japan(cal) => cal.holiday_list(from, to, include_weekends),
            Self::JointCalendar(cal) => cal.holiday_list(from, to, include_weekends),
        }
    }

    fn business_day_list(&self, from: Date, to: Date) -> Vec<Date> {
        match self {
            Self::NullCalendar(cal) => cal.business_day_list(from, to),
            Self::WeekendsOnly(cal) => cal.business_day_list(from, to),
            Self::TARGET(cal) => cal.business_day_list(from, to),
            Self::UnitedStates(cal) => cal.business_day_list(from, to),
            Self::Brazil(cal) => cal.business_day_list(from, to),
            Self::Chile(cal) => cal.business_day_list(from, to),
            Self::UnitedKingdom(cal) => cal.business_day_list(from, to),
            Self::Japan(cal) => cal.business_day_list(from, to),
            Self::JointCalendar(cal) => cal.business_day_list(from, to),
        }
    }
}

impl IsCalendar for Calendar {}

#[cfg(test)]
mod test {
    use crate::time::{
        calendar::Calendar,
        calendars::{
            brazil::Brazil, chile::Chile, nullcalendar::NullCalendar, target::TARGET,
            traits::ImplCalendar, unitedstates::UnitedStates, weekendsonly::WeekendsOnly,
        },
    };

    #[test]
    fn test_create_calendar() {
        let calendar = Calendar::NullCalendar(NullCalendar::new());
        assert_eq!(calendar.impl_name(), "NullCalendar");
        let calendar = Calendar::WeekendsOnly(WeekendsOnly::new());
        assert_eq!(calendar.impl_name(), "WeekendsOnly");
        let calendar = Calendar::TARGET(TARGET::new());
        assert_eq!(calendar.impl_name(), "TARGET");
        let calendar = Calendar::UnitedStates(UnitedStates::default());
        assert_eq!(calendar.impl_name(), "UnitedStates(Sofr)");
        let calendar = Calendar::Brazil(Brazil::default());
        assert_eq!(calendar.impl_name(), "Brazil(Settlement)");
        let calendar = Calendar::Chile(Chile::default());
        assert_eq!(calendar.impl_name(), "Chile(SSE)");
    }

    #[test]
    fn calendar_strings_have_one_canonical_spelling() {
        for name in [
            "NullCalendar",
            "WeekendsOnly",
            "TARGET",
            "UnitedStates(Sofr)",
            "UnitedStates(Settlement)",
            "UnitedStates(LiborImpact)",
            "UnitedStates(Nyse)",
            "UnitedStates(GovernmentBond)",
            "UnitedStates(Nerc)",
            "UnitedStates(FederalReserve)",
            "Brazil(Settlement)",
            "Brazil(Exchange)",
            "Chile(SSE)",
            "Japan",
            "UnitedKingdom(Settlement)",
            "JointCalendar(JoinHolidays,UnitedKingdom(Settlement),Japan)",
            "JointCalendar(JoinBusinessDays,TARGET,JointCalendar(JoinHolidays,Japan,UnitedStates(Sofr)))",
        ] {
            let calendar = Calendar::try_from(name.to_string()).unwrap();
            assert_eq!(String::from(calendar.clone()), name);
            let json = serde_json::to_string(&calendar).unwrap();
            assert_eq!(serde_json::from_str::<Calendar>(&json).unwrap(), calendar);
        }
        for alias in [
            "UnitedStates",
            "Brazil",
            "Chile",
            "SOFR",
            "Sofr",
            "target",
            "JointCalendar(JoinHolidays,Japan)",
            "JointCalendar(Other,Japan,TARGET)",
            "JointCalendar(JoinHolidays,Japan,TARGET))",
            "JointCalendar(JoinHolidays,,Japan)",
        ] {
            assert!(Calendar::try_from(alias.to_string()).is_err());
            assert!(serde_json::from_value::<Calendar>(serde_json::json!(alias)).is_err());
        }
    }
}

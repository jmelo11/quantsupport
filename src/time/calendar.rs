use serde::Serialize;

use super::calendars::{
    brazil::Brazil,
    chile::Chile,
    nullcalendar::NullCalendar,
    target::TARGET,
    traits::{ImplCalendar, IsCalendar},
    unitedstates::UnitedStates,
    weekendsonly::WeekendsOnly,
};
use crate::{
    time::date::Date,
    utils::errors::{QSError, Result},
};
use std::collections::HashSet;

/// # `Calendar`
/// A calendar.
///
/// ## Enums
/// * `NullCalendar` - A calendar that considers all days as business days.
/// * `WeekendsOnly` - A calendar that considers only weekends as business days.
/// * `TARGET` - A calendar that considers only TARGET business days as business days.
/// * `UnitedStates` - A calendar for the United States.
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
        match s.as_str() {
            "NullCalendar" => Ok(Self::NullCalendar(NullCalendar::new())),
            "WeekendsOnly" => Ok(Self::WeekendsOnly(WeekendsOnly::new())),
            "TARGET" => Ok(Self::TARGET(TARGET::new())),
            "UnitedStates" => Ok(Self::UnitedStates(UnitedStates::default())),
            "Brazil" => Ok(Self::Brazil(Brazil::default())),
            "Chile" => Ok(Self::Chile(Chile::default())),
            _ => Err(serde::de::Error::custom(format!("Invalid calendar: {s}"))),
        }
    }
}

impl TryFrom<String> for Calendar {
    type Error = QSError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "NullCalendar" => Ok(Self::NullCalendar(NullCalendar::new())),
            "WeekendsOnly" => Ok(Self::WeekendsOnly(WeekendsOnly::new())),
            "TARGET" => Ok(Self::TARGET(TARGET::new())),
            "UnitedStates" => Ok(Self::UnitedStates(UnitedStates::default())),
            "Brazil" => Ok(Self::Brazil(Brazil::default())),
            "Chile" => Ok(Self::Chile(Chile::default())),
            _ => Err(QSError::InvalidValueErr(format!(
                "Invalid calendar: {s}"
            ))),
        }
    }
}

impl From<Calendar> for String {
    fn from(calendar: Calendar) -> Self {
        match calendar {
            Calendar::NullCalendar(_) => "NullCalendar".to_string(),
            Calendar::WeekendsOnly(_) => "WeekendsOnly".to_string(),
            Calendar::TARGET(_) => "TARGET".to_string(),
            Calendar::UnitedStates(_) => "UnitedStates".to_string(),
            Calendar::Brazil(_) => "Brazil".to_string(),
            Calendar::Chile(_) => "Chile".to_string(),
        }
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
}

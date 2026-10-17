//! UTC instants backed by chrono, complementing calendar-only dates.

use std::{fmt::Display, ops::Sub, str::FromStr};

use chrono::{Datelike, NaiveDate, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// A date and time identifying a UTC instant, with nanosecond precision.
///
/// Offset-aware input is normalized to UTC. Calendar [`Date`] values remain
/// distinct from instants: converting to a date uses the UTC calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DateTime {
    base_datetime: chrono::DateTime<Utc>,
}

impl DateTime {
    /// Creates a UTC instant from calendar and clock components.
    ///
    /// # Errors
    /// Returns an error for invalid date or clock components.
    pub fn new(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> Result<Self> {
        NaiveDate::from_ymd_opt(year, month, day)
            .and_then(|date| date.and_hms_opt(hour, minute, second))
            .map(|datetime| Self::from(datetime.and_utc()))
            .ok_or_else(|| QSError::InvalidValueErr("Invalid UTC date or time components".into()))
    }

    /// Combines a calendar date and UTC clock time.
    ///
    /// # Errors
    /// Returns an error for invalid clock components.
    pub fn from_date(date: Date, hour: u32, minute: u32, second: u32) -> Result<Self> {
        date.base_date()
            .and_hms_opt(hour, minute, second)
            .map(|datetime| Self::from(datetime.and_utc()))
            .ok_or_else(|| QSError::InvalidValueErr("Invalid UTC time components".into()))
    }

    /// Parses an offset-aware datetime using chrono's strftime format syntax.
    ///
    /// # Errors
    /// Returns an error if the input is invalid or does not specify an offset.
    pub fn from_str(datetime: &str, fmt: &str) -> Result<Self> {
        Ok(Self::from(chrono::DateTime::parse_from_str(datetime, fmt)?))
    }

    /// Formats this UTC instant using chrono's strftime format syntax.
    #[must_use]
    pub fn to_str(&self, fmt: &str) -> String {
        self.base_datetime.format(fmt).to_string()
    }

    /// Returns the underlying chrono UTC datetime.
    #[must_use]
    pub const fn base_datetime(&self) -> chrono::DateTime<Utc> {
        self.base_datetime
    }

    /// Returns the UTC calendar date.
    #[must_use]
    pub fn date(&self) -> Date {
        Date::from(self.base_datetime.date_naive())
    }

    /// Returns the UTC calendar year.
    #[must_use]
    pub fn year(&self) -> i32 {
        self.base_datetime.year()
    }

    /// Returns the UTC calendar month (1-12).
    #[must_use]
    pub fn month(&self) -> u32 {
        self.base_datetime.month()
    }

    /// Returns the UTC day of the month (1-31).
    #[must_use]
    pub fn day(&self) -> u32 {
        self.base_datetime.day()
    }

    /// Returns the UTC hour (0-23).
    #[must_use]
    pub fn hour(&self) -> u32 {
        self.base_datetime.hour()
    }

    /// Returns the UTC minute (0-59).
    #[must_use]
    pub fn minute(&self) -> u32 {
        self.base_datetime.minute()
    }

    /// Returns the UTC second (0-59).
    #[must_use]
    pub fn second(&self) -> u32 {
        self.base_datetime.second()
    }

    /// Returns the fractional second in nanoseconds, including chrono leap seconds.
    #[must_use]
    pub fn nanosecond(&self) -> u32 {
        self.base_datetime.nanosecond()
    }
}

impl<Zone: TimeZone> From<chrono::DateTime<Zone>> for DateTime {
    fn from(datetime: chrono::DateTime<Zone>) -> Self {
        Self {
            base_datetime: datetime.with_timezone(&Utc),
        }
    }
}

impl FromStr for DateTime {
    type Err = QSError;

    fn from_str(datetime: &str) -> Result<Self> {
        Ok(Self::from(chrono::DateTime::parse_from_rfc3339(datetime)?))
    }
}

impl Display for DateTime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.base_datetime.to_rfc3339())
    }
}

impl Sub for DateTime {
    type Output = chrono::TimeDelta;

    fn sub(self, other: Self) -> Self::Output {
        self.base_datetime - other.base_datetime
    }
}

impl Serialize for DateTime {
    fn serialize<Serializer: serde::Serializer>(
        &self,
        serializer: Serializer,
    ) -> std::result::Result<Serializer::Ok, Serializer::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<Deserializer: serde::Deserializer<'de>>(
        deserializer: Deserializer,
    ) -> std::result::Result<Self, Deserializer::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn components_and_date_conversion() -> TestResult {
        let datetime = DateTime::new(2025, 1, 15, 10, 30, 45)?;
        assert_eq!(datetime.date(), Date::new(2025, 1, 15));
        assert_eq!(
            (datetime.year(), datetime.month(), datetime.day()),
            (2025, 1, 15)
        );
        assert_eq!(
            (datetime.hour(), datetime.minute(), datetime.second()),
            (10, 30, 45)
        );
        assert_eq!(DateTime::from_date(datetime.date(), 10, 30, 45)?, datetime);
        assert!(DateTime::new(2025, 2, 29, 0, 0, 0).is_err());
        assert!(DateTime::new(2025, 1, 15, 24, 0, 0).is_err());
        assert!(DateTime::from_date(datetime.date(), 0, 60, 0).is_err());
        Ok(())
    }

    #[test]
    fn offsets_precision_ordering_and_serialization() -> TestResult {
        let datetime: DateTime = "2025-01-16T00:30:45.123456789+02:00".parse()?;
        assert_eq!(datetime.date(), Date::new(2025, 1, 15));
        assert_eq!(datetime.hour(), 22);
        assert_eq!(datetime.nanosecond(), 123_456_789);
        assert_eq!(datetime.to_string(), "2025-01-15T22:30:45.123456789+00:00");
        assert_eq!(datetime, DateTime::from(datetime.base_datetime()));
        assert_eq!(
            DateTime::from_str("2025-01-15 22:30:45 +0000", "%Y-%m-%d %H:%M:%S %z")?,
            DateTime::new(2025, 1, 15, 22, 30, 45)?
        );
        assert_eq!(datetime.to_str("%Y-%m-%d %H:%M"), "2025-01-15 22:30");
        assert!(datetime > DateTime::new(2025, 1, 15, 22, 30, 45)?);
        let encoded = serde_json::to_string(&datetime)?;
        assert_eq!(serde_json::from_str::<DateTime>(&encoded)?, datetime);
        assert!("2025-01-15T22:30:45".parse::<DateTime>().is_err());
        assert!("2025-01-15".parse::<DateTime>().is_err());
        Ok(())
    }
}

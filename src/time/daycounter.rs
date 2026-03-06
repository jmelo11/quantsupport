use serde::{Deserialize, Serialize};

use super::daycounters::{
    actual360::Actual360,
    actual365::Actual365,
    actualactual::ActualActual,
    business252::Business252,
    thirty360::{Thirty360, Thirty360US},
};
use crate::{
    time::{date::Date, daycounters::daycount::DayCount},
    utils::errors::{QSError, Result},
};

/// # `DayCounter`
/// Day count convention enum.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DayCounter {
    /// Actual/360 day count convention
    Actual360,
    /// Actual/365 day count convention
    Actual365,
    /// 30/360 day count convention
    Thirty360,
    /// 30/360 US day count convention
    Thirty360US,
    /// Actual/Actual day count convention
    ActualActual,
    /// Business/252 day count convention
    Business252,
}

impl DayCounter {
    /// Calculates the day count between two dates using the selected day count convention.
    #[must_use]
    pub fn day_count(&self, start: Date, end: Date) -> i64 {
        match self {
            Self::Actual360 => Actual360::day_count(start, end),
            Self::Actual365 => Actual365::day_count(start, end),
            Self::Thirty360 => Thirty360::day_count(start, end),
            Self::Thirty360US => Thirty360US::day_count(start, end),
            Self::ActualActual => ActualActual::day_count(start, end),
            Self::Business252 => Business252::day_count(start, end),
        }
    }

    /// Calculates the year fraction between two dates using the selected day count convention.
    #[must_use]
    pub fn year_fraction(&self, start: Date, end: Date) -> f64 {
        match self {
            Self::Actual360 => Actual360::year_fraction(start, end),
            Self::Actual365 => Actual365::year_fraction(start, end),
            Self::Thirty360 => Thirty360::year_fraction(start, end),
            Self::Thirty360US => Thirty360US::year_fraction(start, end),
            Self::ActualActual => ActualActual::year_fraction(start, end),
            Self::Business252 => Business252::year_fraction(start, end),
        }
    }
}

impl TryFrom<String> for DayCounter {
    type Error = QSError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "Actual360" => Ok(Self::Actual360),
            "Actual365" => Ok(Self::Actual365),
            "Thirty360" => Ok(Self::Thirty360), // to match curveengine
            "Thirty360US" => Ok(Self::Thirty360US),
            "ActualActual" => Ok(Self::ActualActual),
            "Business252" => Ok(Self::Business252),
            _ => Err(QSError::InvalidValueErr(format!(
                "Invalid day counter: {s}"
            ))),
        }
    }
}

impl From<DayCounter> for String {
    fn from(day_counter: DayCounter) -> Self {
        match day_counter {
            DayCounter::Actual360 => "Actual360".to_string(),
            DayCounter::Actual365 => "Actual365".to_string(),
            DayCounter::Thirty360 => "Thirty360".to_string(),
            DayCounter::Thirty360US => "Thirty360US".to_string(),
            DayCounter::ActualActual => "ActualActual".to_string(),
            DayCounter::Business252 => "Business252".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_day_count_standard() {
        let start = Date::new(2020, 1, 1);
        let end = Date::new(2020, 1, 2);

        let day_count = DayCounter::Actual360.day_count(start, end);
        assert_eq!(day_count, 1);
        let day_count = DayCounter::Actual365.day_count(start, end);
        assert_eq!(day_count, 1);
        let day_count = DayCounter::Thirty360.day_count(start, end);
        assert_eq!(day_count, 1);
        let day_count = DayCounter::Thirty360US.day_count(start, end);
        assert_eq!(day_count, 1);
        let day_count = DayCounter::ActualActual.day_count(start, end);
        assert_eq!(day_count, 1);
    }

    #[test]
    fn test_day_count_standard_inverted() {
        let start = Date::new(2020, 1, 1);
        let end = Date::new(2020, 1, 2);

        let day_count = DayCounter::Actual360.day_count(end, start);
        assert_eq!(day_count, -1);
        let day_count = DayCounter::Actual365.day_count(end, start);
        assert_eq!(day_count, -1);
        let day_count = DayCounter::Thirty360.day_count(end, start);
        assert_eq!(day_count, -1);
        let day_count = DayCounter::Thirty360US.day_count(end, start);
        assert_eq!(day_count, -1);
        let day_count = DayCounter::ActualActual.day_count(end, start);
        assert_eq!(day_count, -1);
    }

    fn almost_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn test_year_fraction() {
        let start = Date::new(2020, 1, 1);
        let end = Date::new(2020, 1, 2);

        let year_fraction = DayCounter::Actual360.year_fraction(start, end);
        assert!(almost_eq(year_fraction, 1.0 / 360.0, 1e-12));
        let year_fraction = DayCounter::Actual365.year_fraction(start, end);
        assert!(almost_eq(year_fraction, 1.0 / 365.0, 1e-12));
        let year_fraction = DayCounter::Thirty360.year_fraction(start, end);
        assert!(almost_eq(year_fraction, 1.0 / 360.0, 1e-12));
        let year_fraction = DayCounter::Thirty360US.year_fraction(start, end);
        assert!(almost_eq(year_fraction, 1.0 / 360.0, 1e-12));
        let year_fraction = DayCounter::ActualActual.year_fraction(start, end);
        assert!(almost_eq(year_fraction, 1.0 / 366.0, 1e-12));
    }

    #[test]
    fn test_year_fraction_inverse() {
        let start = Date::new(2020, 1, 1);
        let end = Date::new(2020, 1, 2);

        let year_fraction = DayCounter::Actual360.year_fraction(end, start);
        assert!(almost_eq(year_fraction, -1.0 / 360.0, 1e-12));
        let year_fraction = DayCounter::Actual365.year_fraction(end, start);
        assert!(almost_eq(year_fraction, -1.0 / 365.0, 1e-12));
        let year_fraction = DayCounter::Thirty360.year_fraction(end, start);
        assert!(almost_eq(year_fraction, -1.0 / 360.0, 1e-12));
        let year_fraction = DayCounter::Thirty360US.year_fraction(end, start);
        assert!(almost_eq(year_fraction, -1.0 / 360.0, 1e-12));
        let year_fraction = DayCounter::ActualActual.year_fraction(end, start);
        assert!(almost_eq(year_fraction, -1.0 / 366.0, 1e-12));
    }

    #[test]
    fn test_year_fraction_trithy360_end_of_month() {
        let start = Date::new(2023, 12, 10);
        let end_1 = Date::new(2023, 12, 30);
        let end_2 = Date::new(2023, 12, 31);

        let yf_1 = DayCounter::Thirty360.year_fraction(start, end_1);
        let yf_2 = DayCounter::Thirty360.year_fraction(start, end_2);
        assert!(!almost_eq(yf_1, yf_2, 1e-12));
    }

    #[test]
    fn test_year_fraction_trithy360_beetween_end_and_start_of_month() {
        let start = Date::new(2023, 12, 10);
        let end_1 = Date::new(2023, 12, 31);
        let end_2 = Date::new(2024, 1, 1);

        let yf_1 = DayCounter::Thirty360.year_fraction(start, end_1);
        let yf_2 = DayCounter::Thirty360.year_fraction(start, end_2);
        assert!(almost_eq(yf_1, yf_2, 1e-12));
    }

    #[test]
    fn test_year_fraction_trithy360_star_in_end_of_month() {
        let start = Date::new(2023, 12, 30);
        let end_1 = Date::new(2023, 12, 31);
        let end_2 = Date::new(2024, 1, 1);

        let yf_1 = DayCounter::Thirty360.year_fraction(start, end_1);
        let yf_2 = DayCounter::Thirty360.year_fraction(start, end_2);
        assert!((yf_1 - yf_2).abs() > 1e-12);
    }

    #[test]
    fn test_year_fraction_trithy360_leap_february() {
        let start = Date::new(2024, 2, 10);
        let end_1 = Date::new(2024, 2, 29);
        let end_2 = Date::new(2024, 3, 1);

        let yf_1 = DayCounter::Thirty360.year_fraction(start, end_1);
        let yf_2 = DayCounter::Thirty360.year_fraction(start, end_2);

        assert!(!almost_eq(yf_1, yf_2, 1e-12));
    }

    #[test]
    fn test_year_fraction_trithy360_february() {
        let start = Date::new(2023, 2, 10);
        let end_1 = Date::new(2023, 2, 28);
        let end_2 = Date::new(2023, 3, 1);

        let yf_1 = DayCounter::Thirty360.year_fraction(start, end_1);
        let yf_2 = DayCounter::Thirty360.year_fraction(start, end_2);

        assert!(!almost_eq(yf_1, yf_2, 1e-12));
    }
}

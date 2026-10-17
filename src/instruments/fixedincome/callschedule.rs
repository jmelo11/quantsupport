//! Issuer call schedules for callable bonds.

use serde::{Deserialize, Serialize};

use crate::{
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// An inclusive issuer call window and strike.
///
/// `strike` is quoted per the bond's units and excludes contractual coupon or
/// amortization cashflows paid on the call date.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallScheduleEntry {
    #[serde(rename = "start_date", alias = "date")]
    date: Date,
    #[serde(default)]
    end_date: Option<Date>,
    strike: f64,
}

impl CallScheduleEntry {
    /// Creates an issuer call date and strike.
    #[must_use]
    pub const fn new(date: Date, strike: f64) -> Self {
        Self {
            date,
            end_date: None,
            strike,
        }
    }

    /// Creates an inclusive dated call window.
    #[must_use]
    pub const fn window(start_date: Date, end_date: Date, strike: f64) -> Self {
        Self {
            date: start_date,
            end_date: Some(end_date),
            strike,
        }
    }

    /// Returns the last eligible exercise date.
    #[must_use]
    pub fn end_date(&self) -> Date {
        self.end_date.unwrap_or(self.date)
    }

    /// Returns whether the given date lies in the call window.
    #[must_use]
    pub fn contains(&self, date: Date) -> bool {
        self.date <= date && date <= self.end_date()
    }

    /// Returns the first eligible exercise date, or the single discrete call date.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }

    /// Returns the strike per bond units.
    #[must_use]
    pub const fn strike(&self) -> f64 {
        self.strike
    }

    /// Checks window ordering and a finite, strictly positive strike.
    ///
    /// # Errors
    /// Returns an error for an end date before the start date, or a strike
    /// that is nonfinite or nonpositive.
    pub fn validate(&self) -> Result<()> {
        if self.end_date() < self.date {
            return Err(QSError::InvalidValueErr(
                "Bond call window end date must not precede its start date".into(),
            ));
        }
        if !self.strike.is_finite() || self.strike <= 0.0 {
            return Err(QSError::InvalidValueErr(format!(
                "Bond call strike must be finite and positive, got {}",
                self.strike
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inclusive_window_and_date_only_compatibility() {
        let start = Date::new(2028, 1, 1);
        let end = Date::new(2028, 6, 1);
        let window = CallScheduleEntry::window(start, end, 100.0);
        assert!(window.validate().is_ok());
        assert!(window.contains(start));
        assert!(window.contains(end));
        assert!(!window.contains(Date::new(2028, 6, 2)));
        assert!(CallScheduleEntry::window(end, start, 100.0)
            .validate()
            .is_err());
        let single: CallScheduleEntry =
            serde_json::from_str(r#"{"date":"2028-01-01","strike":100.0}"#).unwrap();
        assert_eq!(single.end_date(), start);
        let decoded: CallScheduleEntry = serde_json::from_str(
            r#"{"start_date":"2028-01-01","end_date":"2028-06-01","strike":100.0}"#,
        )
        .unwrap();
        assert_eq!(decoded, window);
    }
}

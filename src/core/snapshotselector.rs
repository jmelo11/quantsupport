//! Timestamp and quote-side selection for historical market snapshots.

use serde::{Deserialize, Serialize};

use crate::{quotes::quote::Level, time::datetime::DateTime};

/// Rule for resolving a requested market timestamp.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotSelectionRule {
    /// Requires the requested instant to exist.
    Exact,
    /// Uses the nearest instant, preferring the earlier one on a tie.
    /// This rule can select data after the requested timestamp.
    Closest,
    /// Uses the latest instant at or before the requested timestamp.
    #[default]
    Previous,
}

/// Selects one coherent market snapshot and its quote side.
///
/// Timestamps are UTC instants; serialized timestamps accept RFC 3339 offsets.
/// Selection does not roll curves or change the selected snapshot's pricing date.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SnapshotSelector {
    timestamp: DateTime,
    #[serde(default)]
    rule: SnapshotSelectionRule,
    #[serde(default)]
    level: Level,
}

impl SnapshotSelector {
    /// Requests the last available snapshot at or before this instant, at mid.
    #[must_use]
    pub const fn new(timestamp: DateTime) -> Self {
        Self {
            timestamp,
            rule: SnapshotSelectionRule::Previous,
            level: Level::Mid,
        }
    }

    /// Sets the timestamp matching rule.
    #[must_use]
    pub const fn with_rule(mut self, rule: SnapshotSelectionRule) -> Self {
        self.rule = rule;
        self
    }

    /// Sets the quote side used to construct the selected market.
    #[must_use]
    pub const fn with_level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Returns the requested UTC instant.
    #[must_use]
    pub const fn timestamp(&self) -> DateTime {
        self.timestamp
    }

    /// Returns the timestamp matching rule.
    #[must_use]
    pub const fn rule(&self) -> SnapshotSelectionRule {
        self.rule
    }

    /// Returns the requested quote side.
    #[must_use]
    pub const fn level(&self) -> Level {
        self.level
    }

    pub(crate) fn matches(
        &self,
        earlier: Option<DateTime>,
        later: Option<DateTime>,
    ) -> Option<DateTime> {
        match self.rule {
            SnapshotSelectionRule::Exact => earlier.filter(|instant| *instant == self.timestamp),
            SnapshotSelectionRule::Previous => earlier,
            SnapshotSelectionRule::Closest => match (earlier, later) {
                (Some(before), Some(after)) => {
                    Some(if self.timestamp - before <= after - self.timestamp {
                        before
                    } else {
                        after
                    })
                }
                (before, after) => before.or(after),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::errors::Result;

    fn instant(hour: u32) -> DateTime {
        chrono::NaiveDate::from_ymd_opt(2025, 1, 15)
            .and_then(|date| date.and_hms_opt(hour, 0, 0))
            .unwrap_or_else(|| panic!("valid test timestamp"))
            .and_utc()
            .into()
    }

    #[test]
    fn selection_rules_and_ties() {
        let selector = SnapshotSelector::new(instant(12));
        assert_eq!(
            selector.matches(Some(instant(10)), Some(instant(13))),
            Some(instant(10))
        );
        assert_eq!(selector.matches(None, Some(instant(13))), None);
        let exact = selector.with_rule(SnapshotSelectionRule::Exact);
        assert_eq!(exact.matches(Some(instant(10)), Some(instant(13))), None);
        assert_eq!(
            exact.matches(Some(instant(12)), Some(instant(12))),
            Some(instant(12))
        );
        let closest = selector.with_rule(SnapshotSelectionRule::Closest);
        assert_eq!(
            closest.matches(Some(instant(10)), Some(instant(13))),
            Some(instant(13))
        );
        assert_eq!(
            closest.matches(Some(instant(10)), Some(instant(14))),
            Some(instant(10))
        );
        assert_eq!(closest.matches(None, Some(instant(13))), Some(instant(13)));
        assert_eq!(closest.matches(Some(instant(10)), None), Some(instant(10)));
        assert_eq!(closest.matches(None, None), None);
    }

    #[test]
    fn serde_defaults_and_timezone_normalization() -> Result<()> {
        let selector: SnapshotSelector = serde_json::from_str(
            r#"{"timestamp":"2025-01-15T14:00:00+02:00"}"#,
        )
        .map_err(|error| crate::utils::errors::QSError::InvalidValueErr(error.to_string()))?;
        assert_eq!(selector.timestamp(), instant(12));
        assert_eq!(selector.rule(), SnapshotSelectionRule::Previous);
        assert_eq!(selector.level(), Level::Mid);
        Ok(())
    }
}

use xll_rs::types::XllError;
use xllgen::xll_bindgen;

use crate::{functions::helpers::value_error, runtime::historical};

#[xll_bindgen(
    name = "QS.DATETIME.PARSE",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Normalizes an offset-aware RFC 3339 timestamp to UTC, preserving nanoseconds"
)]
pub fn qs_datetime_parse(timestamp: &str) -> Result<String, XllError> {
    historical::timestamp(timestamp)
        .map(|instant| instant.to_string())
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.DATETIME.DATE",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Returns the UTC calendar date of an offset-aware timestamp, not its market business date"
)]
pub fn qs_datetime_date(timestamp: &str) -> Result<String, XllError> {
    historical::timestamp(timestamp)
        .map(|instant| instant.date().to_string())
        .map_err(value_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_offsets_and_rejects_dates_without_timezones() {
        assert_eq!(
            qs_datetime_parse("2026-01-16T00:30:00.123456789+02:00").unwrap(),
            "2026-01-15T22:30:00.123456789+00:00"
        );
        assert_eq!(
            qs_datetime_date("2026-01-16T00:30:00+02:00").unwrap(),
            "2026-01-15"
        );
        for invalid in ["2026-01-15", "2026-01-15T16:00:00", "invalid"] {
            assert!(qs_datetime_parse(invalid).is_err());
            assert!(qs_datetime_date(invalid).is_err());
        }
    }
}

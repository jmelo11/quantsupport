use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};

use crate::{
    functions::helpers::{optional_string_values, string_column, value_error},
    runtime::{
        historical,
        registry::{with_registry, with_registry_mut},
    },
};

#[xll_bindgen(
    name = "QS.SNAPSHOT.FROM.CONTEXT",
    category = "QuantSupport - Market",
    help = "Captures serializable base market inputs from a context at an observation timestamp; excludes fixings and temporary scenarios"
)]
pub fn qs_snapshot_from_context(
    name: &str,
    timestamp: &str,
    context: &str,
    inputs: *const XLOPER12,
) -> Result<String, XllError> {
    let instant = historical::timestamp(timestamp).map_err(value_error)?;
    let inputs = optional_string_values(inputs, "inputs")?;
    with_registry_mut(|registry| {
        historical::capture_inputs(registry, name, instant, context, &inputs)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SNAPSHOTS.CREATE",
    category = "QuantSupport - Market",
    help = "Builds a complete snapshot archive from a range of snapshot handles; changing or removing rows rebuilds it"
)]
pub fn qs_snapshots_create(name: &str, snapshots: *const XLOPER12) -> Result<String, XllError> {
    let handles = optional_string_values(snapshots, "snapshots")?;
    with_registry_mut(|registry| historical::archive(registry, name, &handles)).map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SNAPSHOT.SELECTOR",
    category = "QuantSupport - Market",
    help = "Creates a native timestamp and quote-side selector; defaults to Previous and Mid"
)]
pub fn qs_snapshot_selector(
    name: &str,
    timestamp: &str,
    rule: Option<String>,
    quote_level: Option<String>,
) -> Result<String, XllError> {
    let selector = historical::selector(timestamp, rule.as_deref(), quote_level.as_deref())
        .map_err(value_error)?;
    with_registry_mut(|registry| historical::store_selector(registry, name, selector))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SNAPSHOTS.SELECT",
    category = "QuantSupport - Market",
    help = "Selects one complete market from an archive; inspect its timestamp for the actual match"
)]
pub fn qs_snapshots_select(name: &str, store: &str, selector: &str) -> Result<String, XllError> {
    with_registry_mut(|registry| historical::select(registry, name, store, selector))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SNAPSHOT.TIMESTAMP",
    category = "QuantSupport - Market",
    help = "Returns the snapshot's actual observation timestamp as UTC RFC 3339 text"
)]
pub fn qs_snapshot_timestamp(snapshot: &str) -> Result<String, XllError> {
    with_registry(|registry| {
        registry
            .market_snapshot(snapshot)
            .map(|value| value.timestamp().to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SNAPSHOT.REFERENCE.DATE",
    category = "QuantSupport - Market",
    help = "Returns the snapshot's market business date, which is the selected context's valuation date"
)]
pub fn qs_snapshot_reference_date(snapshot: &str) -> Result<String, XllError> {
    with_registry(|registry| {
        registry
            .market_snapshot(snapshot)
            .map(|value| value.quotes().reference_date().to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SNAPSHOTS.TIMESTAMPS",
    category = "QuantSupport - Market",
    help = "Spills the archive's observation timestamps in ascending UTC order"
)]
pub fn qs_snapshots_timestamps(
    store: &str,
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let timestamps: Vec<String> = with_registry(|registry| {
        registry.market_snapshot_store(store).map(|value| {
            value
                .timestamps()
                .into_iter()
                .map(|instant| instant.to_string())
                .collect()
        })
    })
    .map_err(value_error)?;
    if timestamps.is_empty() {
        return Ok(xll_rs::convert::build_multi(vec![XLOPER12::nil()], 1, 1));
    }
    Ok(string_column(timestamps, transpose))
}

#[xll_bindgen(
    name = "QS.SNAPSHOTS.FROM.JSON",
    category = "QuantSupport - Market",
    help = "Loads the native snapshot JSON array shared with Python and Rust; duplicate timestamps are rejected"
)]
pub fn qs_snapshots_from_json(name: &str, json: &str) -> Result<String, XllError> {
    with_registry_mut(|registry| historical::from_json(registry, name, json)).map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SNAPSHOTS.TO.JSON",
    category = "QuantSupport - Market",
    help = "Serializes native snapshot inputs as JSON text; errors if Excel's 32767 UTF-16-unit cell limit is exceeded"
)]
pub fn qs_snapshots_to_json(store: &str) -> Result<String, XllError> {
    let json =
        with_registry(|registry| historical::to_json(registry, store)).map_err(value_error)?;
    if json.encode_utf16().count() > 32_767 {
        return Err(value_error(
            "Snapshot JSON exceeds Excel's 32767-character cell limit",
        ));
    }
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::registry::QsObject;
    use quantsupport::prelude::{Date, PricingContext, QuoteStore};

    #[test]
    fn worksheet_archive_accepts_handle_ranges_and_reports_actual_match() {
        let context =
            PricingContext::new().with_quote_store(QuoteStore::new(Date::new(2026, 1, 15)));
        let context = with_registry_mut(|registry| {
            registry.upsert_if_changed(
                "worksheet_market",
                QsObject::PricingContext(std::rc::Rc::new(context)),
                "2026-01-15",
            )
        })
        .unwrap();
        let snapshot = qs_snapshot_from_context(
            "worksheet_snapshot",
            "2026-01-15T16:00:00Z",
            &context,
            &XLOPER12::nil(),
        )
        .unwrap();
        let range = xll_rs::convert::build_multi(
            vec![XLOPER12::from_str(&snapshot), XLOPER12::nil()],
            2,
            1,
        );
        let store = qs_snapshots_create("worksheet_archive", range).unwrap();
        xll_rs::memory::xlAutoFree12(range);
        let selector =
            qs_snapshot_selector("worksheet_selector", "2026-01-16T16:00:00Z", None, None).unwrap();
        let selected = qs_snapshots_select("worksheet_selected", &store, &selector).unwrap();
        assert_eq!(
            qs_snapshot_timestamp(&selected).unwrap(),
            "2026-01-15T16:00:00+00:00"
        );
        assert_eq!(qs_snapshot_reference_date(&selected).unwrap(), "2026-01-15");
        let duplicate = xll_rs::convert::build_multi(
            vec![XLOPER12::from_str(&snapshot), XLOPER12::from_str(&snapshot)],
            2,
            1,
        );
        assert!(qs_snapshots_create("worksheet_duplicates", duplicate).is_err());
        xll_rs::memory::xlAutoFree12(duplicate);
        let empty = qs_snapshots_create("worksheet_archive", &XLOPER12::nil()).unwrap();
        assert!(qs_snapshots_select("worksheet_selected", &empty, &selector).is_err());
        let blank_range =
            xll_rs::convert::build_multi(vec![XLOPER12::nil(), XLOPER12::from_str("")], 2, 1);
        let empty_range = qs_snapshots_create("worksheet_archive", blank_range).unwrap();
        xll_rs::memory::xlAutoFree12(blank_range);
        assert_eq!(empty_range, empty);
        assert!(qs_snapshots_create("worksheet_bad", &XLOPER12::from_f64(1.0)).is_err());
    }
}

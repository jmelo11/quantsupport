use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};

use crate::runtime::registry::{with_registry, with_registry_mut};

use crate::functions::helpers::{not_found, value_error};

#[xll_bindgen(
    name = "QS.ENGINE.STATUS",
    category = "QuantSupport - System",
    help = "Returns explicit background job completion and stage metrics as JSON; CalculationState is not a completion barrier"
)]
pub fn qs_engine_status() -> Result<String, XllError> {
    Ok(crate::runtime::engine::bridge::status())
}

#[xll_bindgen(
    name = "QS.ENGINE.PROBE",
    category = "QuantSupport - System",
    help = "Tests asynchronous scalar or spill delivery with a synthetic background delay in seconds"
)]
pub fn qs_evaluate_probe(seconds: f64, rows: i32) -> Result<*mut XLOPER12, XllError> {
    if !seconds.is_finite() || !(0.0..=30.0).contains(&seconds) || !(1..=10000).contains(&rows) {
        return Err(value_error("seconds must be 0..30 and rows 1..10000"));
    }
    std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
    if rows == 1 {
        return Ok(xll_rs::returning::XlReturn::num(42.0).into_raw());
    }
    Ok(crate::functions::helpers::spill(
        (0..rows)
            .map(|row| XLOPER12::from_f64(42.0 + f64::from(row)))
            .collect(),
        rows as usize,
        1,
        None,
        None,
    ))
}

#[xll_bindgen(
    name = "QS.OBJECT.TYPE",
    category = "QuantSupport - System",
    help = "Returns the type stored behind a QuantSupport object handle"
)]
pub fn qs_object_type(handle: &str) -> Result<String, XllError> {
    if let Some(kind) = crate::runtime::engine::bridge::opaque_kind(handle) {
        return Ok(kind);
    }
    with_registry(|registry| registry.object_type(handle).map(str::to_string)).map_err(value_error)
}

#[xll_bindgen(
    name = "QS.OBJECT.COUNT",
    category = "QuantSupport - System",
    help = "Counts live objects in this workbook, or all open workbooks when TRUE"
)]
pub fn qs_object_count(all_workbooks: *const XLOPER12) -> Result<i32, XllError> {
    let all_workbooks =
        crate::functions::helpers::optional_bool(all_workbooks, "all_workbooks")?.unwrap_or(false);
    Ok(
        i32::try_from(crate::runtime::registry::live_object_count(all_workbooks))
            .unwrap_or(i32::MAX),
    )
}

#[xll_bindgen(
    name = "QS.OBJECT.REMOVE",
    category = "QuantSupport - System",
    help = "Removes the object referenced by a current handle"
)]
pub fn qs_object_remove(handle: &str) -> Result<bool, XllError> {
    with_registry_mut(|registry| registry.remove(handle)).map_err(not_found)
}

use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};

use crate::runtime::registry::{with_registry, with_registry_mut};

use crate::functions::helpers::{not_found, value_error};

#[xll_bindgen(
    name = "QS.OBJECT.TYPE",
    category = "QuantSupport - System",
    help = "Returns the type stored behind a QuantSupport object handle"
)]
pub fn qs_object_type(handle: &str) -> Result<String, XllError> {
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

#[xll_bindgen(
    name = "QS.OBJECT.RESET",
    aliases = ["QS.STORE.RESET"],
    category = "QuantSupport - System",
    help = "Clears a mutable store or configuration collection when the reset key changes and returns its new handle"
)]
pub fn qs_object_reset(handle: &str, reset_key: &str) -> Result<String, XllError> {
    with_registry_mut(|registry| registry.reset_if_changed(handle, reset_key)).map_err(value_error)
}

use crate::functions::helpers::{cached_handle, parse_json_list, value_error};
use crate::runtime::registry::{with_registry_mut, QsObject};
use quantsupport::prelude::Scenario;
use xll_rs::types::XllError;
use xllgen::xll_bindgen;

#[xll_bindgen(
    name = "QS.SCENARIOS.FROM.JSON",
    volatile,
    category = "QuantSupport - Risk",
    help = "Stores native QuantSupport Scenario values parsed from JSON"
)]
pub fn qs_scenarios_from_json(name: &str, json: &str) -> Result<String, XllError> {
    if let Some(handle) = cached_handle(name, "Scenarios", json)? {
        return Ok(handle);
    }
    let values: Vec<Scenario> = parse_json_list(json, "Scenario values")?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Scenarios(values), json)
    })
    .map_err(value_error)
}

use crate::functions::helpers::{cached_handle, parse_json_list, value_error};
use crate::runtime::registry::{with_registry_mut, QsObject};
use quantsupport::prelude::PricerConfiguration;
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::XllError;

#[xll_bindgen(
    name = "QS.PRICER.CONFIGS.FROM.JSON",
    category = "QuantSupport - Pricing",
    help = "Stores native QuantSupport PricerConfiguration values parsed from JSON"
)]
pub fn qs_pricer_configs_from_json(name: &str, json: &str) -> Result<String, XllError> {
    if let Some(handle) = cached_handle(name, "PricerConfigurations", json)? {
        return Ok(handle);
    }
    let values: Vec<PricerConfiguration> = parse_json_list(json, "PricerConfiguration values")?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::PricerConfigurations(values), json)
    })
    .map_err(value_error)
}

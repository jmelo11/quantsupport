//! Configured native pricers and named loan pricing settings.
use crate::{
    functions::helpers::*,
    runtime::{
        inputs::Cell,
        pricing::{self, PricerObject},
        registry::{with_registry, with_registry_mut, QsObject},
    },
};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};
#[xll_bindgen(
    name = "QS.LOAN.PRICER",
    category = "QuantSupport - Pricing",
    help = "Creates a loan pricer with explicit named discount, refinancing and fair-rate amortization parameters"
)]
pub fn qs_loan_pricer(
    name: &str,
    currency: &str,
    discount_index: &str,
    refinancing_index: Option<String>,
    rebuild_amortization: *const XLOPER12,
) -> Result<String, XllError> {
    let rebuild = optional_bool(rebuild_amortization, "rebuild_amortization")?.unwrap_or(false);
    let key = input_key(&(currency, discount_index, &refinancing_index, rebuild))?;
    if let Some(handle) = cached_handle(name, "Pricer", &key)? {
        return Ok(handle);
    }
    let policy = pricing::fixed_income_policy(
        &vec![vec![
            Cell::Text(currency.into()),
            Cell::Text(discount_index.into()),
        ]],
        false,
    )
    .map_err(value_error)?;
    let mut settings = vec![vec![
        Cell::Text("Rebuild amortization for fair rate".into()),
        Cell::Boolean(rebuild),
    ]];
    if let Some(index) = refinancing_index.filter(|s| !s.trim().is_empty()) {
        settings.push(vec![
            Cell::Text("Refinancing benchmark index".into()),
            Cell::Text(index),
        ]);
    }
    let pricer = PricerObject::new("LoanPricer", Some(&policy), &settings).map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Pricer(Box::new(pricer)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.PRICER.CREATE",
    category = "QuantSupport - Pricing",
    help = "Creates an explicit native pricer with independent optional setting-name and setting-value arrays"
)]
pub fn qs_pricer_create(
    name: &str,
    implementation: &str,
    discount_policy: Option<String>,
    setting_names: *const XLOPER12,
    setting_values: *const XLOPER12,
) -> Result<String, XllError> {
    let settings = worksheet_columns(&[
        ("setting_names", setting_names),
        ("setting_values", setting_values),
    ])?;
    let policy = discount_policy.as_deref().filter(|s| !s.trim().is_empty());
    let key = input_key(&(implementation, policy, &settings))?;
    with_registry(|registry| {
        policy
            .map(|handle| registry.discount_policy(handle))
            .transpose()
            .map(|_| ())
    })
    .map_err(value_error)?;
    if let Some(handle) = cached_handle(name, "Pricer", &key)? {
        return Ok(handle);
    }
    let pricer = with_registry(|registry| {
        let policy = policy
            .map(|handle| registry.discount_policy(handle).map_err(|e| e.to_string()))
            .transpose()?;
        PricerObject::new(implementation, policy, &settings)
    })
    .map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Pricer(Box::new(pricer)), key)
    })
    .map_err(value_error)
}

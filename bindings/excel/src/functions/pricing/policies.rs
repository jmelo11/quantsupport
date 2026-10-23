//! Native discount policies select ordinary context curve indices.
use crate::{
    functions::helpers::*,
    runtime::{
        inputs::Cell,
        pricing::{self, PolicyObject},
        registry::{with_registry_mut, QsObject},
    },
};
use quantsupport::prelude::SingleCurveCSADiscountPolicy;
use quantsupport_xll_macros::xll_bindgen;
use std::sync::Arc;
use xll_rs::types::{XllError, XLOPER12};
#[xll_bindgen(
    name = "QS.DISCOUNT.POLICY.FIXED.INCOME",
    category = "QuantSupport - Pricing",
    help = "Creates a native fixed-income discount policy from a currency and an ordinary discount-curve index"
)]
pub fn qs_discount_policy_fixed_income(
    name: &str,
    currency: *const XLOPER12,
    market_index: *const XLOPER12,
    prefer_instrument_index: *const XLOPER12,
) -> Result<String, XllError> {
    let mappings = worksheet_columns(&[("currency", currency), ("market_index", market_index)])?;
    fixed_income_mappings(
        name,
        mappings,
        optional_bool(prefer_instrument_index, "prefer_instrument_index")?.unwrap_or(false),
    )
}

fn fixed_income_mappings(
    name: &str,
    mappings: Vec<Vec<Cell>>,
    prefer: bool,
) -> Result<String, XllError> {
    let key = input_key(&(&mappings, prefer))?;
    if let Some(handle) = cached_handle(name, "FixedIncomeDiscountPolicy", &key)? {
        return Ok(handle);
    }
    let policy = pricing::fixed_income_policy(&mappings, prefer).map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::DiscountPolicy(Box::new(policy)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.DISCOUNT.POLICY.CSA",
    category = "QuantSupport - Pricing",
    help = "Creates a native single-curve CSA discount policy"
)]
pub fn qs_discount_policy_csa(
    name: &str,
    collateral_currency: &str,
    collateral_index: &str,
) -> Result<String, XllError> {
    let key = input_key(&(collateral_currency, collateral_index))?;
    let policy = PolicyObject {
        native: Arc::new(SingleCurveCSADiscountPolicy::new(
            parse_index(collateral_index)?,
            parse_currency(collateral_currency)?,
        )),
        kind: "SingleCurveCSADiscountPolicy",
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::DiscountPolicy(Box::new(policy)), key)
    })
    .map_err(value_error)
}

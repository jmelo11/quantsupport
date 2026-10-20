//! Worksheet objects mirror native contracts, positions, policies, pricers and results.
use crate::{
    functions::helpers::*,
    runtime::{
        inputs::Cell,
        pricing::{self, PolicyObject, PricerObject},
        registry::{with_registry, with_registry_mut, QsObject},
    },
};
use quantsupport::prelude::SingleCurveCSADiscountPolicy;
use std::sync::Arc;
use xll_rs::types::{XllError, XLOPER12};
use xllgen::xll_bindgen;

#[xll_bindgen(
    name = "QS.DISCOUNT.POLICY.FIXED.INCOME",
    volatile,
    category = "QuantSupport - Pricing",
    help = "Creates a native fixed-income discount policy from Currency/Index worksheet mappings"
)]
pub fn qs_discount_policy_fixed_income(
    name: &str,
    mappings: *const XLOPER12,
    prefer_instrument_index: *const XLOPER12,
) -> Result<String, XllError> {
    let mappings = worksheet_table(mappings, "mappings")?;
    let prefer =
        optional_bool(prefer_instrument_index, "prefer_instrument_index")?.unwrap_or(false);
    let key = input_key(&(&mappings, prefer))?;
    let policy = pricing::fixed_income_policy(&mappings, prefer).map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::DiscountPolicy(Box::new(policy)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.DISCOUNT.POLICY.CSA",
    volatile,
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

#[xll_bindgen(
    name = "QS.PRICER.CREATE",
    volatile,
    category = "QuantSupport - Pricing",
    help = "Creates an explicit native pricer with a discount-policy handle and optional Setting/Value range"
)]
pub fn qs_pricer_create(
    name: &str,
    implementation: &str,
    discount_policy: Option<String>,
    settings: *const XLOPER12,
) -> Result<String, XllError> {
    let settings = worksheet_table(settings, "settings")?;
    let policy = discount_policy.as_deref().filter(|s| !s.trim().is_empty());
    let key = input_key(&(implementation, policy, &settings))?;
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

#[xll_bindgen(
    name = "QS.REQUEST.FAIR.RATE",
    category = "QuantSupport - Pricing",
    help = "Selects a zero-based swap leg or loan coupon/spread segment for a FairRate request"
)]
pub fn qs_request_fair_rate(index: f64) -> Result<String, XllError> {
    Ok(format!(
        "FairRate({})",
        Cell::Number(index)
            .count("fair-rate target")
            .map_err(value_error)?
    ))
}

#[xll_bindgen(
    name = "QS.EVALUATE",
    volatile,
    category = "QuantSupport - Pricing",
    help = "Evaluates a native trade with an explicit pricer, context and worksheet range of requests"
)]
pub fn qs_evaluate(
    name: &str,
    pricer: &str,
    trade: &str,
    context: &str,
    requests: *const XLOPER12,
) -> Result<String, XllError> {
    let names = string_values(requests, "requests")?;
    let requests = pricing::parse_requests(&names).map_err(value_error)?;
    let key = input_key(&(pricer, trade, context, names))?;
    // Cached output still requires current upstream handles.
    with_registry(|registry| {
        registry.pricer(pricer).map_err(|e| e.to_string())?;
        registry.object(trade).map_err(|e| e.to_string())?;
        registry
            .pricing_context(context)
            .map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })
    .map_err(value_error)?;
    if let Some(handle) = cached_handle(name, "EvaluationResults", &key)? {
        return Ok(handle);
    }
    let results = with_registry(|registry| {
        registry
            .pricer(pricer)
            .map_err(|e| e.to_string())?
            .evaluate(
                registry.object(trade).map_err(|e| e.to_string())?,
                &requests,
                registry
                    .pricing_context(context)
                    .map_err(|e| e.to_string())?,
            )
    })
    .map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::EvaluationResults(Box::new(results)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.RESULT.VALUE",
    category = "QuantSupport - Pricing",
    help = "Present value from stored evaluation results"
)]
pub fn qs_result_value(result: &str) -> Result<f64, XllError> {
    with_registry(|registry| {
        registry
            .evaluation_results(result)
            .map_err(|e| value_error(e.to_string()))?
            .price()
            .ok_or_else(|| not_found("result does not contain price; request it in QS.EVALUATE"))
    })
}

#[xll_bindgen(
    name = "QS.RESULT.FAIR.RATE",
    category = "QuantSupport - Pricing",
    help = "Fair coupon or spread for the requested target from stored evaluation results"
)]
pub fn qs_result_fair_rate(result: &str) -> Result<f64, XllError> {
    with_registry(|registry| {
        registry
            .evaluation_results(result)
            .map_err(|e| value_error(e.to_string()))?
            .fair_rate()
            .ok_or_else(|| {
                not_found("result does not contain fair_rate; request it in QS.EVALUATE")
            })
    })
}

#[xll_bindgen(
    name = "QS.RESULT.YIELD",
    category = "QuantSupport - Pricing",
    help = "Annual decimal yield from stored evaluation results"
)]
pub fn qs_result_yield(result: &str) -> Result<f64, XllError> {
    with_registry(|registry| {
        registry
            .evaluation_results(result)
            .map_err(|e| value_error(e.to_string()))?
            .ytm()
            .ok_or_else(|| {
                not_found("result does not contain yield_to_maturity; request it in QS.EVALUATE")
            })
    })
}

#[xll_bindgen(
    name = "QS.RESULT.MODIFIED.DURATION",
    category = "QuantSupport - Pricing",
    help = "Modified duration from stored evaluation results"
)]
pub fn qs_result_modified_duration(result: &str) -> Result<f64, XllError> {
    with_registry(|registry| {
        registry
            .evaluation_results(result)
            .map_err(|e| value_error(e.to_string()))?
            .modified_duration()
            .ok_or_else(|| {
                not_found("result does not contain modified_duration; request it in QS.EVALUATE")
            })
    })
}

#[xll_bindgen(
    name = "QS.RESULT.Z.SPREAD",
    category = "QuantSupport - Pricing",
    help = "Decimal Z-spread from stored evaluation results"
)]
pub fn qs_result_z_spread(result: &str) -> Result<f64, XllError> {
    with_registry(|registry| {
        registry
            .evaluation_results(result)
            .map_err(|e| value_error(e.to_string()))?
            .z_spread()
            .ok_or_else(|| not_found("result does not contain z_spread; request it in QS.EVALUATE"))
    })
}

#[xll_bindgen(
    name = "QS.RESULT.OAS",
    category = "QuantSupport - Pricing",
    help = "Decimal option-adjusted spread from stored evaluation results"
)]
pub fn qs_result_oas(result: &str) -> Result<f64, XllError> {
    with_registry(|registry| {
        registry
            .evaluation_results(result)
            .map_err(|e| value_error(e.to_string()))?
            .oas()
            .ok_or_else(|| not_found("result does not contain oas; request it in QS.EVALUATE"))
    })
}

#[xll_bindgen(
    name = "QS.RESULT.CASHFLOWS",
    category = "QuantSupport - Pricing",
    help = "Spills cashflows from stored evaluation results with optional transpose and headers"
)]
pub fn qs_result_cashflows(
    result: &str,
    transpose: *const XLOPER12,
    headers: *const XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let transpose = optional_bool(transpose, "transpose")?;
    let headers = optional_bool(headers, "headers")?;
    with_registry(|registry| {
        let results = registry
            .evaluation_results(result)
            .map_err(|e| value_error(e.to_string()))?;
        super::valuation::cashflows_output(
            results
                .cashflows()
                .ok_or_else(|| not_found("request Cashflows in QS.EVALUATE"))?,
            transpose,
            headers,
        )
    })
}

#[xll_bindgen(
    name = "QS.RESULT.SENSITIVITIES",
    category = "QuantSupport - Pricing",
    help = "Spills raw derivatives from stored evaluation results with optional transpose and headers"
)]
pub fn qs_result_sensitivities(
    result: &str,
    transpose: *const XLOPER12,
    headers: *const XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let transpose = optional_bool(transpose, "transpose")?;
    let headers = optional_bool(headers, "headers")?;
    with_registry(|registry| {
        let results = registry
            .evaluation_results(result)
            .map_err(|e| value_error(e.to_string()))?;
        let sensitivities = results
            .sensitivities()
            .ok_or_else(|| not_found("request Sensitivities in QS.EVALUATE"))?;
        let rows = sensitivities.instrument_keys().len() + 1;
        let mut cells = vec![
            XLOPER12::from_str("Risk factor"),
            XLOPER12::from_str("Sensitivity"),
        ];
        for (key, exposure) in sensitivities
            .instrument_keys()
            .iter()
            .zip(sensitivities.exposure())
        {
            cells.push(XLOPER12::from_str(key));
            cells.push(XLOPER12::from_f64(*exposure));
        }
        Ok(spill(cells, rows, 2, transpose, headers))
    })
}

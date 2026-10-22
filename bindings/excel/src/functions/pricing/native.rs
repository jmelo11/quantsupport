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
    let currencies = worksheet_table(currency, "currency")?;
    if currencies.is_empty() {
        let prefer = optional_bool(prefer_instrument_index, "prefer_instrument_index")?
            .or(optional_bool(market_index, "prefer_instrument_index")?)
            .unwrap_or(false);
        return fixed_income_mappings(name, Vec::new(), prefer);
    }
    // Existing workbooks using (name, two-column mappings, prefer) remain valid.
    if currencies.iter().any(|row| row.len() > 1) {
        return fixed_income_mappings(
            name,
            currencies,
            optional_bool(market_index, "prefer_instrument_index")?.unwrap_or(false),
        );
    }
    let indices = worksheet_table(market_index, "market_index")?;
    let currency =
        crate::runtime::inputs::scalar_text(&currencies, "currency").map_err(value_error)?;
    let index =
        crate::runtime::inputs::scalar_text(&indices, "market_index").map_err(value_error)?;
    let mappings = vec![vec![Cell::Text(currency), Cell::Text(index)]];
    fixed_income_mappings(
        name,
        mappings,
        optional_bool(prefer_instrument_index, "prefer_instrument_index")?.unwrap_or(false),
    )
}

#[xll_bindgen(
    name = "QS.DISCOUNT.POLICY.FIXED.INCOME.FROM.MAPPINGS",
    category = "QuantSupport - Pricing",
    help = "Creates a native fixed-income policy from multiple Currency/Index rows"
)]
pub fn qs_discount_policy_fixed_income_from_mappings(
    name: &str,
    mappings: *const XLOPER12,
    prefer_instrument_index: *const XLOPER12,
) -> Result<String, XllError> {
    fixed_income_mappings(
        name,
        worksheet_table(mappings, "mappings")?,
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

#[xll_bindgen(
    name = "QS.PRICER.CREATE",
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
    name = "QS.EVALUATE.VALUE",
    category = "QuantSupport - Pricing",
    help = "Returns monetary PV directly without storing an EvaluationResults object"
)]
pub fn qs_evaluate_value(pricer: &str, trade: &str, context: &str) -> Result<f64, XllError> {
    with_registry(|registry| {
        pricing::evaluate_values(registry, pricer, &[trade.to_string()], context)
    })
    .map(|values| values[0])
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.EVALUATE.VALUES",
    category = "QuantSupport - Pricing",
    help = "Returns a PV list for a range of trade handles; no EvaluationResults objects are stored"
)]
pub fn qs_evaluate_values(
    pricer: &str,
    trades: *const XLOPER12,
    context: &str,
    transpose: *const XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let trades = string_values(trades, "trades")?;
    let transpose = optional_bool(transpose, "transpose")?;
    let values =
        with_registry(|registry| pricing::evaluate_values(registry, pricer, &trades, context))
            .map_err(value_error)?;
    let rows = values.len();
    Ok(spill(
        values.into_iter().map(XLOPER12::from_f64).collect(),
        rows,
        1,
        transpose,
        None,
    ))
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

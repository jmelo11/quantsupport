//! Read-only views of native EvaluationResults.
use crate::{functions::helpers::*, runtime::registry::with_registry};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};
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
        super::output::cashflows_output(
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

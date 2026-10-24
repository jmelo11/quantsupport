//! Context-first evaluation. Direct results avoid registry growth in large studies.
use crate::{
    functions::helpers::*,
    runtime::{
        context, historical, pricing,
        registry::{with_registry, with_registry_mut, QsObject},
    },
};
use quantsupport::prelude::Request;
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};

fn request(value: Option<&str>) -> Result<Request, XllError> {
    pricing::parse_requests(&[value
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Value")
        .into()])
    .map(|mut requests| requests.remove(0))
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.EVALUATE",
    category = "QuantSupport - Pricing",
    help = "Returns a requested number or table asynchronously from the committed context-owned market"
)]
pub fn qs_evaluate(
    context: &str,
    instrument: &str,
    request_name: Option<String>,
    pricer: Option<String>,
    policy: Option<String>,
    transpose: *const XLOPER12,
    headers: *const XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let request = request(request_name.as_deref())?;
    let transpose = optional_bool(transpose, "transpose")?;
    let headers = optional_bool(headers, "headers")?;
    let results = with_registry(|registry| {
        let context = context::resolve(registry, context)?;
        context::evaluate_requests(
            registry,
            &context,
            instrument,
            std::slice::from_ref(&request),
            pricer.as_deref().filter(|s| !s.is_empty()),
            policy.as_deref().filter(|s| !s.is_empty()),
        )
    })
    .map_err(value_error)?;
    match request {
        Request::Cashflows => super::output::cashflows_output(
            results
                .cashflows()
                .ok_or_else(|| not_found("pricer did not return cashflows"))?,
            transpose,
            headers,
        ),
        Request::Sensitivities => {
            let risk = results
                .sensitivities()
                .ok_or_else(|| not_found("pricer did not return sensitivities"))?;
            let mut cells = vec![
                XLOPER12::from_str("Risk factor"),
                XLOPER12::from_str("Sensitivity"),
            ];
            for (label, value) in risk.instrument_keys().iter().zip(risk.exposure()) {
                cells.push(XLOPER12::from_str(label));
                cells.push(XLOPER12::from_f64(*value));
            }
            Ok(spill(
                cells,
                risk.instrument_keys().len() + 1,
                2,
                transpose,
                headers,
            ))
        }
        _ => Ok(xll_rs::returning::XlReturn::num(
            context::scalar(&results, &request).map_err(value_error)?,
        )
        .into_raw()),
    }
}

#[xll_bindgen(
    name = "QS.EVALUATE.RANGE",
    category = "QuantSupport - Pricing",
    help = "Prices instruments against one context or paired context handles; dated contexts run across workers and scalar results spill in input order"
)]
pub fn qs_evaluate_range(
    context: *const XLOPER12,
    instruments: *const XLOPER12,
    request_name: Option<String>,
    pricer: Option<String>,
    policy: Option<String>,
    transpose: *const XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let request = request(request_name.as_deref())?;
    let instruments = string_values(instruments, "instruments")?;
    let contexts = string_values(context, "context")?;
    if contexts.len() != 1 && contexts.len() != instruments.len() {
        return Err(value_error(
            "context must be one handle or an array aligned with instruments",
        ));
    }
    if contexts.is_empty() {
        return Err(value_error("supply at least one context"));
    }
    if instruments.is_empty() {
        return Err(value_error("supply at least one instrument"));
    }
    let values = with_registry(|registry| {
        if contexts.len() > 1 {
            return instruments
                .iter()
                .zip(&contexts)
                .map(|(instrument, handle)| {
                    let market = context::resolve(registry, handle)?;
                    let results = context::evaluate_requests(
                        registry,
                        &market,
                        instrument,
                        std::slice::from_ref(&request),
                        pricer.as_deref().filter(|s| !s.is_empty()),
                        policy.as_deref().filter(|s| !s.is_empty()),
                    )?;
                    context::scalar(&results, &request)
                })
                .collect::<Result<Vec<_>, String>>();
        }
        let context = context::resolve(registry, &contexts[0])?;
        if matches!(request, Request::Value) && policy.as_deref().is_none_or(str::is_empty) {
            if let Some(pricer) = pricer.as_deref().filter(|s| !s.is_empty()) {
                return pricing::evaluate_values(registry, pricer, &instruments, &context);
            }
        }
        instruments
            .iter()
            .map(|instrument| {
                let results = context::evaluate_requests(
                    registry,
                    &context,
                    instrument,
                    std::slice::from_ref(&request),
                    pricer.as_deref().filter(|s| !s.is_empty()),
                    policy.as_deref().filter(|s| !s.is_empty()),
                )?;
                context::scalar(&results, &request)
            })
            .collect::<Result<Vec<_>, String>>()
    })
    .map_err(value_error)?;
    let count = values.len();
    Ok(spill(
        values.into_iter().map(XLOPER12::from_f64).collect(),
        count,
        1,
        optional_bool(transpose, "transpose")?,
        None,
    ))
}

#[xll_bindgen(
    name = "QS.EVALUATE.FROM.SNAPSHOT",
    category = "QuantSupport - Pricing",
    help = "Prices each instrument at its origination timestamp, constructing each matched market once per batch"
)]
pub fn qs_evaluate_from_snapshot(
    store: &str,
    instruments: *const XLOPER12,
    timestamps: *const XLOPER12,
    pricer: &str,
    request_name: Option<String>,
    fixings: Option<String>,
    rule: Option<String>,
    quote_level: Option<String>,
    transpose: *const XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    // Preserve blank positions: silently dropping a timestamp could misalign rows.
    let strings = |range, label| {
        worksheet_array(range, label)?
            .iter()
            .map(|cell| {
                cell.text(label).and_then(|s| {
                    if s.is_empty() {
                        Err(format!("{label} cannot contain blanks"))
                    } else {
                        Ok(s.to_string())
                    }
                })
            })
            .collect::<Result<Vec<_>, String>>()
            .map_err(value_error)
    };
    let instruments = strings(instruments, "instruments")?;
    let timestamps = strings(timestamps, "timestamps")?;
    let request = request(request_name.as_deref())?;
    let values = with_registry(|registry| {
        historical::values(
            registry,
            store,
            &instruments,
            &timestamps,
            &request,
            pricer,
            fixings.as_deref().filter(|s| !s.is_empty()),
            rule.as_deref(),
            quote_level.as_deref(),
        )
    })
    .map_err(value_error)?;
    let count = values.len();
    Ok(spill(
        values.into_iter().map(XLOPER12::from_f64).collect(),
        count,
        1,
        optional_bool(transpose, "transpose")?,
        None,
    ))
}

#[xll_bindgen(
    name = "QS.EVALUATE.RESULTS",
    category = "QuantSupport - Pricing",
    help = "Evaluates request arrays and stores one native EvaluationResults object for reuse by QS.RESULT functions"
)]
pub fn qs_evaluate_results(
    context: &str,
    trade: &str,
    requests: *const XLOPER12,
    pricer: Option<String>,
    policy: Option<String>,
) -> Result<String, XllError> {
    let names = string_values(requests, "requests")?;
    let requests = pricing::parse_requests(&names).map_err(value_error)?;
    let market_revision =
        with_registry(|r| context::revision_key(r, context)).map_err(value_error)?;
    let key = input_key(&(context, trade, &names, &pricer, &policy, market_revision))?;
    let name = with_registry(|r| r.cell_name("evaluation")).map_err(value_error)?;
    let results = with_registry(|registry| {
        let context = context::resolve(registry, context)?;
        // Validate all dependencies even when the result can be reused.
        registry.object(trade).map_err(|e| e.to_string())?;
        if let Some(handle) = pricer.as_deref().filter(|s| !s.is_empty()) {
            registry.pricer(handle).map_err(|e| e.to_string())?;
        }
        if let Some(handle) = policy.as_deref().filter(|s| !s.is_empty()) {
            registry
                .discount_policy(handle)
                .map_err(|e| e.to_string())?;
        }
        if let Some(handle) = registry
            .handle_if_unchanged(&name, "EvaluationResults", &key)
            .map_err(|e| e.to_string())?
        {
            return Ok(Err(handle));
        }
        context::evaluate_requests(
            registry,
            &context,
            trade,
            &requests,
            pricer.as_deref().filter(|s| !s.is_empty()),
            policy.as_deref().filter(|s| !s.is_empty()),
        )
        .map(Ok)
    })
    .map_err(value_error)?;
    match results {
        Err(handle) => Ok(handle),
        Ok(results) => with_registry_mut(|registry| {
            registry.upsert_if_changed(&name, QsObject::EvaluationResults(Box::new(results)), key)
        })
        .map_err(value_error),
    }
}

use crate::{
    functions::{
        helpers::{not_found, value_error},
        pricing::valuation::{evaluate, evaluate_bond},
    },
    runtime::registry::with_registry,
};
use quantsupport::prelude::Request;
use xll_rs::types::{XllError, XLOPER12};
use xllgen::xll_bindgen;

#[xll_bindgen(
    name = "QS.BOND.SENSITIVITIES",
    category = "QuantSupport - Risk",
    help = "Spills curve and dPV/dZSpread risk in one table for a straight fixed-rate bond"
)]
pub fn qs_bond_sensitivities(
    trade: &str,
    context: &str,
    transpose: *const xll_rs::types::XLOPER12,
    headers: *const xll_rs::types::XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let headers = crate::functions::helpers::optional_bool(headers, "headers")?;
    let results = with_registry(|registry| {
        evaluate_bond(
            registry,
            trade,
            context,
            &[Request::Value, Request::Sensitivities],
        )
    })
    .map_err(value_error)?;
    let sensitivities = results
        .sensitivities()
        .ok_or_else(|| not_found("the bond pricer did not return sensitivities"))?;
    let rows = sensitivities.instrument_keys().len() + 1;
    let mut cells = Vec::with_capacity(rows * 2);
    cells.push(XLOPER12::from_str("Risk factor"));
    cells.push(XLOPER12::from_str("Sensitivity"));
    for (key, exposure) in sensitivities
        .instrument_keys()
        .iter()
        .zip(sensitivities.exposure())
    {
        cells.push(XLOPER12::from_str(key));
        cells.push(XLOPER12::from_f64(*exposure));
    }
    Ok(crate::functions::helpers::spill(
        cells, rows, 2, transpose, headers,
    ))
}

#[xll_bindgen(
    name = "QS.SENSITIVITIES",
    category = "QuantSupport - Risk",
    help = "Spills sensitivity keys and values for a supported trade/pricer"
)]
pub fn qs_sensitivities(
    trade: &str,
    context: &str,
    transpose: *const xll_rs::types::XLOPER12,
    headers: *const xll_rs::types::XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let headers = crate::functions::helpers::optional_bool(headers, "headers")?;
    let results = with_registry(|registry| {
        evaluate(
            registry,
            trade,
            context,
            &[Request::Value, Request::Sensitivities],
        )
    })
    .map_err(value_error)?;
    let sensitivities = results
        .sensitivities()
        .ok_or_else(|| not_found("the selected pricer did not return sensitivities"))?;

    let rows = sensitivities.instrument_keys().len() + 1;
    let mut cells = Vec::with_capacity(rows * 2);
    cells.push(XLOPER12::from_str("Risk factor"));
    cells.push(XLOPER12::from_str("Sensitivity"));
    for (key, exposure) in sensitivities
        .instrument_keys()
        .iter()
        .zip(sensitivities.exposure())
    {
        cells.push(XLOPER12::from_str(key));
        cells.push(XLOPER12::from_f64(*exposure));
    }
    Ok(crate::functions::helpers::spill(
        cells, rows, 2, transpose, headers,
    ))
}

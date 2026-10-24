use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::XllError;

use crate::runtime::registry::with_registry;

use crate::functions::helpers::{parse_index, string_column, value_error};

#[xll_bindgen(
    name = "QS.SIMULATION.NPATHS",
    category = "QuantSupport - Market",
    help = "Returns the number of paths in a context simulation"
)]
pub fn qs_simulation_n_paths(context: &str, market_index: &str) -> Result<i32, XllError> {
    let index = parse_index(market_index)?;
    with_registry(|registry| {
        let market = crate::runtime::context::elements(
            registry,
            context,
            quantsupport::prelude::ConstructedElementRequest::Simulation {
                market_index: index.clone(),
            },
        )?;
        let element = market
            .simulations()
            .get(&index)
            .ok_or_else(|| format!("simulation '{index}' was not constructed"))?;
        let output = i32::try_from(element.simulation().borrow().n_paths())
            .map_err(|_| "simulation path count does not fit in an Excel integer".to_string());
        output
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SIMULATION.DT",
    category = "QuantSupport - Market",
    help = "Returns the time step of a context simulation"
)]
pub fn qs_simulation_dt(context: &str, market_index: &str) -> Result<f64, XllError> {
    let index = parse_index(market_index)?;
    with_registry(|registry| {
        let market = crate::runtime::context::elements(
            registry,
            context,
            quantsupport::prelude::ConstructedElementRequest::Simulation {
                market_index: index.clone(),
            },
        )?;
        let element = market
            .simulations()
            .get(&index)
            .ok_or_else(|| format!("simulation '{index}' was not constructed"))?;
        let output = Ok::<f64, String>(element.simulation().borrow().dt());
        output
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SIMULATION.DATES",
    category = "QuantSupport - Market",
    help = "Spills the dates of a context simulation"
)]
pub fn qs_simulation_dates(
    context: &str,
    market_index: &str,
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let index = parse_index(market_index)?;
    with_registry(|registry| {
        let market = crate::runtime::context::elements(
            registry,
            context,
            quantsupport::prelude::ConstructedElementRequest::Simulation {
                market_index: index.clone(),
            },
        )?;
        let element = market
            .simulations()
            .get(&index)
            .ok_or_else(|| format!("simulation '{index}' was not constructed"))?;
        let output = Ok::<Vec<String>, String>(
            element
                .simulation()
                .borrow()
                .dates()
                .iter()
                .map(ToString::to_string)
                .collect(),
        );
        output
    })
    .map(|values| string_column(values, transpose))
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SIMULATION.PATH",
    category = "QuantSupport - Market",
    help = "Spills one zero-based path from a context simulation"
)]
pub fn qs_simulation_path(
    context: &str,
    market_index: &str,
    path_index: i32,
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let index = parse_index(market_index)?;
    let path_index =
        usize::try_from(path_index).map_err(|_| value_error("path index must be non-negative"))?;
    with_registry(|registry| {
        let market = crate::runtime::context::elements(
            registry,
            context,
            quantsupport::prelude::ConstructedElementRequest::Simulation {
                market_index: index.clone(),
            },
        )?;
        let element = market
            .simulations()
            .get(&index)
            .ok_or_else(|| format!("simulation '{index}' was not constructed"))?;
        let output = element
            .simulation()
            .borrow()
            .path()
            .get(path_index)
            .ok_or_else(|| format!("simulation path {path_index} is out of range"))
            .map(|path| {
                path.iter()
                    .map(|value| xll_rs::types::XLOPER12::from_f64(value.value()))
                    .collect::<Vec<_>>()
            });
        output
    })
    .map(|cells| {
        let rows = cells.len();
        crate::functions::helpers::spill(cells, rows, 1, transpose, None)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SIMULATION.CALIBRATION",
    category = "QuantSupport - Market",
    help = "Spills Hull-White calibration quotes, fitted parameters and pricing residuals"
)]
pub fn qs_simulation_calibration(
    context: &str,
    market_index: &str,
    transpose: *const xll_rs::types::XLOPER12,
    headers: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let headers = crate::functions::helpers::optional_bool(headers, "headers")?;
    use xll_rs::types::XLOPER12;
    let index = parse_index(market_index)?;
    let quality = with_registry(|registry| {
        let market = crate::runtime::context::elements(
            registry,
            context,
            quantsupport::prelude::ConstructedElementRequest::Simulation {
                market_index: index.clone(),
            },
        )?;
        let element = market
            .simulations()
            .get(&index)
            .ok_or_else(|| format!("simulation '{index}' was not constructed"))?;
        element.calibration_quality().cloned().ok_or_else(|| {
            format!("simulation '{index}' has no Hull-White calibration diagnostics")
        })
    })
    .map_err(value_error)?;
    let labels = [
        "identifier",
        "expiry",
        "t",
        "big_t",
        "market_vol",
        "market_price",
        "model_price",
        "calibrated_sigma",
        "forward_rate",
        "effective_strike",
        "price_error",
    ];
    let rows = quality.records.len() + 1;
    let mut cells: Vec<_> = labels
        .iter()
        .map(|header| XLOPER12::from_str(header))
        .collect();
    for record in quality.records {
        cells.push(XLOPER12::from_str(&record.identifier));
        cells.push(XLOPER12::from_str(&record.expiry.to_string()));
        for value in [
            record.t,
            record.big_t,
            record.market_vol,
            record.market_price,
            record.model_price,
            record.calibrated_sigma,
            record.forward_rate,
            record.effective_strike,
            record.model_price - record.market_price,
        ] {
            cells.push(XLOPER12::from_f64(value));
        }
    }
    Ok(crate::functions::helpers::spill(
        cells,
        rows,
        labels.len(),
        transpose,
        headers,
    ))
}

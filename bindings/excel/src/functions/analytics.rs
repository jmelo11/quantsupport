use quantsupport::prelude::{Period, Scalar};
use xll_rs::types::XllError;
use xllgen::xll_bindgen;

use crate::registry::with_registry;

use super::helpers::{parse_date, parse_index, parse_time_unit, string_column, value_error};

#[xll_bindgen(
    name = "QS.VOL.SURFACE",
    category = "QuantSupport - Volatility",
    help = "Returns volatility from a surface constructed in a PricingContext"
)]
pub fn qs_vol_surface(
    context: &str,
    market_index: &str,
    expiry_date: &str,
    key: f64,
) -> Result<f64, XllError> {
    let index = parse_index(market_index)?;
    let expiry = parse_date(expiry_date)?;
    with_registry(|registry| {
        registry
            .pricing_context(context)
            .map_err(|error| error.to_string())?
            .constructed_elements()
            .volatility_surface(&index)
            .ok_or_else(|| format!("volatility surface '{index}' was not constructed"))?
            .surface()
            .volatility_from_date(expiry, key)
            .map(|value| value.value())
            .map_err(|error| error.to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.VOL.CUBE",
    category = "QuantSupport - Volatility",
    help = "Returns volatility from a cube constructed in a PricingContext"
)]
pub fn qs_vol_cube(
    context: &str,
    market_index: &str,
    expiry_date: &str,
    maturity_length: i32,
    maturity_unit: &str,
    key: f64,
) -> Result<f64, XllError> {
    let index = parse_index(market_index)?;
    let expiry = parse_date(expiry_date)?;
    let maturity = Period::new(maturity_length, parse_time_unit(maturity_unit)?);
    with_registry(|registry| {
        registry
            .pricing_context(context)
            .map_err(|error| error.to_string())?
            .constructed_elements()
            .volatility_cube(&index)
            .ok_or_else(|| format!("volatility cube '{index}' was not constructed"))?
            .cube()
            .volatility_from_date(expiry, maturity, key)
            .map(|value| value.value())
            .map_err(|error| error.to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SIMULATION.NPATHS",
    category = "QuantSupport - Simulations",
    help = "Returns the number of paths in a context simulation"
)]
pub fn qs_simulation_n_paths(context: &str, market_index: &str) -> Result<i32, XllError> {
    let index = parse_index(market_index)?;
    with_registry(|registry| {
        let element = registry
            .pricing_context(context)
            .map_err(|error| error.to_string())?
            .constructed_elements()
            .simulations()
            .get(&index)
            .ok_or_else(|| format!("simulation '{index}' was not constructed"))?;
        i32::try_from(element.simulation().borrow().n_paths())
            .map_err(|_| "simulation path count does not fit in an Excel integer".to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SIMULATION.DT",
    category = "QuantSupport - Simulations",
    help = "Returns the time step of a context simulation"
)]
pub fn qs_simulation_dt(context: &str, market_index: &str) -> Result<f64, XllError> {
    let index = parse_index(market_index)?;
    with_registry(|registry| {
        let element = registry
            .pricing_context(context)
            .map_err(|error| error.to_string())?
            .constructed_elements()
            .simulations()
            .get(&index)
            .ok_or_else(|| format!("simulation '{index}' was not constructed"))?;
        Ok::<f64, String>(element.simulation().borrow().dt())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SIMULATION.DATES",
    category = "QuantSupport - Simulations",
    help = "Spills the dates of a context simulation"
)]
pub fn qs_simulation_dates(
    context: &str,
    market_index: &str,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let index = parse_index(market_index)?;
    with_registry(|registry| {
        let element = registry
            .pricing_context(context)
            .map_err(|error| error.to_string())?
            .constructed_elements()
            .simulations()
            .get(&index)
            .ok_or_else(|| format!("simulation '{index}' was not constructed"))?;
        Ok::<Vec<String>, String>(
            element
                .simulation()
                .borrow()
                .dates()
                .iter()
                .map(ToString::to_string)
                .collect(),
        )
    })
    .map(string_column)
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SIMULATION.PATH",
    category = "QuantSupport - Simulations",
    help = "Spills one zero-based path from a context simulation"
)]
pub fn qs_simulation_path(
    context: &str,
    market_index: &str,
    path_index: i32,
) -> Result<Vec<f64>, XllError> {
    let index = parse_index(market_index)?;
    let path_index =
        usize::try_from(path_index).map_err(|_| value_error("path index must be non-negative"))?;
    with_registry(|registry| {
        let element = registry
            .pricing_context(context)
            .map_err(|error| error.to_string())?
            .constructed_elements()
            .simulations()
            .get(&index)
            .ok_or_else(|| format!("simulation '{index}' was not constructed"))?;
        element
            .simulation()
            .borrow()
            .path()
            .get(path_index)
            .ok_or_else(|| format!("simulation path {path_index} is out of range"))
            .map(|path| path.iter().map(Scalar::value).collect())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.SIMULATION.CALIBRATION",
    category = "QuantSupport - Simulations",
    help = "Spills Hull-White calibration quotes, fitted parameters and pricing residuals"
)]
pub fn qs_simulation_calibration(
    context: &str,
    market_index: &str,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    use xll_rs::{convert::build_multi, types::XLOPER12};
    let index = parse_index(market_index)?;
    let quality = with_registry(|registry| {
        let element = registry
            .pricing_context(context)
            .map_err(|error| error.to_string())?
            .constructed_elements()
            .simulations()
            .get(&index)
            .ok_or_else(|| format!("simulation '{index}' was not constructed"))?;
        element.calibration_quality().cloned().ok_or_else(|| {
            format!("simulation '{index}' has no Hull-White calibration diagnostics")
        })
    })
    .map_err(value_error)?;
    let headers = [
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
    let mut cells: Vec<_> = headers
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
    Ok(build_multi(cells, rows, headers.len()))
}

//! Native Request helpers.
use crate::{functions::helpers::value_error, runtime::inputs::Cell};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::XllError;
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

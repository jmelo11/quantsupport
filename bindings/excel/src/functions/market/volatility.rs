use quantsupport::prelude::Period;
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::XllError;

use crate::runtime::registry::with_registry;

use crate::functions::helpers::{parse_date, parse_index, parse_time_unit, value_error};

#[xll_bindgen(
    name = "QS.VOL.SURFACE",
    category = "QuantSupport - Market",
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
        crate::runtime::context::elements(
            registry,
            context,
            quantsupport::prelude::ConstructedElementRequest::VolatilitySurface {
                market_index: index.clone(),
            },
        )?
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
    category = "QuantSupport - Market",
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
        crate::runtime::context::elements(
            registry,
            context,
            quantsupport::prelude::ConstructedElementRequest::VolatilityCube {
                market_index: index.clone(),
            },
        )?
        .volatility_cube(&index)
        .ok_or_else(|| format!("volatility cube '{index}' was not constructed"))?
        .cube()
        .volatility_from_date(expiry, maturity, key)
        .map(|value| value.value())
        .map_err(|error| error.to_string())
    })
    .map_err(value_error)
}

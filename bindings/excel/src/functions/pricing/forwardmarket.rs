//! Forward scenario configuration and context worksheet functions.

use quantsupport::core::forwardmarket::configuration::{
    CurveRoll, FixingRoll, ForwardMarketConfiguration, VolatilityRoll,
};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::XllError;

use crate::{
    functions::helpers::{parse_date, value_error},
    runtime::{forwardmarket::ForwardMarketRuntime, registry::with_registry_mut},
};

#[xll_bindgen(
    name = "QS.FORWARD.CONFIG",
    category = "QuantSupport - Pricing",
    help = "Creates horizon assumptions: curve ImpliedForward/ConstantTenor, volatility ConstantTenor/ForwardVariance, fixings Project/Preserve"
)]
pub fn qs_forward_config(
    name: &str,
    target_date: &str,
    curve_roll: Option<String>,
    volatility_roll: Option<String>,
    fixing_roll: Option<String>,
) -> Result<String, XllError> {
    let curve = match curve_roll
        .as_deref()
        .unwrap_or("ImpliedForward")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "impliedforward" => CurveRoll::ImpliedForward,
        "constanttenor" => CurveRoll::ConstantTenor,
        _ => {
            return Err(value_error(
                "curve_roll must be ImpliedForward or ConstantTenor",
            ))
        }
    };
    let volatility = match volatility_roll
        .as_deref()
        .unwrap_or("ConstantTenor")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "constanttenor" => VolatilityRoll::ConstantTenor,
        "forwardvariance" => VolatilityRoll::ForwardVariance,
        _ => {
            return Err(value_error(
                "volatility_roll must be ConstantTenor or ForwardVariance",
            ))
        }
    };
    let fixings = match fixing_roll
        .as_deref()
        .unwrap_or("Project")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "project" => FixingRoll::Project,
        "preserve" => FixingRoll::Preserve,
        _ => return Err(value_error("fixing_roll must be Project or Preserve")),
    };
    let configuration = ForwardMarketConfiguration::new(parse_date(target_date)?)
        .with_curve_roll(curve)
        .with_volatility(volatility)
        .with_fixings(fixings);
    with_registry_mut(|registry| {
        ForwardMarketRuntime::new(registry).configuration(name, configuration)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADVANCE",
    category = "QuantSupport - Pricing",
    help = "Generates an independent horizon pricing context from current market inputs and a QS.FORWARD.CONFIG handle"
)]
pub fn qs_context_advance(
    name: &str,
    context: &str,
    configuration: &str,
) -> Result<String, XllError> {
    with_registry_mut(|registry| {
        ForwardMarketRuntime::new(registry).pricing_context(name, context, configuration)
    })
    .map_err(value_error)
}

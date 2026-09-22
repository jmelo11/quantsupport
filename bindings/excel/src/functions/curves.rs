use std::{cell::RefCell, rc::Rc};

use quantsupport::prelude::{
    DiscountCurveElement, DualFwd, FlatForwardTermStructure, RateDefinition,
};
use xll_rs::types::XllError;
use xllgen::xll_bindgen;

use crate::registry::{with_registry, with_registry_mut, QsObject};

use super::helpers::{
    parse_compounding, parse_date, parse_day_counter, parse_frequency, parse_index, value_error,
};

#[xll_bindgen(
    name = "QS.CURVE.FLAT",
    volatile,
    category = "QuantSupport - Curves",
    help = "Creates a QuantSupport DiscountCurveElement backed by FlatForwardTermStructure"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_curve_flat(
    name: &str,
    market_index: &str,
    reference_date: &str,
    rate: f64,
    day_counter: &str,
    compounding: &str,
    frequency: &str,
) -> Result<String, XllError> {
    let index = parse_index(market_index)?;
    let curve = FlatForwardTermStructure::new(
        parse_date(reference_date)?,
        DualFwd::from(rate),
        RateDefinition::new(
            parse_day_counter(day_counter)?,
            parse_compounding(compounding)?,
            parse_frequency(frequency)?,
        ),
    )
    .with_pillar_label(name.to_string());
    let element = DiscountCurveElement::new(index, Rc::new(RefCell::new(curve)));
    with_registry_mut(|registry| registry.upsert(name, QsObject::DiscountCurve(Box::new(element))))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CURVE.DISCOUNT",
    category = "QuantSupport - Curves",
    help = "Returns a DiscountCurveElement discount factor at a date"
)]
pub fn qs_curve_discount(curve: &str, date: &str) -> Result<f64, XllError> {
    let date = parse_date(date)?;
    with_registry(|registry| {
        registry
            .discount_curve(curve)
            .map_err(|error| error.to_string())?
            .curve()
            .discount_factor(date)
            .map(|value| value.value())
            .map_err(|error| error.to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CURVE.FORWARD",
    category = "QuantSupport - Curves",
    help = "Returns a DiscountCurveElement forward rate between two dates"
)]
pub fn qs_curve_forward(
    curve: &str,
    start_date: &str,
    end_date: &str,
    compounding: &str,
    frequency: &str,
) -> Result<f64, XllError> {
    let start = parse_date(start_date)?;
    let end = parse_date(end_date)?;
    let compounding = parse_compounding(compounding)?;
    let frequency = parse_frequency(frequency)?;
    with_registry(|registry| {
        registry
            .discount_curve(curve)
            .map_err(|error| error.to_string())?
            .curve()
            .forward_rate(start, end, compounding, frequency)
            .map(|value| value.value())
            .map_err(|error| error.to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.ELEMENTS.CREATE",
    volatile,
    category = "QuantSupport - Curves",
    help = "Creates a QuantSupport ConstructedElementStore"
)]
pub fn qs_elements_create(name: &str) -> Result<String, XllError> {
    with_registry_mut(|registry| {
        registry.upsert(name, QsObject::ConstructedElements(Box::default()))
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.ELEMENTS.ADD.DISCOUNT.CURVE",
    category = "QuantSupport - Curves",
    help = "Adds a DiscountCurveElement to a ConstructedElementStore"
)]
pub fn qs_elements_add_discount_curve(elements: &str, curve: &str) -> Result<String, XllError> {
    with_registry_mut(|registry| {
        let curve = registry.discount_curve(curve)?.clone();
        registry
            .update_constructed_elements(elements, |store| {
                store
                    .discount_curves_mut()
                    .insert(curve.market_index().clone(), curve);
            })
            .map(|(_, handle)| handle)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.CURVE.DISCOUNT",
    category = "QuantSupport - Curves",
    help = "Queries a bootstrapped discount curve in a PricingContext"
)]
pub fn qs_context_curve_discount(
    context: &str,
    market_index: &str,
    date: &str,
) -> Result<f64, XllError> {
    let index = parse_index(market_index)?;
    let date = parse_date(date)?;
    with_registry(|registry| {
        let context = registry
            .pricing_context(context)
            .map_err(|error| error.to_string())?;
        context
            .constructed_elements()
            .discount_curve(&index)
            .ok_or_else(|| format!("discount curve '{index}' was not constructed"))?
            .curve()
            .discount_factor(date)
            .map(|value| value.value())
            .map_err(|error| error.to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.CURVE.FORWARD",
    category = "QuantSupport - Curves",
    help = "Queries a bootstrapped curve forward rate in a PricingContext"
)]
pub fn qs_context_curve_forward(
    context: &str,
    market_index: &str,
    start_date: &str,
    end_date: &str,
    compounding: &str,
    frequency: &str,
) -> Result<f64, XllError> {
    let index = parse_index(market_index)?;
    let start = parse_date(start_date)?;
    let end = parse_date(end_date)?;
    let compounding = parse_compounding(compounding)?;
    let frequency = parse_frequency(frequency)?;
    with_registry(|registry| {
        let context = registry
            .pricing_context(context)
            .map_err(|error| error.to_string())?;
        context
            .constructed_elements()
            .discount_curve(&index)
            .ok_or_else(|| format!("discount curve '{index}' was not constructed"))?
            .curve()
            .forward_rate(start, end, compounding, frequency)
            .map(|value| value.value())
            .map_err(|error| error.to_string())
    })
    .map_err(value_error)
}

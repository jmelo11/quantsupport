use std::{cell::RefCell, rc::Rc};

use quantsupport::prelude::{
    Compounding, DiscountCurveElement, DualFwd, FlatForwardTermStructure, Frequency, RateDefinition,
};
use xll_rs::types::{XllError, XLOPER12};
use xllgen::xll_bindgen;

use crate::runtime::registry::{with_registry, with_registry_mut, QsObject};

use crate::functions::helpers::{
    input_key, parse_compounding, parse_date, parse_day_counter, parse_frequency, parse_index,
    value_error,
};

fn cached_handle(name: &str, kind: &str, key: &str) -> Result<Option<String>, XllError> {
    with_registry(|registry| registry.handle_if_unchanged(name, kind, key)).map_err(value_error)
}

fn derived_curve_name(context: &str, market_index: &str) -> String {
    let context_identity = context
        .rsplit_once(':')
        .map_or(context, |(identity, _)| identity);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in context_identity
        .bytes()
        .chain(std::iter::once(b'|'))
        .chain(market_index.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("context_curve_{hash:016x}")
}

fn rate_convention(
    compounding: Option<String>,
    frequency: Option<String>,
) -> Result<(Compounding, Frequency), XllError> {
    let compounding = parse_compounding(compounding.as_deref().unwrap_or("Simple"))?;
    let frequency = parse_frequency(frequency.as_deref().unwrap_or("Annual"))?;
    Ok((compounding, frequency))
}

fn flat_curve_element(
    name: &str,
    market_index: &str,
    reference_date: &str,
    rate: f64,
    day_counter: &str,
    compounding: &str,
    frequency: &str,
) -> Result<DiscountCurveElement, XllError> {
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
    Ok(DiscountCurveElement::new(
        index,
        Rc::new(RefCell::new(curve)),
    ))
}

#[xll_bindgen(
    name = "QS.CURVE.FLAT",
    volatile,
    category = "QuantSupport - Market",
    help = "Creates a flat DiscountCurveElement; add its handle to an elements store with QS.CURVE.ADD"
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
    let definition_key = input_key(&(
        market_index,
        reference_date,
        rate.to_bits(),
        day_counter,
        compounding,
        frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "DiscountCurveElement", &definition_key)? {
        return Ok(handle);
    }
    let element = flat_curve_element(
        name,
        market_index,
        reference_date,
        rate,
        day_counter,
        compounding,
        frequency,
    )?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::DiscountCurve(Box::new(element)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CURVE.GET",
    category = "QuantSupport - Market",
    help = "Returns a reusable discount-curve handle from an initialized PricingContext"
)]
pub fn qs_curve_get(context: &str, market_index: &str) -> Result<String, XllError> {
    let index = parse_index(market_index)?;
    let name = derived_curve_name(context, &index.to_string());
    let definition_key = input_key(&(context, index.to_string()))?;
    if let Some(handle) = cached_handle(&name, "DiscountCurveElement", &definition_key)? {
        return Ok(handle);
    }

    with_registry_mut(|registry| {
        let curve = registry
            .pricing_context(context)
            .map_err(|error| error.to_string())?
            .constructed_elements()
            .discount_curve(&index)
            .ok_or_else(|| format!("discount curve '{index}' was not constructed"))?
            .clone();
        registry
            .upsert_if_changed(
                &name,
                QsObject::DiscountCurve(Box::new(curve)),
                definition_key,
            )
            .map_err(|error| error.to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CURVE.DISCOUNT",
    category = "QuantSupport - Market",
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
    name = "QS.CURVE.ZERO.RATE",
    category = "QuantSupport - Market",
    help = "Returns the zero rate from the curve reference date to a date; defaults to Simple and Annual"
)]
pub fn qs_curve_zero_rate(
    curve: &str,
    date: &str,
    compounding: Option<String>,
    frequency: Option<String>,
) -> Result<f64, XllError> {
    let date = parse_date(date)?;
    let (compounding, frequency) = rate_convention(compounding, frequency)?;
    with_registry(|registry| {
        let element = registry
            .discount_curve(curve)
            .map_err(|error| error.to_string())?;
        let curve = element.curve();
        curve
            .forward_rate(curve.reference_date(), date, compounding, frequency)
            .map(|value| value.value())
            .map_err(|error| error.to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CURVE.FORWARD",
    category = "QuantSupport - Market",
    help = "Returns a DiscountCurveElement forward rate between two dates"
)]
pub fn qs_curve_forward(
    curve: &str,
    start_date: &str,
    end_date: &str,
    compounding: Option<String>,
    frequency: Option<String>,
) -> Result<f64, XllError> {
    let start = parse_date(start_date)?;
    let end = parse_date(end_date)?;
    let (compounding, frequency) = rate_convention(compounding, frequency)?;
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
    name = "QS.CURVE.TABLE",
    category = "QuantSupport - Market",
    help = "Spills a curve's pillar dates, discount factors, zero rates, and forwards; rate convention defaults to Simple/Annual"
)]
pub fn qs_curve_table(
    curve: &str,
    compounding: Option<String>,
    frequency: Option<String>,
    transpose: *const xll_rs::types::XLOPER12,
    headers: *const xll_rs::types::XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let headers = crate::functions::helpers::optional_bool(headers, "headers")?;
    let (compounding, frequency) = rate_convention(compounding, frequency)?;
    with_registry(|registry| {
        let element = registry
            .discount_curve(curve)
            .map_err(|error| error.to_string())?;
        let curve = element.curve();
        let nodes = curve.nodes().ok_or_else(|| {
            "curve does not expose nodes; use QS.CURVE.DISCOUNT or QS.CURVE.FORWARD to sample it"
                .to_string()
        })?;
        if nodes.is_empty() {
            return Err("curve exposes no nodes".to_string());
        }
        let labels = curve.pillar_labels().unwrap_or_default();
        let reference_date = curve.reference_date();

        const COLUMNS: usize = 5;
        let mut cells = Vec::with_capacity((nodes.len() + 1) * COLUMNS);
        for header in [
            "Pillar",
            "Date",
            "Discount factor",
            "Zero rate",
            "Forward from prior node",
        ] {
            cells.push(XLOPER12::from_str(header));
        }

        for (row, (date, discount_factor)) in nodes.iter().enumerate() {
            let label = if *date == reference_date {
                "Reference"
            } else {
                labels.get(row.saturating_sub(1)).map_or("", String::as_str)
            };
            cells.push(XLOPER12::from_str(label));
            cells.push(XLOPER12::from_str(&date.to_string()));
            cells.push(XLOPER12::from_f64(discount_factor.value()));

            if *date == reference_date {
                cells.push(XLOPER12::nil());
            } else {
                let zero = curve
                    .forward_rate(reference_date, *date, compounding, frequency)
                    .map_err(|error| error.to_string())?;
                cells.push(XLOPER12::from_f64(zero.value()));
            }

            if row == 0 {
                cells.push(XLOPER12::nil());
            } else {
                let forward = curve
                    .forward_rate(nodes[row - 1].0, *date, compounding, frequency)
                    .map_err(|error| error.to_string())?;
                cells.push(XLOPER12::from_f64(forward.value()));
            }
        }

        Ok(crate::functions::helpers::spill(
            cells,
            nodes.len() + 1,
            COLUMNS,
            transpose,
            headers,
        ))
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.ELEMENTS.CREATE",
    volatile,
    category = "QuantSupport - Market",
    help = "Creates a QuantSupport ConstructedElementStore"
)]
pub fn qs_elements_create(name: &str) -> Result<String, XllError> {
    if let Some(handle) = cached_handle(name, "ConstructedElementStore", "empty")? {
        return Ok(handle);
    }
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::ConstructedElements(Box::default()), "empty")
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CURVE.ADD",
    category = "QuantSupport - Market",
    help = "Adds a curve from QS.CURVE.FLAT to an elements store and returns the updated store handle"
)]
pub fn qs_elements_add_discount_curve(elements: &str, curve: &str) -> Result<String, XllError> {
    let value_key = curve.to_string();
    with_registry_mut(|registry| {
        let curve = registry.discount_curve(curve)?.clone();
        let operation_id = format!("discount_curve:{}", curve.market_index());
        registry.update_constructed_elements_if_changed(
            elements,
            &operation_id,
            &value_key,
            |store| {
                store
                    .discount_curves_mut()
                    .insert(curve.market_index().clone(), curve);
            },
        )
    })
    .map_err(value_error)
}

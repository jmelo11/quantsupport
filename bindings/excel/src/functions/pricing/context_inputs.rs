//! Context-first worksheet workflow with explicit range dependencies.

use crate::{
    functions::helpers::*,
    runtime::{
        context::{self, InputData},
        historical, market_inputs, pricing,
        registry::{with_registry, with_registry_mut},
    },
};
use quantsupport::prelude::{
    CreditCurveConfiguration, DayCounter, Frequency, Interpolator, Level, Request,
    VolatilityCubeConfiguration, VolatilitySurfaceConfiguration,
};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};

#[xll_bindgen(
    name = "QS.CONTEXT.CREATE",
    category = "QuantSupport - Pricing",
    help = "Creates a dated pricing context; pass independent CONTEXT.ADD inputs as a range to CONTEXT.PRICE"
)]
pub fn qs_context_new(
    name: &str,
    reference_date: &str,
    base_currency: &str,
    base_index: &str,
    quote_level: Option<String>,
) -> Result<String, XllError> {
    let level = match quote_level
        .as_deref()
        .unwrap_or("Mid")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "mid" => Level::Mid,
        "bid" => Level::Bid,
        "ask" => Level::Ask,
        _ => return Err(value_error("quote_level must be Mid, Bid, or Ask")),
    };
    let (date, currency, index) = (
        parse_date(reference_date)?,
        parse_currency(base_currency)?,
        parse_index(base_index)?,
    );
    with_registry_mut(|registry| context::create(registry, name, date, currency, index, level))
        .map_err(value_error)
}

fn add(context: &str, data: InputData, key: String) -> Result<String, XllError> {
    with_registry_mut(|registry| {
        // Several additions may be nested in the same worksheet formula.
        // Different definitions must not overwrite one another's cell output.
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        (context, &key).hash(&mut hash);
        let name = registry.cell_name(&format!("context.input.{:016x}", hash.finish()))?;
        context::add(registry, &name, context, data, &key)
    })
    .map_err(value_error)
}

fn date(context: &str) -> Result<quantsupport::prelude::Date, XllError> {
    with_registry(|registry| {
        registry
            .pricing_context(context)
            .map(|c| c.evaluation_date())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.QUOTES",
    category = "QuantSupport - Pricing",
    help = "Adds separate identifier and Mid/Bid/Ask arrays to a context; each array can be a row or column"
)]
pub fn qs_context_add_quotes(
    context: &str,
    identifiers: *const XLOPER12,
    mid: *const XLOPER12,
    bid: *const XLOPER12,
    ask: *const XLOPER12,
) -> Result<String, XllError> {
    let table = worksheet_quotes(identifiers, mid, bid, ask)?;
    let key = input_key(&("quotes", &table))?;
    add(
        context,
        InputData::Quotes(market_inputs::quotes(date(context)?, &table).map_err(value_error)?),
        key,
    )
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.FIXINGS",
    category = "QuantSupport - Pricing",
    help = "Adds fixing history from separate market-index, date and value arrays"
)]
pub fn qs_context_add_fixings(
    context: &str,
    market_indices: *const XLOPER12,
    dates: *const XLOPER12,
    values: *const XLOPER12,
) -> Result<String, XllError> {
    let table = worksheet_columns(&[
        ("market_indices", market_indices),
        ("dates", dates),
        ("values", values),
    ])?;
    let key = input_key(&("fixings", &table))?;
    add(
        context,
        InputData::Fixings(market_inputs::fixings(&table).map_err(value_error)?),
        key,
    )
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.FX",
    category = "QuantSupport - Pricing",
    help = "Adds separate base-currency, quote-currency and rate arrays; one base unit equals rate quote units"
)]
pub fn qs_context_add_fx(
    context: &str,
    base_currencies: *const XLOPER12,
    quote_currencies: *const XLOPER12,
    rates: *const XLOPER12,
) -> Result<String, XllError> {
    let rows = worksheet_columns(&[
        ("base_currencies", base_currencies),
        ("quote_currencies", quote_currencies),
        ("rates", rates),
    ])?;
    let key = input_key(&("fx", &rows))?;
    add(
        context,
        InputData::Fx(market_inputs::fx(&rows).map_err(value_error)?),
        key,
    )
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.CURVE",
    category = "QuantSupport - Pricing",
    help = "Defines a curve from separate identifier and quote-side arrays; omit identifiers to match context quotes"
)]
pub fn qs_context_add_curve(
    context: &str,
    market_index: &str,
    identifiers: *const XLOPER12,
    mid: *const XLOPER12,
    bid: *const XLOPER12,
    ask: *const XLOPER12,
    day_counter: Option<String>,
    interpolator: Option<String>,
    extrapolate: *const XLOPER12,
) -> Result<String, XllError> {
    let table = worksheet_quotes(identifiers, mid, bid, ask)?;
    let dc = day_counter
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(parse_day_counter)
        .transpose()?
        .unwrap_or(DayCounter::Actual360);
    let interpolation = interpolator
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(parse_interpolator)
        .transpose()?
        .unwrap_or(Interpolator::LogLinear);
    let extrapolate = optional_bool(extrapolate, "extrapolate")?.unwrap_or(false);
    let index = parse_index(market_index)?;
    let key = input_key(&("curve", &index, &table, dc, interpolation, extrapolate))?;
    add(
        context,
        context::curve(
            date(context)?,
            index,
            &table,
            dc,
            interpolation,
            extrapolate,
        )
        .map_err(value_error)?,
        key,
    )
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.CREDIT.CURVE",
    category = "QuantSupport - Pricing",
    help = "Defines a credit curve and its CDS quotes directly in a context"
)]
pub fn qs_context_add_credit_curve(
    context: &str,
    market_index: &str,
    currency: &str,
    discount_index: &str,
    recovery: f64,
    identifiers: *const XLOPER12,
    mid: *const XLOPER12,
    bid: *const XLOPER12,
    ask: *const XLOPER12,
    day_counter: Option<String>,
    premium_frequency: Option<String>,
) -> Result<String, XllError> {
    let table = worksheet_quotes(identifiers, mid, bid, ask)?;
    let (ids, quotes) =
        market_inputs::definition_quotes(date(context)?, &table).map_err(value_error)?;
    let dc = day_counter
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(parse_day_counter)
        .transpose()?
        .unwrap_or(DayCounter::Actual360);
    let frequency = premium_frequency
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(parse_frequency)
        .transpose()?
        .unwrap_or(Frequency::Quarterly);
    let configuration = CreditCurveConfiguration::new(
        parse_index(market_index)?,
        parse_currency(currency)?,
        parse_index(discount_index)?,
        recovery,
        ids,
    )
    .with_day_counter(dc)
    .with_premium_frequency(frequency);
    use crate::runtime::configuration_inputs::WorksheetConfiguration;
    configuration.validate().map_err(value_error)?;
    let key = input_key(&("credit", &configuration, &table))?;
    add(
        context,
        InputData::Credit {
            configuration,
            quotes,
        },
        key,
    )
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.VOL.SURFACE",
    category = "QuantSupport - Pricing",
    help = "Defines a volatility surface and its quotes directly in a context"
)]
pub fn qs_context_add_vol_surface(
    context: &str,
    market_index: &str,
    volatility_type: &str,
    smile_type: &str,
    identifiers: *const XLOPER12,
    mid: *const XLOPER12,
    bid: *const XLOPER12,
    ask: *const XLOPER12,
) -> Result<String, XllError> {
    let table = worksheet_quotes(identifiers, mid, bid, ask)?;
    let (ids, quotes) =
        market_inputs::definition_quotes(date(context)?, &table).map_err(value_error)?;
    let configuration = VolatilitySurfaceConfiguration::new(
        parse_index(market_index)?,
        volatility_type.parse().map_err(value_error)?,
        smile(smile_type)?,
        ids,
    );
    let key = input_key(&("surface", &configuration, &table))?;
    add(
        context,
        InputData::Surface {
            configuration,
            quotes,
        },
        key,
    )
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.VOL.CUBE",
    category = "QuantSupport - Pricing",
    help = "Defines a volatility cube and its quotes directly in a context"
)]
pub fn qs_context_add_vol_cube(
    context: &str,
    market_index: &str,
    volatility_type: &str,
    smile_type: &str,
    identifiers: *const XLOPER12,
    mid: *const XLOPER12,
    bid: *const XLOPER12,
    ask: *const XLOPER12,
) -> Result<String, XllError> {
    let table = worksheet_quotes(identifiers, mid, bid, ask)?;
    let (ids, quotes) =
        market_inputs::definition_quotes(date(context)?, &table).map_err(value_error)?;
    let configuration = VolatilityCubeConfiguration::new(
        parse_index(market_index)?,
        volatility_type.parse().map_err(value_error)?,
        smile(smile_type)?,
        ids,
    );
    let key = input_key(&("cube", &configuration, &table))?;
    add(
        context,
        InputData::Cube {
            configuration,
            quotes,
        },
        key,
    )
}

fn request(value: Option<&str>) -> Result<Request, XllError> {
    pricing::parse_requests(&[value
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Value")
        .into()])
    .map(|mut requests| requests.remove(0))
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.ZERO.CURVE",
    category = "QuantSupport - Pricing",
    help = "Defines an explicit zero-rate curve directly in a context from ordered pillars and annual decimal rates"
)]
pub fn qs_context_add_zero_curve(
    context: &str,
    market_index: &str,
    pillars: *const XLOPER12,
    rates: *const XLOPER12,
    day_counter: &str,
    compounding: &str,
    frequency: &str,
    interpolator: &str,
    extrapolate: *const XLOPER12,
) -> Result<String, XllError> {
    use crate::runtime::configuration_inputs::WorksheetConfiguration;
    use quantsupport::prelude::{
        CurveConfiguration, CurveConstruction, CurvePillar, Period, QuoteStore,
    };
    let pillars = string_values(pillars, "pillars")?
        .into_iter()
        .map(|pillar| {
            if let Ok(period) = pillar.parse::<Period>() {
                Ok(CurvePillar::Tenor(period))
            } else {
                parse_date(&pillar).map(CurvePillar::Date)
            }
        })
        .collect::<Result<Vec<_>, XllError>>()?;
    let rates = numeric_values(rates, "rates")?;
    let configuration = CurveConfiguration::new(
        parse_index(market_index)?,
        parse_day_counter(day_counter)?,
        parse_interpolator(interpolator)?,
        optional_bool(extrapolate, "extrapolate")?.unwrap_or(false),
        vec![],
    )
    .with_construction(CurveConstruction::ZeroRate {
        pillars,
        rates,
        compounding: parse_compounding(compounding)?,
        frequency: parse_frequency(frequency)?,
    });
    configuration.validate().map_err(value_error)?;
    let key = input_key(&("zero", &configuration))?;
    add(
        context,
        InputData::Curve {
            configuration,
            quotes: QuoteStore::new(date(context)?),
        },
        key,
    )
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.COMPOSITE.CURVE",
    category = "QuantSupport - Pricing",
    help = "Defines a composite curve from base and spread curve indices in the same context"
)]
pub fn qs_context_add_composite_curve(
    context: &str,
    market_index: &str,
    base_index: &str,
    spread_index: &str,
) -> Result<String, XllError> {
    use crate::runtime::configuration_inputs::WorksheetConfiguration;
    use quantsupport::prelude::{CurveConfiguration, CurveConstruction, QuoteStore};
    let configuration = CurveConfiguration::new(
        parse_index(market_index)?,
        DayCounter::Actual360,
        Interpolator::LogLinear,
        false,
        vec![],
    )
    .with_construction(CurveConstruction::Composite {
        base_index: parse_index(base_index)?,
        spread_index: parse_index(spread_index)?,
    });
    configuration.validate().map_err(value_error)?;
    let key = input_key(&("composite", &configuration))?;
    add(
        context,
        InputData::Curve {
            configuration,
            quotes: QuoteStore::new(date(context)?),
        },
        key,
    )
}

fn smile(value: &str) -> Result<quantsupport::prelude::SmileType, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "strike" => Ok(quantsupport::prelude::SmileType::Strike),
        "delta" => Ok(quantsupport::prelude::SmileType::Delta),
        _ => Err(value_error("smile_type must be Strike or Delta")),
    }
}

#[xll_bindgen(
    name = "QS.CONTEXT.PRICE",
    category = "QuantSupport - Pricing",
    help = "Returns a requested number or table directly; inputs is the complete range of independent CONTEXT.ADD cells"
)]
pub fn qs_context_price(
    context: &str,
    instrument: &str,
    request_name: Option<String>,
    inputs: *const XLOPER12,
    pricer: Option<String>,
    transpose: *const XLOPER12,
    headers: *const XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let request = request(request_name.as_deref())?;
    let inputs = optional_string_values(inputs, "inputs")?;
    let transpose = optional_bool(transpose, "transpose")?;
    let headers = optional_bool(headers, "headers")?;
    let results = with_registry(|registry| {
        let context = context::prepare(registry, context, &inputs)?;
        context::evaluate(
            registry,
            &context,
            instrument,
            &request,
            pricer.as_deref().filter(|s| !s.is_empty()),
        )
    })
    .map_err(value_error)?;
    match request {
        Request::Cashflows => super::valuation::cashflows_output(
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
    name = "QS.CONTEXT.VALUES",
    category = "QuantSupport - Pricing",
    help = "Prices an instrument range against one prepared context and spills scalar results in input order"
)]
pub fn qs_context_values(
    context: &str,
    instruments: *const XLOPER12,
    request_name: Option<String>,
    inputs: *const XLOPER12,
    pricer: Option<String>,
    transpose: *const XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let request = request(request_name.as_deref())?;
    let inputs = optional_string_values(inputs, "inputs")?;
    let instruments = string_values(instruments, "instruments")?;
    if instruments.is_empty() {
        return Err(value_error("supply at least one instrument"));
    }
    let values = with_registry(|registry| {
        let context = context::prepare(registry, context, &inputs)?;
        instruments
            .iter()
            .map(|instrument| {
                let results = context::evaluate(
                    registry,
                    &context,
                    instrument,
                    &request,
                    pricer.as_deref().filter(|s| !s.is_empty()),
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
    name = "QS.HISTORY.VALUES",
    category = "QuantSupport - Pricing",
    help = "Prices each instrument at its origination timestamp, constructing each matched market once per batch"
)]
pub fn qs_history_values(
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

#[cfg(test)]
mod array_boundary_tests {
    use super::*;
    use crate::runtime::registry::CellId;
    use xll_rs::{convert::build_multi, memory::xlAutoFree12};

    struct Array(*mut XLOPER12);
    impl Drop for Array {
        fn drop(&mut self) {
            xlAutoFree12(self.0);
        }
    }
    fn array(values: Vec<XLOPER12>, rows: usize, columns: usize) -> Array {
        Array(build_multi(values, rows, columns))
    }
    fn in_cell<T>(row: i32, run: impl FnOnce() -> Result<T, XllError>) -> Result<T, XllError> {
        with_registry_mut(|registry| {
            registry.begin_cell(CellId {
                sheet: 0xa991,
                row,
                column: 0,
            })
        });
        let result = run();
        with_registry_mut(|registry| registry.finish_cell(result.is_ok()));
        result
    }

    #[test]
    fn independent_arrays_construct_curves_and_reuse_handles_after_transposition() {
        let identifiers = || {
            vec![
                XLOPER12::from_str("FixedRateDeposit_USD_SOFR_6M"),
                XLOPER12::from_str("FixedRateDeposit_USD_SOFR_1Y"),
            ]
        };
        let levels = || vec![XLOPER12::from_f64(0.04), XLOPER12::from_f64(0.05)];
        let ids_column = array(identifiers(), 2, 1);
        let ids_row = array(identifiers(), 1, 2);
        let mid_row = array(levels(), 1, 2);
        let mid_column = array(levels(), 2, 1);
        let root = in_cell(0, || {
            qs_context_new("array.boundary.context", "2026-01-15", "USD", "SOFR", None)
        })
        .unwrap();
        let define = |ids, mid| {
            in_cell(1, || {
                qs_context_add_curve(
                    &root,
                    "SOFR",
                    ids,
                    mid,
                    std::ptr::null(),
                    std::ptr::null(),
                    None,
                    None,
                    &XLOPER12::nil(),
                )
            })
        };
        let first = define(ids_column.0, mid_row.0).unwrap();
        let prepared = with_registry(|registry| {
            context::prepare(registry, &root, std::slice::from_ref(&first))
        })
        .unwrap();
        assert_eq!(prepared.quote_store().quotes().len(), 2);
        assert!(prepared
            .constructed_elements()
            .discount_curve(&quantsupport::prelude::MarketIndex::SOFR)
            .is_some());
        let second = define(ids_row.0, mid_column.0).unwrap();
        assert_eq!(first, second);
        let transposed =
            with_registry(|registry| context::prepare(registry, &root, &[second])).unwrap();
        assert!(std::rc::Rc::ptr_eq(&prepared, &transposed));
        let rectangle = array(
            vec![
                XLOPER12::from_str("Identifier"),
                XLOPER12::from_str("Mid"),
                XLOPER12::from_str("FixedRateDeposit_USD_SOFR_6M"),
                XLOPER12::from_f64(0.04),
            ],
            2,
            2,
        );
        assert!(define(rectangle.0, std::ptr::null()).is_err());
    }
}

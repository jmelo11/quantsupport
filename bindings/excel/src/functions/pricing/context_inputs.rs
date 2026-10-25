//! Context-owned market contributions; evaluation commits a complete calculation pass.

use crate::{
    functions::helpers::*,
    runtime::{
        context::{self, InputData},
        market_inputs,
        registry::{with_registry, with_registry_mut},
    },
};
use quantsupport::prelude::{
    CreditCurveConfiguration, DayCounter, Frequency, Interpolator, Level,
    VolatilityCubeConfiguration, VolatilitySurfaceConfiguration,
};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};

#[xll_bindgen(
    name = "QS.CONTEXT.CREATE",
    category = "QuantSupport - Pricing",
    help = "Creates a stable workbook-scoped pricing context; additions update its owned market definitions"
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
        let name = registry.contribution_name()?;
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
    name = "QS.CONTEXT.ADD.QUOTE",
    category = "QuantSupport - Pricing",
    help = "Adds one quote identifier, finite value and explicit Mid, Bid or Ask level to a context"
)]
pub fn qs_context_add_quote(
    context: &str,
    identifier: &str,
    value: f64,
    level: Option<String>,
) -> Result<String, XllError> {
    use quantsupport::prelude::{Quote, QuoteLevels, QuoteStore};
    if !value.is_finite() {
        return Err(value_error("quote value must be finite"));
    }
    let levels = match level
        .as_deref()
        .unwrap_or("Mid")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "mid" => QuoteLevels::new(Some(value), None, None),
        "bid" => QuoteLevels::new(None, Some(value), None),
        "ask" => QuoteLevels::new(None, None, Some(value)),
        _ => return Err(value_error("level must be Mid, Bid, or Ask")),
    };
    let quote = Quote::new(identifier.trim().parse().map_err(value_error)?, levels);
    let key = input_key(&("quote", &quote))?;
    let mut quotes = QuoteStore::new(date(context)?);
    quotes.add_quote(quote);
    add(context, InputData::Quotes(quotes), key)
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.PRICER.CONFIGURATION",
    category = "QuantSupport - Pricing",
    help = "Adds one native PricerConfiguration JSON object to a context, including typed model assignments"
)]
pub fn qs_context_add_pricer_configuration(context: &str, json: &str) -> Result<String, XllError> {
    let value = serde_json::from_str::<quantsupport::prelude::PricerConfiguration>(json)
        .map_err(value_error)?;
    let key = input_key(&("pricer", &value))?;
    add(context, InputData::Pricer(value), key)
}

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.SIMULATION.CONFIGURATION",
    category = "QuantSupport - Pricing",
    help = "Adds one native SimulationConfiguration JSON object to a context"
)]
pub fn qs_context_add_simulation_configuration(
    context: &str,
    json: &str,
) -> Result<String, XllError> {
    let value = serde_json::from_str::<quantsupport::prelude::SimulationConfiguration>(json)
        .map_err(value_error)?;
    let key = input_key(&("simulation", &value))?;
    add(context, InputData::Simulation(value), key)
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

#[xll_bindgen(
    name = "QS.CONTEXT.ADD.ZERO.CURVE",
    category = "QuantSupport - Pricing",
    help = "Adds zero-rate quotes and their curve configuration to a context"
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
    use quantsupport::prelude::{CurveConfiguration, Quote, QuoteLevels, QuoteStore};
    let pillars = string_values(pillars, "pillars")?;
    let rates = numeric_values(rates, "rates")?;
    if pillars.is_empty() || pillars.len() != rates.len() {
        return Err(value_error(
            "zero rates require equally sized, nonempty pillars and rates",
        ));
    }
    let index = parse_index(market_index)?;
    let index_label = match &index {
        quantsupport::prelude::MarketIndex::Credit(entity) => format!("Credit({entity})"),
        quantsupport::prelude::MarketIndex::Collateral(currency, collateral) => {
            format!("Collateral({currency}/{collateral})")
        }
        _ => index.to_string(),
    };
    let compounding = parse_compounding(compounding)?;
    let frequency = parse_frequency(frequency)?;
    let (currency, level) = with_registry(|registry| {
        registry
            .pricing_context(context)
            .map(|context| (context.base_currency(), context.quote_level()))
    })
    .map_err(value_error)?;
    let mut quotes = QuoteStore::new(date(context)?);
    let mut identifiers = Vec::new();
    for (pillar, rate) in pillars.iter().zip(rates) {
        let identifier =
            format!("ZeroRate_{currency}_{index_label}_{pillar}_{compounding:?}_{frequency:?}");
        let details = identifier.parse().map_err(value_error)?;
        if !rate.is_finite() {
            return Err(value_error("zero rates must be finite"));
        }
        let levels = match level {
            Level::Mid => QuoteLevels::new(Some(rate), None, None),
            Level::Bid => QuoteLevels::new(None, Some(rate), None),
            Level::Ask => QuoteLevels::new(None, None, Some(rate)),
        };
        quotes.add_quote(Quote::new(details, levels));
        identifiers.push(identifier);
    }
    let configuration = CurveConfiguration::new(
        index,
        parse_day_counter(day_counter)?,
        parse_interpolator(interpolator)?,
        optional_bool(extrapolate, "extrapolate")?.unwrap_or(false),
        identifiers,
    );
    configuration.validate().map_err(value_error)?;
    let key = input_key(&("zero", &configuration, &quotes))?;
    add(
        context,
        InputData::Curve {
            configuration,
            quotes,
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
    use quantsupport::prelude::{CurveConfiguration, QuoteStore};
    let configuration = CurveConfiguration::composite(
        parse_index(market_index)?,
        parse_index(base_index)?,
        parse_index(spread_index)?,
    );
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

#[cfg(test)]
mod array_boundary_tests {
    use super::*;
    use crate::runtime::registry::CellId;
    use quantsupport::prelude::MarketDataProvider;
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
            .handle_request(
                &quantsupport::prelude::MarketDataRequest::default()
                    .with_constructed_elements_request(vec![
                        quantsupport::prelude::ConstructedElementRequest::DiscountCurve {
                            market_index: quantsupport::prelude::MarketIndex::SOFR
                        }
                    ])
            )
            .unwrap()
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

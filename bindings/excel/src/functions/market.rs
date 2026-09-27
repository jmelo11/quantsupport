use std::str::FromStr;

use quantsupport::prelude::{
    CreditCurveConfiguration, CurveConfiguration, DualFwd, FixingStore, FxRateRecord, FxStore,
    Level, PricingContext, Quote, QuoteDetails, QuoteLevels, QuoteStore, QuoteStoreRecords,
    Scenario, SimulationConfiguration, VolatilityCubeConfiguration, VolatilitySurfaceConfiguration,
};
use serde::de::DeserializeOwned;
use xll_rs::types::{XllError, XLOPER12};
use xllgen::xll_bindgen;

use crate::registry::{with_registry, with_registry_mut, QsObject};

use super::helpers::{
    bool_value, input_key, not_found, parse_currency, parse_date, parse_day_counter, parse_index,
    parse_interpolator, string_values, value_error,
};

fn parse_json<T: DeserializeOwned>(json: &str, what: &str) -> Result<T, XllError> {
    serde_json::from_str(json).map_err(|error| value_error(format!("invalid {what}: {error}")))
}

fn parse_json_list<T: DeserializeOwned>(json: &str, what: &str) -> Result<Vec<T>, XllError> {
    let value: serde_json::Value = parse_json(json, what)?;
    let normalized = match value {
        serde_json::Value::Array(_) => value,
        serde_json::Value::Object(ref object)
            if object.len() == 1
                && object
                    .values()
                    .next()
                    .is_some_and(serde_json::Value::is_array) =>
        {
            object
                .values()
                .next()
                .cloned()
                .ok_or_else(|| value_error(format!("invalid {what}")))?
        }
        other => serde_json::Value::Array(vec![other]),
    };
    serde_json::from_value(normalized)
        .map_err(|error| value_error(format!("invalid {what}: {error}")))
}

fn cached_handle(name: &str, kind: &str, key: &str) -> Result<Option<String>, XllError> {
    with_registry(|registry| registry.handle_if_unchanged(name, kind, key)).map_err(value_error)
}

#[xll_bindgen(
    name = "QS.QUOTES.CREATE",
    volatile,
    category = "QuantSupport - Market Data",
    help = "Creates a QuantSupport QuoteStore"
)]
pub fn qs_quotes_create(name: &str, reference_date: &str) -> Result<String, XllError> {
    let key = input_key(&reference_date)?;
    if let Some(handle) = cached_handle(name, "QuoteStore", &key)? {
        return Ok(handle);
    }
    let store = QuoteStore::new(parse_date(reference_date)?);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::QuoteStore(Box::new(store)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.QUOTES.FROM.JSON",
    volatile,
    category = "QuantSupport - Market Data",
    help = "Creates a QuoteStore from the library's QuoteStoreRecords JSON shape"
)]
pub fn qs_quotes_from_json(name: &str, json: &str) -> Result<String, XllError> {
    if let Some(handle) = cached_handle(name, "QuoteStore", json)? {
        return Ok(handle);
    }
    let records: QuoteStoreRecords = parse_json(json, "quote store")?;
    let store = QuoteStore::try_from(records).map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::QuoteStore(Box::new(store)), json)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.QUOTES.SET",
    category = "QuantSupport - Market Data",
    help = "Adds or replaces a Quote; unchanged inputs reuse the current QuoteStore revision"
)]
pub fn qs_quotes_set(
    store: &str,
    identifier: &str,
    mid: Option<f64>,
    bid: Option<f64>,
    ask: Option<f64>,
) -> Result<String, XllError> {
    let details = QuoteDetails::from_str(identifier).map_err(value_error)?;
    let quote = Quote::new(details, QuoteLevels::new(mid, bid, ask));
    let operation_id = format!("quote:{}", identifier.trim());
    let value_key = input_key(&(
        mid.map(f64::to_bits),
        bid.map(f64::to_bits),
        ask.map(f64::to_bits),
    ))?;
    with_registry_mut(|registry| {
        registry.update_quote_store_if_changed(store, &operation_id, &value_key, |target| {
            target.add_quote(quote)
        })
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.QUOTES.GET",
    category = "QuantSupport - Market Data",
    help = "Returns the Mid, Bid, or Ask level of a quote"
)]
pub fn qs_quotes_get(store: &str, identifier: &str, level: &str) -> Result<f64, XllError> {
    with_registry(|registry| {
        let quote = registry
            .quote_store(store)
            .map_err(|error| error.to_string())?
            .quote(identifier)
            .ok_or_else(|| format!("quote '{identifier}' was not found"))?;
        match level.trim().to_ascii_lowercase().as_str() {
            "mid" => quote.levels().value(Level::Mid),
            "bid" => quote.levels().value(Level::Bid),
            "ask" => quote.levels().value(Level::Ask),
            _ => return Err(format!("invalid level '{level}'; use Mid, Bid, or Ask")),
        }
        .map_err(|error| error.to_string())
    })
    .map_err(not_found)
}

#[xll_bindgen(
    name = "QS.FIXINGS.CREATE",
    volatile,
    category = "QuantSupport - Market Data",
    help = "Creates an empty QuantSupport FixingStore"
)]
pub fn qs_fixings_create(name: &str) -> Result<String, XllError> {
    if let Some(handle) = cached_handle(name, "FixingStore", "empty")? {
        return Ok(handle);
    }
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::FixingStore(Box::default()), "empty")
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.FIXINGS.FROM.JSON",
    volatile,
    category = "QuantSupport - Market Data",
    help = "Creates a FixingStore from its native JSON representation"
)]
pub fn qs_fixings_from_json(name: &str, json: &str) -> Result<String, XllError> {
    if let Some(handle) = cached_handle(name, "FixingStore", json)? {
        return Ok(handle);
    }
    let store: FixingStore = parse_json(json, "fixing store")?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::FixingStore(Box::new(store)), json)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.FIXINGS.SET",
    category = "QuantSupport - Market Data",
    help = "Adds or replaces a historical fixing; unchanged inputs reuse the current revision"
)]
pub fn qs_fixings_set(
    store: &str,
    market_index: &str,
    date: &str,
    value: f64,
) -> Result<String, XllError> {
    let index = parse_index(market_index)?;
    let date = parse_date(date)?;
    let operation_id = format!("fixing:{market_index}:{date}");
    let value_key = input_key(&value.to_bits())?;
    with_registry_mut(|registry| {
        registry.update_fixing_store_if_changed(store, &operation_id, &value_key, |target| {
            target.add_fixing(&index, date, value)
        })
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.FIXINGS.GET",
    category = "QuantSupport - Market Data",
    help = "Returns a historical fixing"
)]
pub fn qs_fixings_get(store: &str, market_index: &str, date: &str) -> Result<f64, XllError> {
    let index = parse_index(market_index)?;
    let date = parse_date(date)?;
    with_registry(|registry| {
        registry
            .fixing_store(store)
            .map_err(|error| error.to_string())?
            .fixing(&index, date)
            .map_err(|error| error.to_string())
    })
    .map_err(not_found)
}

#[xll_bindgen(
    name = "QS.FX.CREATE",
    volatile,
    aliases = ["QS.CREATE.FX.STORE"],
    category = "QuantSupport - Market Data",
    help = "Creates an empty QuantSupport FxStore"
)]
pub fn qs_fx_create(name: &str) -> Result<String, XllError> {
    if let Some(handle) = cached_handle(name, "FxStore", "empty")? {
        return Ok(handle);
    }
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::FxStore(Box::new(FxStore::new())), "empty")
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.FX.FROM.JSON",
    volatile,
    category = "QuantSupport - Market Data",
    help = "Creates an FxStore from an array of native FxRateRecord objects"
)]
pub fn qs_fx_from_json(name: &str, json: &str) -> Result<String, XllError> {
    if let Some(handle) = cached_handle(name, "FxStore", json)? {
        return Ok(handle);
    }
    let records: Vec<FxRateRecord> = parse_json_list(json, "FX rate records")?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::FxStore(Box::new(FxStore::from_records(records))),
            json,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.FX.SET",
    aliases = ["QS.ADD.FX.QUOTE"],
    category = "QuantSupport - Market Data",
    help = "Adds or replaces an FX rate; unchanged inputs reuse the current FxStore revision"
)]
pub fn qs_fx_set(
    store: &str,
    base_currency: &str,
    quote_currency: &str,
    rate: f64,
) -> Result<String, XllError> {
    let base = parse_currency(base_currency)?;
    let quote = parse_currency(quote_currency)?;
    let operation_id = format!("fx:{base}:{quote}");
    let value_key = input_key(&rate.to_bits())?;
    with_registry_mut(|registry| {
        registry.update_fx_store_if_changed(store, &operation_id, &value_key, |target| {
            target.add_fx_rate(base, quote, DualFwd::from(rate));
        })
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.FX.GET",
    aliases = ["QS.GET.FX.QUOTE"],
    category = "QuantSupport - Market Data",
    help = "Returns a direct, inverse, or triangulated FX rate from an FxStore"
)]
pub fn qs_fx_get(store: &str, base_currency: &str, quote_currency: &str) -> Result<f64, XllError> {
    let base = parse_currency(base_currency)?;
    let quote = parse_currency(quote_currency)?;
    with_registry(|registry| {
        registry
            .fx_store(store)
            .map_err(|error| error.to_string())?
            .get_fx_rate(base, quote)
            .map(|rate| rate.value())
            .map_err(|error| error.to_string())
    })
    .map_err(not_found)
}

macro_rules! json_config_function {
    ($fn_name:ident, $excel_name:literal, $type:ty, $variant:ident, $what:literal, $help:literal) => {
        #[xll_bindgen(
                                                    name = $excel_name,
                                                    volatile,
                                                    category = "QuantSupport - Configuration",
                                                    help = $help
        )]
        pub fn $fn_name(name: &str, json: &str) -> Result<String, XllError> {
            if let Some(handle) = cached_handle(name, stringify!($variant), json)? {
                return Ok(handle);
            }
            let values: Vec<$type> = parse_json_list(json, $what)?;
            with_registry_mut(|registry| {
                registry.upsert_if_changed(name, QsObject::$variant(values), json)
            })
            .map_err(value_error)
        }
    };
}

json_config_function!(
    qs_curve_configs_from_json,
    "QS.CURVE.CONFIGS.FROM.JSON",
    CurveConfiguration,
    CurveConfigurations,
    "CurveConfiguration values",
    "Stores native QuantSupport CurveConfiguration values parsed from JSON"
);
json_config_function!(
    qs_credit_curve_configs_from_json,
    "QS.CREDIT.CURVE.CONFIGS.FROM.JSON",
    CreditCurveConfiguration,
    CreditCurveConfigurations,
    "CreditCurveConfiguration values",
    "Stores native QuantSupport CreditCurveConfiguration values parsed from JSON"
);
json_config_function!(
    qs_vol_surface_configs_from_json,
    "QS.VOL.SURFACE.CONFIGS.FROM.JSON",
    VolatilitySurfaceConfiguration,
    VolatilitySurfaceConfigurations,
    "VolatilitySurfaceConfiguration values",
    "Stores native QuantSupport VolatilitySurfaceConfiguration values parsed from JSON"
);
json_config_function!(
    qs_vol_cube_configs_from_json,
    "QS.VOL.CUBE.CONFIGS.FROM.JSON",
    VolatilityCubeConfiguration,
    VolatilityCubeConfigurations,
    "VolatilityCubeConfiguration values",
    "Stores native QuantSupport VolatilityCubeConfiguration values parsed from JSON"
);
json_config_function!(
    qs_simulation_configs_from_json,
    "QS.SIMULATION.CONFIGS.FROM.JSON",
    SimulationConfiguration,
    SimulationConfigurations,
    "SimulationConfiguration values",
    "Stores native QuantSupport SimulationConfiguration values parsed from JSON"
);
json_config_function!(
    qs_scenarios_from_json,
    "QS.SCENARIOS.FROM.JSON",
    Scenario,
    Scenarios,
    "Scenario values",
    "Stores native QuantSupport Scenario values parsed from JSON"
);

#[xll_bindgen(
    name = "QS.CURVE.CONFIGS.CREATE",
    volatile,
    category = "QuantSupport - Configuration",
    help = "Creates an empty curve-configuration collection for worksheet-built curves"
)]
pub fn qs_curve_configs_create(name: &str) -> Result<String, XllError> {
    if let Some(handle) = cached_handle(name, "CurveConfigurations", "empty")? {
        return Ok(handle);
    }
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::CurveConfigurations(Vec::new()), "empty")
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CURVE.CONFIGS.ADD",
    category = "QuantSupport - Configuration",
    help = "Adds or replaces a bootstrapped curve from an Excel range; unchanged inputs reuse the current handle"
)]
pub fn qs_curve_configs_add(
    configurations: &str,
    market_index: &str,
    day_counter: &str,
    interpolator: &str,
    enable_extrapolation: *const XLOPER12,
    quote_identifiers: *const XLOPER12,
) -> Result<String, XllError> {
    let market_index = parse_index(market_index)?;
    let day_counter = parse_day_counter(day_counter)?;
    let interpolator = parse_interpolator(interpolator)?;
    let enable_extrapolation = bool_value(enable_extrapolation, "enable_extrapolation")?;
    let quote_identifiers = string_values(quote_identifiers, "quote_identifiers")?;
    let operation_id = format!("curve:{market_index}");
    let value_key = input_key(&(
        &market_index,
        day_counter,
        interpolator,
        enable_extrapolation,
        &quote_identifiers,
    ))?;
    let configuration = CurveConfiguration::new(
        market_index,
        day_counter,
        interpolator,
        enable_extrapolation,
        quote_identifiers,
    );
    with_registry_mut(|registry| {
        registry.update_curve_configurations_if_changed(
            configurations,
            &operation_id,
            &value_key,
            |target| {
                if let Some(position) = target
                    .iter()
                    .position(|existing| existing.market_index() == configuration.market_index())
                {
                    target[position] = configuration;
                } else {
                    target.push(configuration);
                }
            },
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.CREATE",
    volatile,
    category = "QuantSupport - Context",
    help = "Builds and initializes a native PricingContext from component handles"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_context_create(
    name: &str,
    quotes: &str,
    curve_configurations: Option<String>,
    credit_curve_configurations: Option<String>,
    fixings: Option<String>,
    fx: Option<String>,
    volatility_surface_configurations: Option<String>,
    volatility_cube_configurations: Option<String>,
    simulation_configurations: Option<String>,
    constructed_elements: Option<String>,
    scenarios: Option<String>,
    base_currency: &str,
    base_index: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        quotes,
        curve_configurations.as_deref(),
        credit_curve_configurations.as_deref(),
        fixings.as_deref(),
        fx.as_deref(),
        volatility_surface_configurations.as_deref(),
        volatility_cube_configurations.as_deref(),
        simulation_configurations.as_deref(),
        constructed_elements.as_deref(),
        scenarios.as_deref(),
        base_currency,
        base_index,
    ))?;
    if let Some(handle) = cached_handle(name, "PricingContext", &definition_key)? {
        return Ok(handle);
    }
    let components = with_registry(|registry| {
        Ok::<_, crate::registry::RegistryError>((
            registry.quote_store(quotes)?.clone(),
            curve_configurations
                .as_deref()
                .map(|handle| registry.curve_configurations(handle).cloned())
                .transpose()?,
            credit_curve_configurations
                .as_deref()
                .map(|handle| registry.credit_curve_configurations(handle).cloned())
                .transpose()?,
            fixings
                .as_deref()
                .map(|handle| registry.fixing_store(handle).cloned())
                .transpose()?,
            fx.as_deref()
                .map(|handle| registry.fx_store(handle).cloned())
                .transpose()?,
            volatility_surface_configurations
                .as_deref()
                .map(|handle| registry.volatility_surface_configurations(handle).cloned())
                .transpose()?,
            volatility_cube_configurations
                .as_deref()
                .map(|handle| registry.volatility_cube_configurations(handle).cloned())
                .transpose()?,
            simulation_configurations
                .as_deref()
                .map(|handle| registry.simulation_configurations(handle).cloned())
                .transpose()?,
            constructed_elements
                .as_deref()
                .map(|handle| registry.constructed_elements(handle).cloned())
                .transpose()?,
            scenarios
                .as_deref()
                .map(|handle| registry.scenarios(handle).cloned())
                .transpose()?,
        ))
    })
    .map_err(value_error)?;

    let (
        quotes,
        curves,
        credit_curves,
        fixings,
        fx,
        surfaces,
        cubes,
        simulations,
        elements,
        scenarios,
    ) = components;
    let mut context = PricingContext::new()
        .with_quote_store(quotes)
        .with_base_currency(parse_currency(base_currency)?)
        .with_base_index(parse_index(base_index)?);
    if let Some(value) = curves {
        context = context.with_curve_configurations(value);
    }
    if let Some(value) = credit_curves {
        context = context.with_credit_curve_configurations(value);
    }
    if let Some(value) = fixings {
        context = context.with_fixing_store(value);
    }
    if let Some(value) = fx {
        context = context.with_fx_store(value);
    }
    if let Some(value) = surfaces {
        context = context.with_volatility_surface_configurations(value);
    }
    if let Some(value) = cubes {
        context = context.with_volatility_cube_configurations(value);
    }
    if let Some(value) = simulations {
        context = context.with_simulation_configurations(value);
    }
    if let Some(value) = elements {
        context = context.with_constructed_elements(value);
    }
    if let Some(value) = scenarios {
        context = context.with_scenarios(value);
    }
    context.initialize().map_err(value_error)?;

    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::PricingContext(Box::new(context)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.CURVES",
    category = "QuantSupport - Context",
    help = "Spills discount-curve indices constructed in a PricingContext"
)]
pub fn qs_context_curves(context: &str) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    with_registry(|registry| {
        let mut indices: Vec<_> = registry
            .pricing_context(context)?
            .constructed_elements()
            .discount_curves()
            .keys()
            .map(ToString::to_string)
            .collect();
        indices.sort();
        Ok::<Vec<String>, crate::registry::RegistryError>(indices)
    })
    .map(super::helpers::string_column)
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.VOL.SURFACES",
    category = "QuantSupport - Context",
    help = "Spills volatility-surface indices constructed in a PricingContext"
)]
pub fn qs_context_vol_surfaces(context: &str) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    with_registry(|registry| {
        let mut indices: Vec<_> = registry
            .pricing_context(context)?
            .constructed_elements()
            .volatility_surfaces()
            .keys()
            .map(ToString::to_string)
            .collect();
        indices.sort();
        Ok::<Vec<String>, crate::registry::RegistryError>(indices)
    })
    .map(super::helpers::string_column)
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.VOL.CUBES",
    category = "QuantSupport - Context",
    help = "Spills volatility-cube indices constructed in a PricingContext"
)]
pub fn qs_context_vol_cubes(context: &str) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    with_registry(|registry| {
        let mut indices: Vec<_> = registry
            .pricing_context(context)?
            .constructed_elements()
            .volatility_cubes()
            .keys()
            .map(ToString::to_string)
            .collect();
        indices.sort();
        Ok::<Vec<String>, crate::registry::RegistryError>(indices)
    })
    .map(super::helpers::string_column)
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.SIMULATIONS",
    category = "QuantSupport - Context",
    help = "Spills simulation indices constructed in a PricingContext"
)]
pub fn qs_context_simulations(context: &str) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    with_registry(|registry| {
        let mut indices: Vec<_> = registry
            .pricing_context(context)?
            .constructed_elements()
            .simulations()
            .keys()
            .map(ToString::to_string)
            .collect();
        indices.sort();
        Ok::<Vec<String>, crate::registry::RegistryError>(indices)
    })
    .map(super::helpers::string_column)
    .map_err(value_error)
}

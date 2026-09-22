use std::str::FromStr;

use quantsupport::prelude::{
    CreditCurveConfiguration, CurveConfiguration, DualFwd, FixingStore, FxRateRecord, FxStore,
    Level, PricingContext, Quote, QuoteDetails, QuoteLevels, QuoteStore, QuoteStoreRecords,
    Scenario, SimulationConfiguration, VolatilityCubeConfiguration, VolatilitySurfaceConfiguration,
};
use serde::de::DeserializeOwned;
use xll_rs::types::XllError;
use xllgen::xll_bindgen;

use crate::registry::{with_registry, with_registry_mut, QsObject};

use super::helpers::{not_found, parse_currency, parse_date, parse_index, value_error};

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

#[xll_bindgen(
    name = "QS.QUOTES.CREATE",
    volatile,
    category = "QuantSupport - Market Data",
    help = "Creates a QuantSupport QuoteStore"
)]
pub fn qs_quotes_create(name: &str, reference_date: &str) -> Result<String, XllError> {
    let store = QuoteStore::new(parse_date(reference_date)?);
    with_registry_mut(|registry| registry.upsert(name, QsObject::QuoteStore(Box::new(store))))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.QUOTES.FROM.JSON",
    volatile,
    category = "QuantSupport - Market Data",
    help = "Creates a QuoteStore from the library's QuoteStoreRecords JSON shape"
)]
pub fn qs_quotes_from_json(name: &str, json: &str) -> Result<String, XllError> {
    let records: QuoteStoreRecords = parse_json(json, "quote store")?;
    let store = QuoteStore::try_from(records).map_err(value_error)?;
    with_registry_mut(|registry| registry.upsert(name, QsObject::QuoteStore(Box::new(store))))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.QUOTES.SET",
    category = "QuantSupport - Market Data",
    help = "Adds or replaces a Quote in a QuoteStore and returns the next revision"
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
    with_registry_mut(|registry| {
        registry
            .update_quote_store(store, |target| target.add_quote(quote))
            .map(|(_, handle)| handle)
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
    with_registry_mut(|registry| registry.upsert(name, QsObject::FixingStore(Box::default())))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.FIXINGS.FROM.JSON",
    volatile,
    category = "QuantSupport - Market Data",
    help = "Creates a FixingStore from its native JSON representation"
)]
pub fn qs_fixings_from_json(name: &str, json: &str) -> Result<String, XllError> {
    let store: FixingStore = parse_json(json, "fixing store")?;
    with_registry_mut(|registry| registry.upsert(name, QsObject::FixingStore(Box::new(store))))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.FIXINGS.SET",
    category = "QuantSupport - Market Data",
    help = "Adds or replaces a historical fixing and returns the next revision"
)]
pub fn qs_fixings_set(
    store: &str,
    market_index: &str,
    date: &str,
    value: f64,
) -> Result<String, XllError> {
    let index = parse_index(market_index)?;
    let date = parse_date(date)?;
    with_registry_mut(|registry| {
        registry
            .update_fixing_store(store, |target| target.add_fixing(&index, date, value))
            .map(|(_, handle)| handle)
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
    with_registry_mut(|registry| registry.upsert(name, QsObject::FxStore(Box::new(FxStore::new()))))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.FX.FROM.JSON",
    volatile,
    category = "QuantSupport - Market Data",
    help = "Creates an FxStore from an array of native FxRateRecord objects"
)]
pub fn qs_fx_from_json(name: &str, json: &str) -> Result<String, XllError> {
    let records: Vec<FxRateRecord> = parse_json_list(json, "FX rate records")?;
    with_registry_mut(|registry| {
        registry.upsert(
            name,
            QsObject::FxStore(Box::new(FxStore::from_records(records))),
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.FX.SET",
    aliases = ["QS.ADD.FX.QUOTE"],
    category = "QuantSupport - Market Data",
    help = "Adds or replaces an FX rate and returns the FxStore's next revision"
)]
pub fn qs_fx_set(
    store: &str,
    base_currency: &str,
    quote_currency: &str,
    rate: f64,
) -> Result<String, XllError> {
    let base = parse_currency(base_currency)?;
    let quote = parse_currency(quote_currency)?;
    with_registry_mut(|registry| {
        registry
            .update_fx_store(store, |target| {
                target.add_fx_rate(base, quote, DualFwd::from(rate));
            })
            .map(|(_, handle)| handle)
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
            let values: Vec<$type> = parse_json_list(json, $what)?;
            with_registry_mut(|registry| registry.upsert(name, QsObject::$variant(values)))
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

    with_registry_mut(|registry| registry.upsert(name, QsObject::PricingContext(Box::new(context))))
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

use crate::functions::helpers::{
    bool_value, cached_handle, input_key, numeric_values, parse_compounding, parse_date,
    parse_day_counter, parse_frequency, parse_index, parse_interpolator, parse_json_list,
    string_values, value_error,
};
use crate::runtime::registry::{with_registry_mut, QsObject};
use quantsupport::prelude::{
    CreditCurveConfiguration, CurveConfiguration, CurveConstruction, CurvePillar, DayCounter,
    Frequency, Interpolator, MarketIndex, Period, VolatilityCubeConfiguration,
    VolatilitySurfaceConfiguration,
};
use xll_rs::types::{XllError, XLOPER12};
use xllgen::xll_bindgen;

fn add_configuration(
    configurations: &str,
    configuration: CurveConfiguration,
) -> Result<String, XllError> {
    let operation_id = format!("curve:{}", configuration.market_index());
    let value_key = input_key(&configuration)?;
    with_registry_mut(|registry| {
        registry.update_curve_configurations_if_changed(
            configurations,
            &operation_id,
            &value_key,
            |values| {
                if let Some(position) = values
                    .iter()
                    .position(|value| value.market_index() == configuration.market_index())
                {
                    values[position] = configuration;
                } else {
                    values.push(configuration);
                }
            },
        )
    })
    .map_err(value_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::registry::with_registry;
    use quantsupport::prelude::{Date, MarketIndex, PricingContext, QuoteStore};

    #[test]
    fn worksheet_zero_rates_and_composites_keep_serializable_construction_inputs() {
        let created = qs_curve_configs_create("test_zero_configs").unwrap();
        let add_zero = |handle: &str, index: &str, rate: f64| {
            qs_curve_configs_add_zero(
                handle,
                index,
                "Actual365",
                "Linear",
                &XLOPER12::from_bool(true),
                &XLOPER12::from_str("0D"),
                &XLOPER12::from_f64(rate),
                "Continuous",
                "Annual",
            )
            .unwrap()
        };
        let base = add_zero(&created, "SOFR", 0.03);
        let spread = add_zero(&base, "TermSOFR3m", 0.01);
        let composite =
            qs_curve_configs_add_composite(&spread, "TermSOFR6m", "SOFR", "TermSOFR3m").unwrap();
        let configurations =
            with_registry(|registry| registry.curve_configurations(&composite).unwrap().clone());
        let mut context = PricingContext::new()
            .with_quote_store(QuoteStore::new(Date::new(2026, 1, 15)))
            .with_curve_configurations(configurations);
        context.initialize().unwrap();
        let df = context
            .constructed_elements()
            .discount_curve(&MarketIndex::TermSOFR6m)
            .unwrap()
            .curve()
            .discount_factor(Date::new(2027, 1, 15))
            .unwrap()
            .value();
        assert!((df - (-0.04_f64).exp()).abs() < 1e-12);
        assert!(quantsupport::prelude::MarketSnapshot::from_context(
            "2026-01-15T16:00:00Z".parse().unwrap(),
            &context
        )
        .is_ok());
        assert_eq!(
            qs_curve_configs_add_composite(&spread, "TermSOFR6m", "SOFR", "TermSOFR3m").unwrap(),
            composite
        );
    }

    #[test]
    fn worksheet_zero_rates_reject_missing_or_nonfinite_numeric_cells() {
        let created = qs_curve_configs_create("test_bad_zero_configs").unwrap();
        for rate in [
            XLOPER12::nil(),
            XLOPER12::from_str("0.03"),
            XLOPER12::from_f64(f64::NAN),
            XLOPER12::from_f64(f64::INFINITY),
        ] {
            assert!(qs_curve_configs_add_zero(
                &created,
                "SOFR",
                "Actual365",
                "Linear",
                &XLOPER12::from_bool(true),
                &XLOPER12::from_str("0D"),
                &rate,
                "Continuous",
                "Annual"
            )
            .is_err());
        }
        let pillars = xll_rs::convert::build_multi(
            vec![XLOPER12::from_str("0D"), XLOPER12::from_str("1Y")],
            2,
            1,
        );
        let result = qs_curve_configs_add_zero(
            &created,
            "SOFR",
            "Actual365",
            "Linear",
            &XLOPER12::from_bool(true),
            pillars,
            &XLOPER12::from_f64(0.03),
            "Continuous",
            "Annual",
        );
        xll_rs::memory::xlAutoFree12(pillars);
        assert!(result.is_err());
    }
}

#[xll_bindgen(
    name = "QS.CURVE.CONFIGS.ADD.ZERO",
    category = "QuantSupport - Market",
    help = "Adds a snapshot-compatible explicit zero-rate curve from ordered ISO date or tenor text and numeric rate ranges"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_curve_configs_add_zero(
    configurations: &str,
    market_index: &str,
    day_counter: &str,
    interpolator: &str,
    enable_extrapolation: *const XLOPER12,
    pillars: *const XLOPER12,
    rates: *const XLOPER12,
    compounding: &str,
    frequency: &str,
) -> Result<String, XllError> {
    let pillars = string_values(pillars, "pillars")?
        .into_iter()
        .map(|value| {
            parse_date(&value).map(CurvePillar::Date).or_else(|_| {
                value
                    .trim()
                    .parse::<Period>()
                    .map(CurvePillar::Tenor)
                    .map_err(value_error)
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rates = numeric_values(rates, "rates")?;
    if pillars.len() != rates.len() {
        return Err(value_error(
            "pillars and rates must contain the same number of values",
        ));
    }
    let configuration = CurveConfiguration::new(
        parse_index(market_index)?,
        parse_day_counter(day_counter)?,
        parse_interpolator(interpolator)?,
        bool_value(enable_extrapolation, "enable_extrapolation")?,
        Vec::new(),
    )
    .with_construction(CurveConstruction::ZeroRate {
        pillars,
        rates,
        compounding: parse_compounding(compounding)?,
        frequency: parse_frequency(frequency)?,
    });
    add_configuration(configurations, configuration)
}

#[xll_bindgen(
    name = "QS.CURVE.CONFIGS.ADD.COMPOSITE",
    category = "QuantSupport - Market",
    help = "Adds a snapshot-compatible composite curve multiplying the discount factors of two configured indices"
)]
pub fn qs_curve_configs_add_composite(
    configurations: &str,
    market_index: &str,
    base_index: &str,
    spread_index: &str,
) -> Result<String, XllError> {
    let configuration = CurveConfiguration::new(
        parse_index(market_index)?,
        DayCounter::Actual360,
        Interpolator::LogLinear,
        true,
        Vec::new(),
    )
    .with_construction(CurveConstruction::Composite {
        base_index: parse_index(base_index)?,
        spread_index: parse_index(spread_index)?,
    });
    add_configuration(configurations, configuration)
}

#[xll_bindgen(
    name = "QS.CREDIT.CURVE.CONFIGS.CREATE",
    volatile,
    category = "QuantSupport - Market",
    help = "Creates an empty credit-curve configuration collection for worksheet quote-range builders"
)]
pub fn qs_credit_curve_configs_create(name: &str) -> Result<String, XllError> {
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::CreditCurveConfigurations(Vec::new()),
            "empty",
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CREDIT.CURVE.CONFIGS.ADD",
    category = "QuantSupport - Market",
    help = "Adds a borrower credit curve using worksheet arguments and a range of CDS quote identifiers"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_credit_curve_configs_add(
    configurations: &str,
    market_index: &str,
    currency: &str,
    discount_index: &str,
    recovery: f64,
    quotes: *const XLOPER12,
    day_counter: Option<String>,
    premium_frequency: Option<String>,
) -> Result<String, XllError> {
    let index = parse_index(market_index)?;
    if !matches!(index, MarketIndex::Credit(_)) {
        return Err(value_error("market_index must be Credit:entity"));
    }
    if !recovery.is_finite() || !(0.0..=1.0).contains(&recovery) {
        return Err(value_error("recovery must be in [0, 1]"));
    }
    let configuration = CreditCurveConfiguration::new(
        index,
        crate::functions::helpers::parse_currency(currency)?,
        parse_index(discount_index)?,
        recovery,
        string_values(quotes, "quotes")?,
    )
    .with_day_counter(
        day_counter
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(parse_day_counter)
            .transpose()?
            .unwrap_or(DayCounter::Actual360),
    )
    .with_premium_frequency(
        premium_frequency
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(parse_frequency)
            .transpose()?
            .unwrap_or(Frequency::Quarterly),
    );
    let operation = format!("credit:{}", configuration.market_index());
    let key = input_key(&configuration)?;
    with_registry_mut(|registry| {
        registry.update_credit_curve_configurations_if_changed(
            configurations,
            &operation,
            &key,
            |values| {
                if let Some(position) = values
                    .iter()
                    .position(|value| value.market_index() == configuration.market_index())
                {
                    values[position] = configuration;
                } else {
                    values.push(configuration);
                }
            },
        )
    })
    .map_err(value_error)
}

macro_rules! json_config_function {
    ($fn_name:ident, $excel_name:literal, $type:ty, $variant:ident, $what:literal, $help:literal) => {
        #[xll_bindgen(
                                                            name = $excel_name,
                                                            volatile,
                                                            category = "QuantSupport - Market",
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
#[xll_bindgen(
    name = "QS.CURVE.CONFIGS.CREATE",
    volatile,
    category = "QuantSupport - Market",
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
    category = "QuantSupport - Market",
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

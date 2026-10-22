use crate::functions::helpers::{
    cached_handle, input_key, parse_currency, parse_index, value_error,
};
use crate::runtime::registry::{with_registry, with_registry_mut, QsObject};
use quantsupport::prelude::{
    ConstructedElementRequest, Level, MarketDataProvider, MarketDataRequest, PricingContext,
    QuoteSelector, QuoteStore, Scenario,
};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::XllError;

#[xll_bindgen(
    name = "QS.CONTEXT.FROM.SNAPSHOT",
    category = "QuantSupport - Pricing",
    help = "Builds and initializes a context from the archive's selected complete market; fixings are supplied separately"
)]
pub fn qs_context_from_snapshot(
    name: &str,
    store: &str,
    selector: &str,
    fixings: Option<String>,
) -> Result<String, XllError> {
    with_registry_mut(|registry| {
        crate::runtime::historical::pricing_context(
            registry,
            name,
            store,
            selector,
            fixings.as_deref(),
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.EVALUATION.DATE",
    category = "QuantSupport - Pricing",
    help = "Returns the context's valuation date from its market quote reference date"
)]
pub fn qs_context_evaluation_date(context: &str) -> Result<String, XllError> {
    with_registry(|registry| {
        registry
            .pricing_context(context)
            .map(|value| value.evaluation_date().to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.SNAPSHOT.TIMESTAMP",
    category = "QuantSupport - Pricing",
    help = "Returns the matched snapshot timestamp in UTC, or empty text for a context without historical provenance"
)]
pub fn qs_context_snapshot_timestamp(context: &str) -> Result<String, XllError> {
    with_registry(|registry| {
        registry.pricing_context(context).map(|value| {
            value
                .snapshot_timestamp()
                .map(|instant| instant.to_string())
                .unwrap_or_default()
        })
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.QUOTE.LEVEL",
    category = "QuantSupport - Pricing",
    help = "Returns the quote side used for market construction: Mid, Bid, or Ask"
)]
pub fn qs_context_quote_level(context: &str) -> Result<String, XllError> {
    with_registry(|registry| {
        registry
            .pricing_context(context)
            .map(|value| format!("{:?}", value.quote_level()))
    })
    .map_err(value_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::historical;
    use quantsupport::prelude::Date;

    #[test]
    fn historical_context_worksheet_getters_expose_actual_date_timestamp_and_side() {
        let (store, selector, base) = with_registry_mut(|registry| {
            let context =
                PricingContext::new().with_quote_store(QuoteStore::new(Date::new(2026, 1, 15)));
            let context = registry
                .upsert_if_changed(
                    "getter_market",
                    QsObject::PricingContext(Box::new(context)),
                    "market",
                )
                .unwrap();
            let snapshot = historical::capture(
                registry,
                "getter_snapshot",
                historical::timestamp("2026-01-16T00:30:00Z").unwrap(),
                &context,
            )
            .unwrap();
            let store = historical::archive(registry, "getter_archive", &[snapshot]).unwrap();
            let selector_value =
                historical::selector("2026-01-20T16:00:00Z", None, Some("Ask")).unwrap();
            let selector =
                historical::store_selector(registry, "getter_selector", selector_value).unwrap();
            (store, selector, context)
        });
        let context =
            qs_context_from_snapshot("getter_historical", &store, &selector, None).unwrap();
        assert_eq!(qs_context_evaluation_date(&context).unwrap(), "2026-01-15");
        assert_eq!(
            qs_context_snapshot_timestamp(&context).unwrap(),
            "2026-01-16T00:30:00+00:00"
        );
        assert_eq!(qs_context_quote_level(&context).unwrap(), "Ask");
        assert_eq!(qs_context_snapshot_timestamp(&base).unwrap(), "");
        assert_eq!(
            qs_context_from_snapshot("getter_historical", &store, &selector, None).unwrap(),
            context
        );
    }
}

#[xll_bindgen(
    name = "QS.CONTEXT.CREATE",
    category = "QuantSupport - Pricing",
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
    pricer_configurations: Option<String>,
    simulation_configurations: Option<String>,
    constructed_elements: Option<String>,
    scenarios: Option<String>,
    base_currency: &str,
    base_index: &str,
    quote_level: Option<String>,
) -> Result<String, XllError> {
    let quote_level = match quote_level
        .as_deref()
        .unwrap_or("Mid")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "mid" => Level::Mid,
        "bid" => Level::Bid,
        "ask" => Level::Ask,
        _ => return Err(value_error("invalid quote level; use Mid, Bid, or Ask")),
    };
    let definition_key = input_key(&(
        quotes,
        curve_configurations.as_deref(),
        credit_curve_configurations.as_deref(),
        fixings.as_deref(),
        fx.as_deref(),
        volatility_surface_configurations.as_deref(),
        volatility_cube_configurations.as_deref(),
        pricer_configurations.as_deref(),
        simulation_configurations.as_deref(),
        constructed_elements.as_deref(),
        scenarios.as_deref(),
        base_currency,
        base_index,
        quote_level,
    ))?;
    if let Some(handle) = cached_handle(name, "PricingContext", &definition_key)? {
        return Ok(handle);
    }
    let components = with_registry(|registry| {
        Ok::<_, crate::runtime::registry::RegistryError>((
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
            pricer_configurations
                .as_deref()
                .map(|handle| registry.pricer_configurations(handle).cloned())
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
        pricer_configs,
        simulations,
        elements,
        scenarios,
    ) = components;
    let mut context = PricingContext::new()
        .with_quote_level(quote_level)
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
    if let Some(value) = pricer_configs {
        context = context.with_pricer_configurations(value);
    }
    if let Some(value) = simulations {
        context = context.with_simulation_configurations(value);
    }
    if let Some(value) = elements {
        context = context.with_constructed_elements(value);
    }
    context.initialize().map_err(value_error)?;
    if let Some(definitions) = scenarios {
        // CONTEXT.CREATE produces an owned worksheet snapshot. Native and Python
        // valuations use borrowed scenario views directly.
        let scenario = Scenario::new(
            definitions
                .into_iter()
                .flat_map(|definition| definition.shocks().to_vec())
                .collect(),
        );
        let view = scenario.apply(&context).map_err(value_error)?;
        let elements = context.constructed_elements();
        let requests = elements
            .discount_curves()
            .keys()
            .map(|index| ConstructedElementRequest::DiscountCurve {
                market_index: index.clone(),
            })
            .chain(elements.credit_curves().keys().map(|index| {
                ConstructedElementRequest::CreditCurve {
                    market_index: index.clone(),
                }
            }))
            .chain(elements.volatility_surfaces().keys().map(|index| {
                ConstructedElementRequest::VolatilitySurface {
                    market_index: index.clone(),
                }
            }))
            .chain(elements.volatility_cubes().keys().map(|index| {
                ConstructedElementRequest::VolatilityCube {
                    market_index: index.clone(),
                }
            }))
            .chain(elements.simulations().keys().map(|index| {
                ConstructedElementRequest::Simulation {
                    market_index: index.clone(),
                }
            }))
            .chain(elements.dividend_curves().keys().map(|index| {
                ConstructedElementRequest::DividendCurve {
                    market_index: index.clone(),
                }
            }))
            .collect();
        let market = view
            .handle_request(
                &MarketDataRequest::default().with_constructed_elements_request(requests),
            )
            .map_err(value_error)?;
        let mut quotes = QuoteStore::new(context.evaluation_date());
        for identifier in context.quote_store().quotes().keys() {
            if let Some(quote) = view.select(identifier) {
                quotes.add_quote(quote);
            }
        }
        context = context
            .with_quote_store(quotes)
            .with_constructed_elements(market.constructed_elements().clone());
    }

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
    category = "QuantSupport - Pricing",
    help = "Spills discount-curve indices constructed in a PricingContext"
)]
pub fn qs_context_curves(
    context: &str,
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    with_registry(|registry| {
        let mut indices: Vec<_> = registry
            .pricing_context(context)?
            .constructed_elements()
            .discount_curves()
            .keys()
            .map(ToString::to_string)
            .collect();
        indices.sort();
        Ok::<Vec<String>, crate::runtime::registry::RegistryError>(indices)
    })
    .map(|values| crate::functions::helpers::string_column(values, transpose))
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.VOL.SURFACES",
    category = "QuantSupport - Pricing",
    help = "Spills volatility-surface indices constructed in a PricingContext"
)]
pub fn qs_context_vol_surfaces(
    context: &str,
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    with_registry(|registry| {
        let mut indices: Vec<_> = registry
            .pricing_context(context)?
            .constructed_elements()
            .volatility_surfaces()
            .keys()
            .map(ToString::to_string)
            .collect();
        indices.sort();
        Ok::<Vec<String>, crate::runtime::registry::RegistryError>(indices)
    })
    .map(|values| crate::functions::helpers::string_column(values, transpose))
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.VOL.CUBES",
    category = "QuantSupport - Pricing",
    help = "Spills volatility-cube indices constructed in a PricingContext"
)]
pub fn qs_context_vol_cubes(
    context: &str,
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    with_registry(|registry| {
        let mut indices: Vec<_> = registry
            .pricing_context(context)?
            .constructed_elements()
            .volatility_cubes()
            .keys()
            .map(ToString::to_string)
            .collect();
        indices.sort();
        Ok::<Vec<String>, crate::runtime::registry::RegistryError>(indices)
    })
    .map(|values| crate::functions::helpers::string_column(values, transpose))
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.SIMULATIONS",
    category = "QuantSupport - Pricing",
    help = "Spills simulation indices constructed in a PricingContext"
)]
pub fn qs_context_simulations(
    context: &str,
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    with_registry(|registry| {
        let mut indices: Vec<_> = registry
            .pricing_context(context)?
            .constructed_elements()
            .simulations()
            .keys()
            .map(ToString::to_string)
            .collect();
        indices.sort();
        Ok::<Vec<String>, crate::runtime::registry::RegistryError>(indices)
    })
    .map(|values| crate::functions::helpers::string_column(values, transpose))
    .map_err(value_error)
}

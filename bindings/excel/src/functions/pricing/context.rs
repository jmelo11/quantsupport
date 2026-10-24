use crate::functions::helpers::value_error;
#[cfg(test)]
use crate::runtime::registry::QsObject;
use crate::runtime::registry::{with_registry, with_registry_mut};
#[cfg(test)]
use quantsupport::prelude::{PricingContext, QuoteStore};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::XllError;

#[xll_bindgen(
    name = "QS.CONTEXT.QUOTE",
    category = "QuantSupport - Pricing",
    help = "Reads a quote side from a prepared context; missing sides never fall back"
)]
pub fn qs_context_quote(
    context: &str,
    identifier: &str,
    level: Option<String>,
) -> Result<f64, XllError> {
    use quantsupport::prelude::Level;
    let level = match level
        .as_deref()
        .unwrap_or("Mid")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "mid" => Level::Mid,
        "bid" => Level::Bid,
        "ask" => Level::Ask,
        _ => return Err(value_error("level must be Mid, Bid, or Ask")),
    };
    with_registry(|registry| {
        crate::runtime::context::resolve(registry, context)?
            .quote_store()
            .quote(identifier)
            .ok_or_else(|| format!("quote not found: {identifier}"))?
            .levels()
            .value(level)
            .map_err(|e| e.to_string())
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.CONTEXT.FIXING",
    category = "QuantSupport - Pricing",
    help = "Reads an exact historical fixing from a prepared context in the index's native units"
)]
pub fn qs_context_fixing(context: &str, market_index: &str, date: &str) -> Result<f64, XllError> {
    let index = crate::functions::helpers::parse_index(market_index)?;
    let date = crate::functions::helpers::parse_date(date)?;
    with_registry(|registry| {
        crate::runtime::context::resolve(registry, context)?
            .fixing_store()
            .fixing(&index, date)
            .map_err(|e| e.to_string())
    })
    .map_err(value_error)
}

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
                    QsObject::PricingContext(std::rc::Rc::new(context)),
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
        let market = crate::runtime::context::resolve(registry, context)?;
        let mut indices: Vec<_> = market
            .curve_configurations()
            .iter()
            .map(|c| c.market_index().to_string())
            .chain(
                market
                    .constructed_elements()
                    .discount_curves()
                    .keys()
                    .map(ToString::to_string),
            )
            .collect();
        indices.sort();
        indices.dedup();
        Ok::<Vec<String>, String>(indices)
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
        let market = crate::runtime::context::resolve(registry, context)?;
        let mut indices: Vec<_> = market
            .volatility_surface_configurations()
            .iter()
            .map(|c| c.market_index().to_string())
            .chain(
                market
                    .constructed_elements()
                    .volatility_surfaces()
                    .keys()
                    .map(ToString::to_string),
            )
            .collect();
        indices.sort();
        indices.dedup();
        Ok::<Vec<String>, String>(indices)
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
        let market = crate::runtime::context::resolve(registry, context)?;
        let mut indices: Vec<_> = market
            .volatility_cube_configurations()
            .iter()
            .map(|c| c.market_index().to_string())
            .chain(
                market
                    .constructed_elements()
                    .volatility_cubes()
                    .keys()
                    .map(ToString::to_string),
            )
            .collect();
        indices.sort();
        indices.dedup();
        Ok::<Vec<String>, String>(indices)
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
        let market = crate::runtime::context::resolve(registry, context)?;
        let mut indices: Vec<_> = market
            .simulation_configurations()
            .iter()
            .map(|c| c.market_index().to_string())
            .chain(
                market
                    .constructed_elements()
                    .simulations()
                    .keys()
                    .map(ToString::to_string),
            )
            .collect();
        indices.sort();
        indices.dedup();
        Ok::<Vec<String>, String>(indices)
    })
    .map(|values| crate::functions::helpers::string_column(values, transpose))
    .map_err(value_error)
}

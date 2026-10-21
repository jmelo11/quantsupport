use crate::functions::helpers::{
    cached_handle, input_key, not_found, parse_currency, parse_date, parse_index, parse_json,
    parse_json_list, value_error,
};
use crate::runtime::registry::{with_registry, with_registry_mut, QsObject};
use quantsupport::prelude::{
    DualFwd, FixingStore, FxRateRecord, FxStore, Level, Quote, QuoteDetails, QuoteLevels,
    QuoteStore, QuoteStoreRecords,
};
use quantsupport_xll_macros::xll_bindgen;
use std::str::FromStr;
use xll_rs::types::XllError;

#[xll_bindgen(
    name = "QS.QUOTES.CREATE",
    category = "QuantSupport - Market",
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
    category = "QuantSupport - Market",
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
    category = "QuantSupport - Market",
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
    let operation_id = format!("quote:{}", identifier.trim().to_ascii_lowercase());
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
    category = "QuantSupport - Market",
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
    category = "QuantSupport - Market",
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
    category = "QuantSupport - Market",
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
    category = "QuantSupport - Market",
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
    let operation_id = format!("fixing:{index}:{date}");
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
    category = "QuantSupport - Market",
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
    aliases = ["QS.CREATE.FX.STORE"],
    category = "QuantSupport - Market",
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
    category = "QuantSupport - Market",
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
    category = "QuantSupport - Market",
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
    category = "QuantSupport - Market",
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

#[cfg(test)]
mod ownership_abi_tests {
    #[test]
    fn invalid_abi_arguments_release_the_previous_object_before_typed_code_runs() {
        use super::xl_qs_quotes_create;
        use crate::runtime::{
            lifecycle::windows::set_test_caller,
            registry::{live_object_count, CellId},
        };
        use xll_rs::memory::xlAutoFree12;
        use xll_rs::types::{XLOPER12, XLTYPE_ERR};
        struct CallerReset;
        impl Drop for CallerReset {
            fn drop(&mut self) {
                set_test_caller(None);
            }
        }
        let _reset = CallerReset;
        set_test_caller(Some((
            "ArgumentScope.xlsx".into(),
            "Inputs".into(),
            CellId {
                sheet: 908,
                row: 0,
                column: 0,
            },
            Some("=QS.QUOTES.CREATE(A2,A3)".into()),
        )));
        let before = live_object_count(true);
        let name = Box::into_raw(Box::new(XLOPER12::from_str("scoped")));
        let date = Box::into_raw(Box::new(XLOPER12::from_str("2026-01-15")));
        let created = xl_qs_quotes_create(name, date);
        assert!(unsafe { &*created }
            .as_string()
            .unwrap()
            .starts_with("QSObject:QuoteStore:"));
        assert_eq!(live_object_count(true), before + 1);
        xlAutoFree12(created);
        let failed = xl_qs_quotes_create(name, &XLOPER12::from_f64(123.0));
        assert_eq!(unsafe { &*failed }.base_type(), XLTYPE_ERR);
        assert_eq!(
            live_object_count(true),
            before,
            "conversion failure must not retain the previous object"
        );
        xlAutoFree12(failed);
        xlAutoFree12(name);
        xlAutoFree12(date);
    }
}

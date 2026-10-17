//! Portable implementations of the historical worksheet object workflow.

use quantsupport::prelude::{
    DateTime, Level, MarketSnapshot, MarketSnapshotStore, SnapshotSelectionRule, SnapshotSelector,
};

use super::registry::{QsObject, Registry};

/// Parse an offset-aware observation instant, preserving nanosecond precision.
pub fn timestamp(value: &str) -> Result<DateTime, String> {
    value
        .trim()
        .parse::<DateTime>()
        .map_err(|error| error.to_string())
}

/// Native selector defaults are Previous and Mid; unknown values are errors.
pub fn selector(
    timestamp_text: &str,
    rule: Option<&str>,
    level: Option<&str>,
) -> Result<SnapshotSelector, String> {
    let rule = match rule
        .unwrap_or("Previous")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "previous" => SnapshotSelectionRule::Previous,
        "exact" => SnapshotSelectionRule::Exact,
        "closest" => SnapshotSelectionRule::Closest,
        _ => return Err("Invalid snapshot rule; use Exact, Previous, or Closest".into()),
    };
    let level = match level.unwrap_or("Mid").trim().to_ascii_lowercase().as_str() {
        "" | "mid" => Level::Mid,
        "bid" => Level::Bid,
        "ask" => Level::Ask,
        _ => return Err("Invalid quote level; use Mid, Bid, or Ask".into()),
    };
    Ok(SnapshotSelector::new(timestamp(timestamp_text)?)
        .with_rule(rule)
        .with_level(level))
}

fn key(value: &impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

/// Capture serializable base inputs; native code rejects supplied runtime curves.
pub fn capture(
    registry: &mut Registry,
    name: &str,
    instant: DateTime,
    context: &str,
) -> Result<String, String> {
    let context_value = registry
        .pricing_context(context)
        .map_err(|error| error.to_string())?;
    let definition = key(&(instant, context))?;
    if let Some(handle) = registry
        .handle_if_unchanged(name, "MarketSnapshot", &definition)
        .map_err(|error| error.to_string())?
    {
        return Ok(handle);
    }
    let snapshot =
        MarketSnapshot::from_context(instant, context_value).map_err(|error| error.to_string())?;
    registry
        .upsert_if_changed(
            name,
            QsObject::MarketSnapshot(Box::new(snapshot)),
            definition,
        )
        .map_err(|error| error.to_string())
}

/// Rebuild an archive from the full worksheet range, so removed rows disappear.
pub fn archive(
    registry: &mut Registry,
    name: &str,
    snapshots: &[String],
) -> Result<String, String> {
    // Validate every dependency before returning a cached constructor handle.
    for handle in snapshots {
        registry
            .market_snapshot(handle)
            .map_err(|error| error.to_string())?;
    }
    let definition = key(&snapshots)?;
    if let Some(handle) = registry
        .handle_if_unchanged(name, "MarketSnapshotStore", &definition)
        .map_err(|error| error.to_string())?
    {
        return Ok(handle);
    }
    let mut store = MarketSnapshotStore::new();
    for handle in snapshots {
        let snapshot = registry
            .market_snapshot(handle)
            .map_err(|error| error.to_string())?;
        store
            .insert(snapshot.clone())
            .map_err(|error| error.to_string())?;
    }
    registry
        .upsert_if_changed(
            name,
            QsObject::MarketSnapshotStore(Box::new(store)),
            definition,
        )
        .map_err(|error| error.to_string())
}

/// Construct a selector object; normalized timestamps share the same definition.
pub fn store_selector(
    registry: &mut Registry,
    name: &str,
    selector: SnapshotSelector,
) -> Result<String, String> {
    let definition = key(&selector)?;
    registry
        .upsert_if_changed(
            name,
            QsObject::SnapshotSelector(Box::new(selector)),
            definition,
        )
        .map_err(|error| error.to_string())
}

/// Select one complete market, with the actual matched timestamp on its handle.
pub fn select(
    registry: &mut Registry,
    name: &str,
    store: &str,
    selector: &str,
) -> Result<String, String> {
    let archive = registry
        .market_snapshot_store(store)
        .map_err(|error| error.to_string())?;
    let selector_value = registry
        .snapshot_selector(selector)
        .map_err(|error| error.to_string())?;
    let definition = key(&(store, selector))?;
    if let Some(handle) = registry
        .handle_if_unchanged(name, "MarketSnapshot", &definition)
        .map_err(|error| error.to_string())?
    {
        return Ok(handle);
    }
    let snapshot = archive
        .select(selector_value)
        .map_err(|error| error.to_string())?
        .clone();
    registry
        .upsert_if_changed(
            name,
            QsObject::MarketSnapshot(Box::new(snapshot)),
            definition,
        )
        .map_err(|error| error.to_string())
}

/// Build an initialized Excel context at the selected market's business date.
pub fn pricing_context(
    registry: &mut Registry,
    name: &str,
    store: &str,
    selector: &str,
    fixings: Option<&str>,
) -> Result<String, String> {
    let archive = registry
        .market_snapshot_store(store)
        .map_err(|error| error.to_string())?;
    let selector_value = registry
        .snapshot_selector(selector)
        .map_err(|error| error.to_string())?;
    let fixings_value = fixings
        .map(|handle| registry.fixing_store(handle))
        .transpose()
        .map_err(|error| error.to_string())?;
    let definition = key(&(store, selector, fixings))?;
    if let Some(handle) = registry
        .handle_if_unchanged(name, "PricingContext", &definition)
        .map_err(|error| error.to_string())?
    {
        return Ok(handle);
    }
    let mut context = archive
        .pricing_context(selector_value, fixings_value.cloned().unwrap_or_default())
        .map_err(|error| error.to_string())?;
    context.initialize().map_err(|error| error.to_string())?;
    registry
        .upsert_if_changed(
            name,
            QsObject::PricingContext(Box::new(context)),
            definition,
        )
        .map_err(|error| error.to_string())
}

/// Restore the same JSON snapshot array used by Python and Rust.
pub fn from_json(registry: &mut Registry, name: &str, json: &str) -> Result<String, String> {
    if let Some(handle) = registry
        .handle_if_unchanged(name, "MarketSnapshotStore", json)
        .map_err(|error| error.to_string())?
    {
        return Ok(handle);
    }
    let store: MarketSnapshotStore =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
    registry
        .upsert_if_changed(name, QsObject::MarketSnapshotStore(Box::new(store)), json)
        .map_err(|error| error.to_string())
}

/// Serialize native construction inputs, excluding fixing histories and AD state.
pub fn to_json(registry: &Registry, store: &str) -> Result<String, String> {
    let store = registry
        .market_snapshot_store(store)
        .map_err(|error| error.to_string())?;
    serde_json::to_string(store).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quantsupport::prelude::*;

    fn market(registry: &mut Registry, name: &str, day: u32, rate: f64) -> String {
        let date = Date::new(2025, 1, day);
        let mut quotes = QuoteStore::new(date);
        quotes.add_quote(Quote::new(
            "FixedRateDeposit_USD_SOFR_6M".parse().unwrap(),
            QuoteLevels::new(Some(rate), Some(rate - 0.01), Some(rate + 0.01)),
        ));
        let mut context = PricingContext::new()
            .with_quote_store(quotes)
            .with_curve_configurations(vec![serde_json::from_value(
            serde_json::json!({"market_index":"SOFR","quotes":["FixedRateDeposit_USD_SOFR_6M"]}),
        )
        .unwrap()]);
        context.initialize().unwrap();
        registry
            .upsert_if_changed(
                name,
                QsObject::PricingContext(Box::new(context)),
                format!("{date}:{rate}"),
            )
            .unwrap()
    }

    fn discount(registry: &Registry, context: &str) -> f64 {
        registry
            .pricing_context(context)
            .unwrap()
            .constructed_elements()
            .discount_curve(&MarketIndex::SOFR)
            .unwrap()
            .curve()
            .discount_factor(Date::new(2025, 6, 15))
            .unwrap()
            .value()
    }

    #[test]
    fn selectors_preserve_native_defaults_precision_and_tie_rules() {
        let instant = timestamp("2025-01-16T00:00:00.123456789+02:00").unwrap();
        assert_eq!(instant.to_string(), "2025-01-15T22:00:00.123456789+00:00");
        let default = selector("2025-01-15T12:00:00Z", None, None).unwrap();
        assert_eq!(default.rule(), SnapshotSelectionRule::Previous);
        assert_eq!(default.level(), Level::Mid);
        let closest = selector("2025-01-15T12:00:00Z", Some(" cLoSeSt "), Some("bid")).unwrap();
        assert_eq!(
            closest.matches(
                Some(timestamp("2025-01-15T10:00:00Z").unwrap()),
                Some(timestamp("2025-01-15T14:00:00Z").unwrap())
            ),
            Some(timestamp("2025-01-15T10:00:00Z").unwrap())
        );
        for invalid in ["2025-01-15", "2025-01-15T10:00:00", "bad"] {
            assert!(selector(invalid, None, None).is_err());
        }
        assert!(selector("2025-01-15T10:00:00Z", Some("latest"), None).is_err());
        assert!(selector("2025-01-15T10:00:00Z", None, Some("last")).is_err());
    }

    #[test]
    fn selection_reprices_one_trade_at_each_market_date_and_keeps_fixings_separate() {
        let mut registry = Registry::default();
        let mut snapshots = Vec::new();
        for (day, rate) in [(15, 0.04), (16, 0.06)] {
            let context = market(&mut registry, &format!("market_{day}"), day, rate);
            snapshots.push(
                capture(
                    &mut registry,
                    &format!("snapshot_{day}"),
                    timestamp(&format!("2025-01-{day}T16:00:00Z")).unwrap(),
                    &context,
                )
                .unwrap(),
            );
        }
        let store = archive(&mut registry, "archive", &snapshots).unwrap();
        let instrument = MakeFixedRateDeposit::<DualFwd>::default()
            .with_identifier("same-deposit".into())
            .with_start_date(Date::new(2025, 1, 17))
            .with_maturity_date(Date::new(2025, 6, 17))
            .with_rate(0.05)
            .with_notional(1000.0)
            .with_currency(Currency::USD)
            .with_discount_index(Some(MarketIndex::SOFR))
            .with_rate_definition(RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Simple,
                Frequency::Annual,
            ))
            .build()
            .unwrap();
        let trade = FixedRateDepositTrade::new(
            instrument,
            Date::new(2025, 1, 15),
            1000.0,
            Side::LongReceive,
        );
        let pricer = DiscountedCashflowPricer::<
            FixedRateDeposit<DualFwd>,
            FixedRateDepositTrade<DualFwd>,
        >::new();
        let mut prices = Vec::new();
        for (day, snapshot) in [(15, &snapshots[0]), (16, &snapshots[1])] {
            let requested = selector(&format!("2025-01-{day}T17:00:00Z"), None, None).unwrap();
            let selection =
                store_selector(&mut registry, &format!("selection_{day}"), requested).unwrap();
            let context = pricing_context(
                &mut registry,
                &format!("historical_{day}"),
                &store,
                &selection,
                None,
            )
            .unwrap();
            let value = registry.pricing_context(&context).unwrap();
            assert_eq!(value.evaluation_date(), Date::new(2025, 1, day));
            assert_eq!(
                value.snapshot_timestamp(),
                Some(registry.market_snapshot(snapshot).unwrap().timestamp())
            );
            prices.push(
                pricer
                    .evaluate(&trade, &[Request::Value], value)
                    .unwrap()
                    .price()
                    .unwrap(),
            );
        }
        assert!((prices[0] - prices[1]).abs() > 0.01);
        // A later request selects the earlier market, without moving its date.
        let selected = store_selector(
            &mut registry,
            "later",
            selector("2025-01-20T12:00:00Z", None, None).unwrap(),
        )
        .unwrap();
        let mut fixings = FixingStore::default();
        fixings.add_fixing(&MarketIndex::SOFR, Date::new(2025, 1, 14), 0.03);
        let fixing_handle = registry
            .upsert_if_changed(
                "fixings",
                QsObject::FixingStore(Box::new(fixings)),
                "history",
            )
            .unwrap();
        let context = pricing_context(
            &mut registry,
            "with_fixings",
            &store,
            &selected,
            Some(&fixing_handle),
        )
        .unwrap();
        let context = registry.pricing_context(&context).unwrap();
        assert_eq!(context.evaluation_date(), Date::new(2025, 1, 16));
        assert_eq!(
            context
                .fixing_store()
                .fixing(&MarketIndex::SOFR, Date::new(2025, 1, 14))
                .unwrap(),
            0.03
        );
    }

    #[test]
    fn full_range_rebuild_updates_same_timestamp_removes_rows_and_rejects_duplicates() {
        let mut registry = Registry::default();
        let context = market(&mut registry, "market", 15, 0.04);
        let instant = timestamp("2025-01-15T16:00:00Z").unwrap();
        let first = capture(&mut registry, "snapshot", instant, &context).unwrap();
        let store = archive(&mut registry, "archive", &[first.clone()]).unwrap();
        let selected = store_selector(
            &mut registry,
            "selector",
            selector(&instant.to_string(), Some("Exact"), None).unwrap(),
        )
        .unwrap();
        let original =
            pricing_context(&mut registry, "historical", &store, &selected, None).unwrap();
        let original_discount = discount(&registry, &original);
        assert_eq!(
            pricing_context(&mut registry, "historical", &store, &selected, None).unwrap(),
            original
        );
        let count = registry.len();
        assert_eq!(
            capture(&mut registry, "snapshot", instant, &context).unwrap(),
            first
        );
        assert_eq!(
            archive(&mut registry, "archive", &[first.clone()]).unwrap(),
            store
        );
        assert_eq!(registry.len(), count);
        assert!(archive(&mut registry, "duplicates", &[first.clone(), first.clone()]).is_err());
        let context = market(&mut registry, "market", 15, 0.08);
        let updated = capture(&mut registry, "snapshot", instant, &context).unwrap();
        assert!(archive(&mut registry, "archive", &[first]).is_err());
        let new_store = archive(&mut registry, "archive", &[updated]).unwrap();
        assert!(registry.market_snapshot_store(&store).is_err());
        let new_context =
            pricing_context(&mut registry, "historical", &new_store, &selected, None).unwrap();
        assert!(discount(&registry, &new_context) < original_discount);
        assert!(registry.pricing_context(&original).is_err());
        let empty = archive(&mut registry, "archive", &[]).unwrap();
        assert!(registry
            .market_snapshot_store(&empty)
            .unwrap()
            .timestamps()
            .is_empty());
        assert!(pricing_context(&mut registry, "historical", &empty, &selected, None).is_err());
    }

    #[test]
    fn native_json_rebuilds_each_quote_side_and_rejects_missing_levels() {
        let mut registry = Registry::default();
        let context = market(&mut registry, "market", 15, 0.04);
        let instant = timestamp("2025-01-15T16:00:00Z").unwrap();
        let snapshot = capture(&mut registry, "snapshot", instant, &context).unwrap();
        let store = archive(&mut registry, "archive", &[snapshot]).unwrap();
        let json = to_json(&registry, &store).unwrap();
        let restored = from_json(&mut registry, "restored", &json).unwrap();
        let mut discounts = Vec::new();
        for level in ["Bid", "Mid", "Ask"] {
            let selected = store_selector(
                &mut registry,
                level,
                selector(&instant.to_string(), Some("Exact"), Some(level)).unwrap(),
            )
            .unwrap();
            let context = pricing_context(
                &mut registry,
                &format!("context_{level}"),
                &restored,
                &selected,
                None,
            )
            .unwrap();
            discounts.push(discount(&registry, &context));
        }
        assert!(discounts.windows(2).all(|pair| pair[0] > pair[1]));
        let mut payload: serde_json::Value = serde_json::from_str(&json).unwrap();
        payload[0]["quotes"]["quotes"]["FixedRateDeposit_USD_SOFR_6M"]["levels"]["bid"] =
            serde_json::Value::Null;
        let mid_only = from_json(&mut registry, "missing_bid", &payload.to_string()).unwrap();
        let selected = store_selector(
            &mut registry,
            "bid",
            selector(&instant.to_string(), None, Some("Bid")).unwrap(),
        )
        .unwrap();
        assert!(
            pricing_context(&mut registry, "failure", &mid_only, &selected, None)
                .unwrap_err()
                .to_lowercase()
                .contains("bid")
        );
        let duplicate = serde_json::json!([payload[0], payload[0]]);
        assert!(from_json(&mut registry, "duplicate_json", &duplicate.to_string()).is_err());
    }

    #[test]
    fn snapshots_roundtrip_explicit_zero_and_composite_inputs_and_reject_runtime_curves() {
        let mut registry = Registry::default();
        let curves: Vec<CurveConfiguration> = serde_json::from_value(serde_json::json!([
            {"market_index":"SOFR","interpolator":"Linear","construction":{"type":"ZeroRate","pillars":["0D","1Y"],"rates":[0.02,0.03],"compounding":"Continuous","frequency":"Annual"}},
            {"market_index":{"Credit":"spread"},"interpolator":"Linear","construction":{"type":"ZeroRate","pillars":["0D","1Y"],"rates":[0.01,0.01],"compounding":"Continuous","frequency":"Annual"}},
            {"market_index":{"Credit":"composite"},"construction":{"type":"Composite","base_index":"SOFR","spread_index":{"Credit":"spread"}}}
        ])).unwrap();
        let mut context = PricingContext::new()
            .with_quote_store(QuoteStore::new(Date::new(2025, 1, 15)))
            .with_curve_configurations(curves);
        context.initialize().unwrap();
        let expected = context
            .constructed_elements()
            .discount_curve(&MarketIndex::Credit("composite".into()))
            .unwrap()
            .curve()
            .discount_factor(Date::new(2025, 6, 15))
            .unwrap()
            .value();
        let elements = context.constructed_elements().clone();
        let context = registry
            .upsert_if_changed(
                "zero_market",
                QsObject::PricingContext(Box::new(context)),
                "zero inputs",
            )
            .unwrap();
        let instant = timestamp("2025-01-16T00:30:00Z").unwrap();
        let snapshot = capture(&mut registry, "snapshot", instant, &context).unwrap();
        let archive_handle = archive(&mut registry, "archive", &[snapshot]).unwrap();
        let json = to_json(&registry, &archive_handle).unwrap();
        let archive_handle = from_json(&mut registry, "restored", &json).unwrap();
        let selected = store_selector(
            &mut registry,
            "selector",
            selector(&instant.to_string(), None, Some("Ask")).unwrap(),
        )
        .unwrap();
        let context = pricing_context(
            &mut registry,
            "historical",
            &archive_handle,
            &selected,
            None,
        )
        .unwrap();
        let value = registry.pricing_context(&context).unwrap();
        assert_eq!(value.evaluation_date(), Date::new(2025, 1, 15));
        let actual = value
            .constructed_elements()
            .discount_curve(&MarketIndex::Credit("composite".into()))
            .unwrap()
            .curve()
            .discount_factor(Date::new(2025, 6, 15))
            .unwrap()
            .value();
        assert!((actual - expected).abs() < 1e-12);
        let runtime = PricingContext::new()
            .with_quote_store(QuoteStore::new(Date::new(2025, 1, 15)))
            .with_constructed_elements(elements);
        let runtime = registry
            .upsert_if_changed(
                "runtime_market",
                QsObject::PricingContext(Box::new(runtime)),
                "runtime",
            )
            .unwrap();
        assert!(capture(&mut registry, "unsupported", instant, &runtime)
            .unwrap_err()
            .contains("construction configurations"));
    }
}

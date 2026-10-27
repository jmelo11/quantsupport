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

/// Captures a committed market's definitions without constructing its dependencies.
pub fn capture_committed(
    registry: &mut Registry,
    name: &str,
    instant: DateTime,
    context: &str,
) -> Result<String, String> {
    let market = super::context::resolve(registry, context)?;
    let snapshot = MarketSnapshot::from_context(instant, &market).map_err(|e| e.to_string())?;
    let definition = key(&(instant, &snapshot))?;
    registry
        .upsert_if_changed(
            name,
            QsObject::MarketSnapshot(Box::new(snapshot)),
            definition,
        )
        .map_err(|e| e.to_string())
}

/// Build complete dated snapshots from aligned context and observation arrays.
/// No intermediate snapshot objects are retained in the worksheet registry.
pub fn from_contexts(
    registry: &mut Registry,
    name: &str,
    contexts: &[String],
    timestamps: &[String],
) -> Result<String, String> {
    if contexts.len() != timestamps.len() {
        return Err("supply one observation timestamp per context".into());
    }
    let instants = timestamps
        .iter()
        .map(|s| timestamp(s))
        .collect::<Result<Vec<_>, _>>()?;
    for handle in contexts {
        registry
            .pricing_context(handle)
            .map_err(|e| e.to_string())?;
    }
    let revisions = contexts
        .iter()
        .map(|h| super::context::revision_key(registry, h))
        .collect::<Result<Vec<_>, _>>()?;
    let definition = key(&(contexts, &instants, revisions))?;
    if let Some(handle) = registry
        .handle_if_unchanged(name, "MarketSnapshotStore", &definition)
        .map_err(|e| e.to_string())?
    {
        return Ok(handle);
    }
    let mut store = MarketSnapshotStore::new();
    for (row, (handle, instant)) in contexts.iter().zip(instants).enumerate() {
        let context = super::context::resolve(registry, handle)?;
        let snapshot = MarketSnapshot::from_context(instant, &context)
            .map_err(|e| format!("snapshot row {}: {e}", row + 1))?;
        store
            .insert(snapshot)
            .map_err(|e| format!("snapshot row {}: {e}", row + 1))?;
    }
    registry
        .upsert_if_changed(
            name,
            QsObject::MarketSnapshotStore(Box::new(store)),
            definition,
        )
        .map_err(|e| e.to_string())
}

/// Capture serializable base inputs; native code rejects supplied runtime curves.
#[cfg(test)]
pub fn capture(
    registry: &mut Registry,
    name: &str,
    instant: DateTime,
    context: &str,
) -> Result<String, String> {
    capture_inputs(registry, name, instant, context, &[])
}

/// Captures the complete declarative market referenced by the input range.
#[cfg(test)]
pub fn capture_inputs(
    registry: &mut Registry,
    name: &str,
    instant: DateTime,
    context: &str,
    inputs: &[String],
) -> Result<String, String> {
    let context_value = super::context::prepare(registry, context, inputs)?;
    let definition = key(&(
        instant,
        context,
        registry
            .context_revision(context)
            .map_err(|e| e.to_string())?,
        inputs,
    ))?;
    if let Some(handle) = registry
        .handle_if_unchanged(name, "MarketSnapshot", &definition)
        .map_err(|error| error.to_string())?
    {
        return Ok(handle);
    }
    let snapshot =
        MarketSnapshot::from_context(instant, &context_value).map_err(|error| error.to_string())?;
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
    pricing_context_with_fixing_policy(registry, name, store, selector, fixings, true)
}

pub fn pricing_context_with_fixing_policy(
    registry: &mut Registry,
    name: &str,
    store: &str,
    selector: &str,
    fixings: Option<&str>,
    anchor_fixings: bool,
) -> Result<String, String> {
    let _archive = registry
        .market_snapshot_store(store)
        .map_err(|error| error.to_string())?;
    let selector_value = registry
        .snapshot_selector(selector)
        .map_err(|error| error.to_string())?;
    let _fixings_value = fixings
        .map(|handle| super::context::fixing_history(registry, handle))
        .transpose()?;
    let definition = key(&(store, selector, fixings, anchor_fixings))?;
    if let Some(handle) = registry
        .handle_if_unchanged(name, "PricingContext", &definition)
        .map_err(|error| error.to_string())?
    {
        return Ok(handle);
    }
    let context = shared_context(registry, store, selector_value, fixings)?;
    let context = if anchor_fixings {
        context
    } else {
        std::rc::Rc::new(context.as_ref().clone().with_anchor_fixings(false))
    };
    registry
        .upsert_if_changed(name, QsObject::PricingContext(context), definition)
        .map_err(|error| error.to_string())
}

/// Reuses calibration by matched snapshot, quote side and fixing revision.
/// Selector names and requested instants do not distinguish identical markets.
pub fn shared_context(
    registry: &Registry,
    store: &str,
    selector: &SnapshotSelector,
    fixings: Option<&str>,
) -> Result<std::rc::Rc<quantsupport::prelude::PricingContext>, String> {
    let archive = registry
        .market_snapshot_store(store)
        .map_err(|e| e.to_string())?;
    let fixings_value = fixings
        .map(|handle| super::context::fixing_history(registry, handle))
        .transpose()?;
    let snapshot = archive.select(selector).map_err(|e| e.to_string())?;
    context_for_snapshot(
        registry,
        store,
        snapshot,
        selector.level(),
        fixings,
        fixings_value,
    )
}

fn context_for_snapshot(
    registry: &Registry,
    store: &str,
    snapshot: &MarketSnapshot,
    level: Level,
    fixings: Option<&str>,
    fixings_value: Option<&quantsupport::prelude::FixingStore>,
) -> Result<std::rc::Rc<quantsupport::prelude::PricingContext>, String> {
    let key = key(&(store, snapshot.timestamp(), level, fixings))?;
    if let Some(context) = registry.context_cache.borrow().historical(&key) {
        return Ok(context);
    }
    let context = snapshot.pricing_context(fixings_value.cloned().unwrap_or_default(), level);
    context.validate().map_err(|e| e.to_string())?;
    let context = std::rc::Rc::new(context);
    registry
        .context_cache
        .borrow_mut()
        .remember_historical(key, &context);
    Ok(context)
}

/// One scalar per instrument at its selected origination market. Each matched
/// market is constructed once per batch; no per-row context or result is stored.
pub fn values(
    registry: &Registry,
    store: &str,
    instruments: &[String],
    timestamps: &[String],
    request: &quantsupport::prelude::Request,
    pricer: &str,
    fixings: Option<&str>,
    rule: Option<&str>,
    level: Option<&str>,
) -> Result<Vec<f64>, String> {
    if instruments.is_empty() || instruments.len() != timestamps.len() {
        return Err("supply one origination timestamp per instrument".into());
    }
    registry.pricer(pricer).map_err(|e| e.to_string())?;
    let archive = registry
        .market_snapshot_store(store)
        .map_err(|e| e.to_string())?;
    let fixings_value = fixings
        .map(|handle| super::context::fixing_history(registry, handle))
        .transpose()?;
    if !matches!(
        request,
        quantsupport::prelude::Request::Value
            | quantsupport::prelude::Request::FairRate(_)
            | quantsupport::prelude::Request::YieldToMaturity
            | quantsupport::prelude::Request::ModifiedDuration
            | quantsupport::prelude::Request::ZSpread
            | quantsupport::prelude::Request::OAS
    ) {
        return Err("historical batches require a scalar request".into());
    }
    for (row, instrument) in instruments.iter().enumerate() {
        registry
            .object(instrument)
            .map_err(|e| format!("historical pricing row {}: {e}", row + 1))?;
    }
    // Group rows before constructing markets. The output keeps worksheet order,
    // while native market residency is bounded by one group at a time.
    let mut groups = std::collections::BTreeMap::new();
    for (row, instant) in timestamps.iter().enumerate() {
        let selection = selector(instant, rule, level)
            .map_err(|e| format!("historical pricing row {}: {e}", row + 1))?;
        let snapshot = archive
            .select(&selection)
            .map_err(|e| format!("historical pricing row {}: {e}", row + 1))?;
        groups
            .entry(snapshot.timestamp())
            .or_insert_with(|| (snapshot, selection.level(), Vec::new()))
            .2
            .push(row);
    }
    let mut output = vec![0.0; instruments.len()];
    for (_, (snapshot, level, rows)) in groups {
        let context =
            context_for_snapshot(registry, store, snapshot, level, fixings, fixings_value)
                .map_err(|e| format!("historical pricing row {}: {e}", rows[0] + 1))?;
        for row in rows {
            let results = super::context::evaluate(
                registry,
                &context,
                &instruments[row],
                request,
                Some(pricer),
            )
            .and_then(|results| super::context::scalar(&results, request))
            .map_err(|e| format!("historical pricing row {}: {e}", row + 1))?;
            output[row] = results;
        }
    }
    Ok(output)
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
                QsObject::PricingContext(std::rc::Rc::new(context)),
                format!("{date}:{rate}"),
            )
            .unwrap()
    }

    fn discount(registry: &Registry, context: &str) -> f64 {
        registry
            .pricing_context(context)
            .unwrap()
            .handle_request(
                &MarketDataRequest::default().with_constructed_elements_request(vec![
                    ConstructedElementRequest::DiscountCurve {
                        market_index: MarketIndex::SOFR,
                    },
                ]),
            )
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
    fn named_historical_contexts_share_native_markets_but_separate_sides_and_fixing_revisions() {
        use std::rc::Rc;
        let mut registry = Registry::default();
        let base = market(&mut registry, "base", 1, 0.04);
        let snapshot = capture(
            &mut registry,
            "snapshot",
            timestamp("2025-01-01T16:00:00Z").unwrap(),
            &base,
        )
        .unwrap();
        let store = archive(&mut registry, "archive", &[snapshot]).unwrap();
        let selector_a = store_selector(
            &mut registry,
            "a",
            selector("2025-01-01T17:00:00Z", None, None).unwrap(),
        )
        .unwrap();
        let selector_b = store_selector(
            &mut registry,
            "b",
            selector("2025-01-01T18:00:00Z", None, None).unwrap(),
        )
        .unwrap();
        let a = pricing_context(&mut registry, "instrument.a", &store, &selector_a, None).unwrap();
        let b = pricing_context(&mut registry, "instrument.b", &store, &selector_b, None).unwrap();
        let context = |handle: &str| match registry.object(handle).unwrap() {
            QsObject::PricingContext(context) => Rc::clone(context),
            _ => panic!("context"),
        };
        assert!(Rc::ptr_eq(&context(&a), &context(&b)));
        let bid = shared_context(
            &registry,
            &store,
            &selector("2025-01-01T17:00:00Z", None, Some("Bid")).unwrap(),
            None,
        )
        .unwrap();
        assert!(!Rc::ptr_eq(&context(&a), &bid));
        assert_eq!(bid.quote_level(), Level::Bid);
        let history = registry
            .upsert_if_changed("fixings", QsObject::FixingStore(Box::default()), "empty")
            .unwrap();
        let selector = selector("2025-01-01T17:00:00Z", None, None).unwrap();
        let first = shared_context(&registry, &store, &selector, Some(&history)).unwrap();
        let changed = registry
            .update_fixing_store_if_changed(&history, "observation", "0.1", |store| {
                store.add_fixing(&MarketIndex::SOFR, Date::new(2024, 12, 31), 0.1)
            })
            .unwrap();
        assert!(shared_context(&registry, &store, &selector, Some(&history)).is_err());
        let second = shared_context(&registry, &store, &selector, Some(&changed)).unwrap();
        assert!(!Rc::ptr_eq(&first, &second));
        assert_eq!(
            second
                .fixing_store()
                .fixing(&MarketIndex::SOFR, Date::new(2024, 12, 31))
                .unwrap(),
            0.1
        );
    }

    #[test]
    fn historical_batch_preserves_row_order_and_matches_individual_native_valuations() {
        use crate::runtime::{
            context, pricing,
            products::{InstrumentObject, NativeInstrument},
        };
        let mut registry = Registry::default();
        let mut snapshots = Vec::new();
        for (day, rate) in [(1, 0.02), (2, 0.08)] {
            let base = market(&mut registry, &format!("market{day}"), day, rate);
            snapshots.push(
                capture(
                    &mut registry,
                    &format!("snapshot{day}"),
                    timestamp(&format!("2025-01-{day:02}T16:00:00Z")).unwrap(),
                    &base,
                )
                .unwrap(),
            );
        }
        let store = archive(&mut registry, "archive", &snapshots).unwrap();
        let policy = pricing::fixed_income_policy(
            &vec![vec![
                super::super::inputs::Cell::Text("USD".into()),
                super::super::inputs::Cell::Text("SOFR".into()),
            ]],
            false,
        )
        .unwrap();
        let pricer =
            pricing::PricerObject::new("DiscountedCashflowPricer", Some(&policy), &vec![]).unwrap();
        let pricer = registry
            .upsert_if_changed("pricer", QsObject::Pricer(Box::new(pricer)), "pricer")
            .unwrap();
        let deposit = MakeFixedRateDeposit::<DualFwd>::default()
            .with_identifier("deposit".into())
            .with_start_date(Date::new(2025, 1, 3))
            .with_maturity_date(Date::new(2025, 6, 15))
            .with_currency(Currency::USD)
            .with_notional(1000.0)
            .with_rate(0.05)
            .with_rate_definition(RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Simple,
                Frequency::Annual,
            ))
            .build()
            .unwrap();
        let instrument = registry
            .upsert_if_changed(
                "deposit",
                QsObject::Instrument(Box::new(InstrumentObject {
                    native: NativeInstrument::FixedRateDeposit(deposit),
                    notional: 1000.0,
                    foreign_notional: None,
                })),
                "deposit",
            )
            .unwrap();
        let instants: Vec<String> = [
            "2025-01-02T17:00:00Z",
            "2025-01-01T18:00:00Z",
            "2025-01-02T19:00:00Z",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let instruments = vec![instrument.clone(); 3];
        let count = registry.len();
        let actual = values(
            &registry,
            &store,
            &instruments,
            &instants,
            &Request::Value,
            &pricer,
            None,
            None,
            None,
        )
        .unwrap();
        for (value, instant) in actual.iter().zip(&instants) {
            let selector = selector(instant, None, None).unwrap();
            let mut native = registry
                .market_snapshot_store(&store)
                .unwrap()
                .pricing_context(&selector, FixingStore::default())
                .unwrap();
            native.initialize().unwrap();
            let expected = context::evaluate(
                &registry,
                &native,
                &instrument,
                &Request::Value,
                Some(&pricer),
            )
            .unwrap()
            .price()
            .unwrap();
            assert!((value - expected).abs() < 1e-10);
        }
        assert_eq!(actual[0], actual[2]);
        assert_ne!(actual[0], actual[1]);
        assert_eq!(
            registry.len(),
            count,
            "batch retains no context or result handles"
        );
        let mut bad = instants.clone();
        bad[1] = "bad".into();
        assert!(values(
            &registry,
            &store,
            &instruments,
            &bad,
            &Request::Value,
            &pricer,
            None,
            None,
            None
        )
        .unwrap_err()
        .contains("row 2"));
        assert!(values(
            &registry,
            &store,
            &instruments,
            &instants[..2],
            &Request::Value,
            &pricer,
            None,
            None,
            None
        )
        .is_err());
    }

    #[test]
    #[ignore = "release benchmark; excludes Excel callbacks and instrument valuation"]
    fn benchmark_fifty_thousand_contexts() {
        let mut registry = Registry::default();
        let base = market(&mut registry, "base", 1, 0.04);
        let snapshot = capture(
            &mut registry,
            "snapshot",
            timestamp("2025-01-01T16:00:00Z").unwrap(),
            &base,
        )
        .unwrap();
        let store = archive(&mut registry, "archive", &[snapshot]).unwrap();
        let selection = selector("2025-01-01T17:00:00Z", None, None).unwrap();
        let began = std::time::Instant::now();
        for _ in 0..50_000 {
            let mut context = registry
                .market_snapshot_store(&store)
                .unwrap()
                .pricing_context(&selection, FixingStore::default())
                .unwrap();
            context.initialize().unwrap();
            std::hint::black_box(context);
        }
        let rebuilding = began.elapsed();
        let selector = store_selector(&mut registry, "selector", selection).unwrap();
        let began = std::time::Instant::now();
        for row in 0..50_000 {
            std::hint::black_box(
                pricing_context(
                    &mut registry,
                    &format!("instrument.{row}"),
                    &store,
                    &selector,
                    None,
                )
                .unwrap(),
            );
        }
        println!("50,000 contexts, one matched deposit market: rebuilding {rebuilding:?}; shared named contexts {:?}", began.elapsed());
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
        let lazy = pricing_context(&mut registry, "failure", &mid_only, &selected, None).unwrap();
        let error = super::super::context::elements(
            &registry,
            &lazy,
            ConstructedElementRequest::DiscountCurve {
                market_index: MarketIndex::SOFR,
            },
        )
        .err()
        .unwrap();
        assert!(error.to_lowercase().contains("bid"));
        let duplicate = serde_json::json!([payload[0], payload[0]]);
        assert!(from_json(&mut registry, "duplicate_json", &duplicate.to_string()).is_err());
    }

    #[test]
    fn snapshots_roundtrip_explicit_zero_and_composite_inputs_and_reject_runtime_curves() {
        let mut registry = Registry::default();
        let mut quotes = QuoteStore::new(Date::new(2025, 1, 15));
        let mut curves = Vec::new();
        for (index, label, rates) in [
            (MarketIndex::SOFR, "SOFR", [0.02, 0.03]),
            (
                MarketIndex::Credit("spread".into()),
                "Credit(spread)",
                [0.01, 0.01],
            ),
        ] {
            let mut identifiers = Vec::new();
            for (tenor, rate) in ["0D", "1Y"].iter().zip(rates) {
                let identifier = format!("ZeroRate_USD_{label}_{tenor}");
                quotes.add_quote(Quote::new(
                    identifier.parse().unwrap(),
                    QuoteLevels::new(Some(rate), Some(rate), Some(rate)),
                ));
                identifiers.push(identifier);
            }
            curves.push(CurveConfiguration::new(
                index,
                DayCounter::Actual360,
                Interpolator::Linear,
                true,
                identifiers,
            ));
        }
        curves.push(CurveConfiguration::composite(
            MarketIndex::Credit("composite".into()),
            MarketIndex::SOFR,
            MarketIndex::Credit("spread".into()),
        ));
        let mut context = PricingContext::new()
            .with_quote_store(quotes)
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
                QsObject::PricingContext(std::rc::Rc::new(context)),
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
            .handle_request(
                &MarketDataRequest::default().with_constructed_elements_request(vec![
                    ConstructedElementRequest::DiscountCurve {
                        market_index: MarketIndex::Credit("composite".into()),
                    },
                ]),
            )
            .unwrap()
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
                QsObject::PricingContext(std::rc::Rc::new(runtime)),
                "runtime",
            )
            .unwrap();
        assert!(capture(&mut registry, "unsupported", instant, &runtime)
            .unwrap_err()
            .contains("construction configurations"));
    }
}

#[cfg(test)]
mod context_array_tests {
    use super::*;
    use quantsupport::prelude::*;
    #[test]
    fn context_arrays_rebuild_after_removal_and_reject_duplicate_observation_instants() {
        let mut registry = Registry::default();
        let contexts = (15..=16)
            .map(|day| {
                crate::runtime::context::create(
                    &mut registry,
                    &format!("day{day}"),
                    Date::new(2026, 1, day),
                    Currency::USD,
                    MarketIndex::SOFR,
                    Level::Mid,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let times = vec!["2026-01-15T16:00:00Z".into(), "2026-01-16T16:00:00Z".into()];
        let first = from_contexts(&mut registry, "history", &contexts, &times).unwrap();
        assert_eq!(
            from_contexts(&mut registry, "history", &contexts, &times).unwrap(),
            first
        );
        assert_eq!(
            registry
                .market_snapshot_store(&first)
                .unwrap()
                .timestamps()
                .len(),
            2
        );
        let duplicate = vec![times[0].clone(), "2026-01-15T17:00:00+01:00".into()];
        assert!(from_contexts(&mut registry, "bad", &contexts, &duplicate).is_err());
        assert!(from_contexts(&mut registry, "bad", &contexts, &times[..1]).is_err());
        let next = from_contexts(&mut registry, "history", &contexts[..1], &times[..1]).unwrap();
        assert_ne!(first, next);
        assert_eq!(
            registry
                .market_snapshot_store(&next)
                .unwrap()
                .timestamps()
                .len(),
            1
        );
        let empty = from_contexts(&mut registry, "history", &[], &[]).unwrap();
        assert!(registry
            .market_snapshot_store(&empty)
            .unwrap()
            .timestamps()
            .is_empty());
    }
}

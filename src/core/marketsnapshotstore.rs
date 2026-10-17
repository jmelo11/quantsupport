//! Historical market snapshots indexed by UTC observation instant.

use std::collections::{btree_map::Entry, BTreeMap};

use serde::{Deserialize, Serialize};

use crate::{
    core::{
        marketsnapshot::MarketSnapshot, pricingcontext::PricingContext,
        snapshotselector::SnapshotSelector,
    },
    quotes::fixingstore::FixingStore,
    time::datetime::DateTime,
    utils::errors::{QSError, Result},
};

/// An archive of coherent markets, serialized as a timestamp-ordered JSON array.
///
/// Each selection resolves one complete snapshot, never a mixture of observation
/// dates across curves, FX, and volatility inputs.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(try_from = "Vec<MarketSnapshot>", into = "Vec<MarketSnapshot>")]
pub struct MarketSnapshotStore {
    snapshots: BTreeMap<DateTime, MarketSnapshot>,
}

impl MarketSnapshotStore {
    /// Creates an empty archive.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a snapshot without overwriting an existing observation instant.
    ///
    /// # Errors
    /// Returns an error for duplicate timestamps, including equivalent offsets.
    pub fn insert(&mut self, snapshot: MarketSnapshot) -> Result<()> {
        match self.snapshots.entry(snapshot.timestamp()) {
            Entry::Vacant(entry) => {
                entry.insert(snapshot);
                Ok(())
            }
            Entry::Occupied(entry) => Err(QSError::InvalidValueErr(format!(
                "Market snapshot already exists at {}",
                entry.key()
            ))),
        }
    }

    /// Returns all available observation instants in ascending order.
    #[must_use]
    pub fn timestamps(&self) -> Vec<DateTime> {
        self.snapshots.keys().copied().collect()
    }

    /// Resolves a selector to a complete snapshot, exposing the actual timestamp.
    ///
    /// # Errors
    /// Returns an error when no snapshot satisfies the selection rule.
    pub fn select(&self, selector: &SnapshotSelector) -> Result<&MarketSnapshot> {
        let timestamp = selector.timestamp();
        let earlier = self
            .snapshots
            .range(..=timestamp)
            .next_back()
            .map(|(key, _)| *key);
        let later = self
            .snapshots
            .range(timestamp..)
            .next()
            .map(|(key, _)| *key);
        selector
            .matches(earlier, later)
            .and_then(|selected| self.snapshots.get(&selected))
            .ok_or_else(|| {
                QSError::NotFoundErr(format!(
                    "No market snapshot for {timestamp} using {:?}",
                    selector.rule(),
                ))
            })
    }

    /// Selects historical inputs and creates a fresh context at the chosen side.
    ///
    /// Construction remains lazy unless the caller invokes `initialize`.
    /// The context reports the selected timestamp and market business date.
    ///
    /// # Errors
    /// Returns an error when no snapshot satisfies the selection rule.
    pub fn pricing_context(
        &self,
        selector: &SnapshotSelector,
        fixings: FixingStore,
    ) -> Result<PricingContext> {
        Ok(self
            .select(selector)?
            .pricing_context(fixings, selector.level()))
    }
}

impl TryFrom<Vec<MarketSnapshot>> for MarketSnapshotStore {
    type Error = QSError;

    fn try_from(snapshots: Vec<MarketSnapshot>) -> Result<Self> {
        let mut store = Self::new();
        for snapshot in snapshots {
            store.insert(snapshot)?;
        }
        Ok(store)
    }
}

impl From<MarketSnapshotStore> for Vec<MarketSnapshot> {
    fn from(store: MarketSnapshotStore) -> Self {
        store.snapshots.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core::snapshotselector::SnapshotSelectionRule, prelude::*};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    fn instant(day: u32, hour: u32) -> DateTime {
        chrono::NaiveDate::from_ymd_opt(2025, 1, day)
            .and_then(|date| date.and_hms_opt(hour, 0, 0))
            .unwrap_or_else(|| panic!("valid test instant"))
            .and_utc()
            .into()
    }

    fn snapshot(
        day: u32,
        hour: u32,
        rate: f64,
    ) -> std::result::Result<MarketSnapshot, Box<dyn std::error::Error>> {
        let mut quotes = QuoteStore::new(Date::new(2025, 1, day));
        quotes.add_quote(Quote::new(
            "FixedRateDeposit_USD_SOFR_6M".parse()?,
            QuoteLevels::new(Some(rate), Some(rate - 0.01), Some(rate + 0.01)),
        ));
        let context = PricingContext::new()
            .with_quote_store(quotes)
            .with_fx_store(FxStore::from_records([FxRateRecord {
                base: Currency::EUR,
                quote: Currency::USD,
                rate: 1.1,
            }]))
            .with_curve_configurations(vec![serde_json::from_value(serde_json::json!({
                "market_index": "SOFR", "quotes": ["FixedRateDeposit_USD_SOFR_6M"]
            }))?]);
        Ok(MarketSnapshot::from_context(instant(day, hour), &context)?)
    }

    #[test]
    fn archive_roundtrip_selection_and_curve_sides() -> TestResult {
        let mut store = MarketSnapshotStore::new();
        store.insert(snapshot(15, 10, 0.04)?)?;
        store.insert(snapshot(15, 14, 0.06)?)?;
        store.insert(snapshot(16, 10, 0.08)?)?;
        assert!(store.insert(snapshot(15, 10, 0.99)?).is_err());
        let encoded = serde_json::to_string(&store)?;
        let store: MarketSnapshotStore = serde_json::from_str(&encoded)?;
        assert_eq!(
            store.timestamps(),
            vec![instant(15, 10), instant(15, 14), instant(16, 10)]
        );
        let selector = SnapshotSelector::new(instant(15, 13));
        assert_eq!(store.select(&selector)?.timestamp(), instant(15, 10));
        assert_eq!(
            store
                .select(&selector.with_rule(SnapshotSelectionRule::Closest))?
                .timestamp(),
            instant(15, 14)
        );
        assert!(store
            .select(&selector.with_rule(SnapshotSelectionRule::Exact))
            .is_err());
        assert!(store
            .select(&SnapshotSelector::new(instant(14, 10)))
            .is_err());
        let mut discounts = Vec::new();
        for level in [Level::Bid, Level::Mid, Level::Ask] {
            let mut context =
                store.pricing_context(&selector.with_level(level), FixingStore::default())?;
            assert_eq!(context.snapshot_timestamp(), Some(instant(15, 10)));
            assert_eq!(context.evaluation_date(), Date::new(2025, 1, 15));
            assert!(
                (context
                    .fx_store()
                    .get_fx_rate(Currency::EUR, Currency::USD)?
                    .value()
                    - 1.1)
                    .abs()
                    < 1e-12
            );
            context.initialize()?;
            discounts.push(
                context
                    .constructed_elements()
                    .discount_curve(&MarketIndex::SOFR)
                    .ok_or("missing curve")?
                    .curve()
                    .discount_factor(Date::new(2025, 7, 15))?
                    .value(),
            );
        }
        assert!(discounts.windows(2).all(|pair| pair[0] > pair[1]));
        let next_day = store.pricing_context(
            &SnapshotSelector::new(instant(16, 10)),
            FixingStore::default(),
        )?;
        assert_eq!(next_day.evaluation_date(), Date::new(2025, 1, 16));
        assert!(next_day.constructed_elements().is_empty());
        let changed = next_day.with_quote_level(Level::Ask);
        assert_eq!(changed.snapshot_timestamp(), None);
        Ok(())
    }

    #[test]
    fn empty_and_duplicate_serialized_archives_fail() -> TestResult {
        assert!(MarketSnapshotStore::new()
            .select(&SnapshotSelector::new(instant(15, 10)))
            .is_err());
        let snapshot = snapshot(15, 10, 0.04)?;
        let duplicate = serde_json::to_string(&vec![snapshot.clone(), snapshot])?;
        assert!(serde_json::from_str::<MarketSnapshotStore>(&duplicate).is_err());
        Ok(())
    }
}

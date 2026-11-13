use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::{Interpolate, Interpolator},
    time::{date::Date, enums::TimeUnit, period::Period},
    utils::errors::{QSError, Result},
};

type ObservationMap = HashMap<MarketIndex, BTreeMap<Date, f64>>;

/// Fixing store for market indices, providing access to historical fixings and the ability to fill missing
/// fixings using interpolation.
///
/// Clones share immutable history; updates copy shared data before changing it.
#[derive(Serialize, Debug, Default, Deserialize, Clone)]
pub struct FixingStore {
    #[serde(with = "ObservationMapSerde")]
    values: Arc<ObservationMap>,
}

impl FixingStore {
    /// Iterates over stored observations without copying the shared history.
    pub fn observations(&self) -> impl Iterator<Item = (&MarketIndex, Date, f64)> {
        self.values.iter().flat_map(|(index, values)| {
            values
                .iter()
                .map(move |(date, value)| (index, *date, *value))
        })
    }

    /// Returns the stored fixing for a date, in the index's native quote units.
    ///
    /// ## Errors
    /// Returns [`QSError`] if the fixing is unavailable for the requested date.
    pub fn fixing(&self, market_index: &MarketIndex, date: Date) -> Result<f64> {
        if let MarketIndex::FxPair(pair) = market_index {
            if self
                .values
                .get(market_index)
                .and_then(|values| values.get(&date))
                .is_none()
            {
                let records = self
                    .values
                    .iter()
                    .filter_map(|(index, values)| match index {
                        MarketIndex::FxPair(observed) => values.get(&date).map(|value| {
                            crate::quotes::fxstore::FxRateRecord::new(
                                observed.base(),
                                observed.quote(),
                                *value,
                            )
                        }),
                        _ => None,
                    });
                return crate::quotes::fxstore::FxStore::from_records(records)
                    .get_fx_rate(pair.base(), pair.quote())
                    .map(|value| value.value());
            }
        }
        let fixing = self.fixings(market_index)?.get(&date).ok_or_else(|| {
            QSError::NotFoundErr(format!(
                "Fixings of index {market_index} for date {date} not found in fixings data."
            ))
        })?;
        Ok(*fixing)
    }
    /// Returns a reference to the map of all fixings.
    ///
    /// ## Errors
    /// Returns [`QSError`] if the [`MarketIndex`] is not found.
    pub fn fixings(&self, market_index: &MarketIndex) -> Result<&BTreeMap<Date, f64>> {
        self.values.get(market_index).ok_or_else(|| {
            QSError::NotFoundErr(format!("Index {market_index} not found in fixings data."))
        })
    }
    /// Adds a fixing for a given date and rate.
    pub fn add_fixing(&mut self, market_index: &MarketIndex, date: Date, value: f64) {
        // ensure an entry exists and insert the fixing
        Arc::make_mut(&mut self.values)
            .entry(market_index.clone())
            .or_default()
            .insert(date, value);
    }

    /// Removes one stored fixing and drops an empty index bucket.
    pub fn remove_fixing(&mut self, market_index: &MarketIndex, date: Date) -> Option<f64> {
        // A missing observation must not copy an otherwise shared history.
        self.values.get(market_index)?.get(&date)?;
        let values = Arc::make_mut(&mut self.values);
        let fixings = values.get_mut(market_index)?;
        let previous = fixings.remove(&date);
        if fixings.is_empty() {
            values.remove(market_index);
        }
        previous
    }

    /// Fills missing calendar dates between the first and last stored fixings.
    ///
    /// Existing values are preserved. The current implementation queries at
    /// `last_date - date` against knots measured from the first date: inserted
    /// values therefore use a reversed time coordinate, not conventional
    /// chronological interpolation. No business-day filtering is applied.
    ///
    /// ## Errors
    /// Returns an error if interpolation fails during the filling process.
    #[allow(clippy::cast_precision_loss)]
    pub fn fill_missing_fixings(&mut self, interpolator: Interpolator) -> Result<()> {
        Arc::make_mut(&mut self.values)
            .values_mut()
            .filter(|fixings| !fixings.is_empty())
            .try_for_each(|fixings| {
                // get start and end
                let mut curr_date: Date = fixings.keys().min().copied().ok_or_else(|| {
                    QSError::UnexpectedErr(
                    "An error was found while getting the minimun date for filling missing indices."
                        .into(),
                )
                })?;
                let last_date: Date = fixings.keys().max().copied().ok_or_else(|| {
                    QSError::UnexpectedErr(
                    "An error was found while getting the maximun date for filling missing indices."
                        .into(),
                )
                })?;

                // transform to floats to interpolate
                let times: Vec<f64> = fixings
                    .keys()
                    .map(|date| (*date - curr_date) as f64)
                    .collect();
                let values: Vec<f64> = fixings.values().copied().collect();

                while curr_date < last_date {
                    let time = (last_date - curr_date) as f64;
                    let interp = interpolator.interpolate(time, &times, &values, false)?;
                    fixings.entry(curr_date).or_insert(interp);
                    curr_date = curr_date + Period::new(1, TimeUnit::Days);
                }
                Ok(())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currencies::currency::Currency;

    #[test]
    fn clones_share_history_until_an_observation_changes() {
        let date = Date::new(2026, 1, 15);
        let mut original = FixingStore::default();
        original.add_fixing(&MarketIndex::SOFR, date, 0.04);
        let mut updated = original.clone();
        assert!(Arc::ptr_eq(&original.values, &updated.values));
        assert_eq!(updated.remove_fixing(&MarketIndex::SOFR, date + 1), None);
        assert!(Arc::ptr_eq(&original.values, &updated.values));
        updated.add_fixing(&MarketIndex::SOFR, date, 0.05);
        assert!(!Arc::ptr_eq(&original.values, &updated.values));
        assert_eq!(original.fixing(&MarketIndex::SOFR, date).unwrap(), 0.04);
        assert_eq!(updated.fixing(&MarketIndex::SOFR, date).unwrap(), 0.05);
        let mut removed = original.clone();
        assert_eq!(removed.remove_fixing(&MarketIndex::SOFR, date), Some(0.04));
        assert!(removed.fixings(&MarketIndex::SOFR).is_err());
        assert_eq!(original.fixing(&MarketIndex::SOFR, date).unwrap(), 0.04);
    }

    #[test]
    fn named_observation_keys_round_trip_with_explicit_types() {
        let date = Date::new(2026, 1, 15);
        let mut original = FixingStore::default();
        let indices = [
            MarketIndex::Equity("SPX".into()),
            MarketIndex::Credit("AcmeCorp".into()),
            MarketIndex::Other("SOFR".into()),
            MarketIndex::Other("MyCurve".into()),
            MarketIndex::Collateral(Currency::EUR, Currency::USD),
        ];
        for index in &indices {
            original.add_fixing(index, date, 0.04);
        }
        let value = serde_json::to_value(&original).unwrap();
        assert!(value["values"].get("OTHER:SOFR").is_some());
        let restored: FixingStore = serde_json::from_value(value).unwrap();
        for index in indices {
            assert_eq!(restored.fixing(&index, date).unwrap(), 0.04);
        }
    }

    #[test]
    fn observation_keys_reject_unstructured_or_malformed_indices() {
        for index in ["MyCurve", "FX:BadPair", "CREDIT:", "OTHER:"] {
            let value = serde_json::json!({"values": {index: {"2026-01-15": 0.04}}});
            assert!(
                serde_json::from_value::<FixingStore>(value).is_err(),
                "{index}"
            );
        }
    }

    #[test]
    fn shared_history_preserves_json_and_interpolation_isolation() {
        let json = r#"{"values":{"SOFR":{"2026-01-15":0.04,"2026-01-17":0.06}}}"#;
        let original: FixingStore = serde_json::from_str(json).unwrap();
        assert_eq!(
            serde_json::to_value(&original).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
        let mut filled = original.clone();
        filled.fill_missing_fixings(Interpolator::Linear).unwrap();
        let middle = Date::new(2026, 1, 16);
        assert!((filled.fixing(&MarketIndex::SOFR, middle).unwrap() - 0.05).abs() < 1e-12);
        assert!(original.fixing(&MarketIndex::SOFR, middle).is_err());
    }
}

// JSON object keys cannot directly serialize parameterized enum variants.
// Fieldless index keys retain their existing schema; tagged keys round-trip
// equity, FX, collateral, credit and explicitly named custom indices.
struct ObservationMapSerde;

impl ObservationMapSerde {
    fn serialize<S: Serializer>(
        values: &Arc<ObservationMap>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(values.len()))?;
        for (index, observations) in values.iter() {
            map.serialize_entry(&index.to_string(), observations)?;
        }
        map.end()
    }

    fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Arc<ObservationMap>, D::Error> {
        let values = HashMap::<String, BTreeMap<Date, f64>>::deserialize(deserializer)?;
        let mut result = ObservationMap::new();
        for (key, observations) in values {
            let index = key
                .parse::<MarketIndex>()
                .map_err(serde::de::Error::custom)?;
            if result.insert(index, observations).is_some() {
                return Err(serde::de::Error::custom(format!(
                    "Duplicate fixing index {key}"
                )));
            }
        }
        Ok(Arc::new(result))
    }
}

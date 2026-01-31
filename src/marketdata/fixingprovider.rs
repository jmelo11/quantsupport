use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::{
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::Interpolator,
    prelude::AtlasError,
    time::{date::Date, enums::TimeUnit, period::Period},
    utils::errors::Result,
};

/// # `FixingProvider`
#[derive(Serialize)]
pub struct FixingProvider {
    values: HashMap<MarketIndex, HashMap<Date, f64>>,
}

impl FixingProvider {
    /// Returns the fixing rate for a given date.
    ///
    /// # Errors
    /// Returns an error if the fixing is unavailable for the requested date.
    pub fn fixing(&self, market_index: &MarketIndex, date: Date) -> Result<f64> {
        let fixing = self
            .fixings(market_index)?
            .get(&date)
            .ok_or(AtlasError::NotFoundErr(
                format!(
                    "Fixings of index {market_index} for date {date} not found in fixings data."
                )
                .into(),
            ))?;
        Ok(*fixing)
    }
    /// Returns a reference to the map of all fixings.
    pub fn fixings(&self, market_index: &MarketIndex) -> Result<&HashMap<Date, f64>> {
        self.values.get(market_index).ok_or(AtlasError::NotFoundErr(
            format!("Index {market_index} not found in fixings data.").into(),
        ))
    }
    /// Adds a fixing for a given date and rate.
    pub fn add_fixing(&mut self, market_index: &MarketIndex, date: Date, value: f64) {
        // ensure an entry exists and insert the fixing
        self.values
            .entry(market_index.clone())
            .or_insert_with(HashMap::new)
            .insert(date, value);
    }

    /// Fill missing fixings using interpolation.
    pub fn fill_missing_fixings(&mut self, interpolator: Interpolator) -> Result<()> {
        if self.values.is_empty() {
            return Ok(());
        }

        // collect keys to avoid borrowing self while mutating it
        let index_keys: Vec<MarketIndex> = self.values.keys().cloned().collect();

        for index_key in index_keys {
            let aux_btreemap: BTreeMap<Date, f64> = match self.values.get(&index_key) {
                Some(map) => map.iter().map(|(k, v)| (*k, *v)).collect(),
                None => continue,
            };

            if aux_btreemap.is_empty() {
                continue;
            }

            let first_date = *aux_btreemap.keys().min().unwrap();
            let last_date = *aux_btreemap.keys().max().unwrap();

            let mut x: Vec<f64> = Vec::with_capacity(aux_btreemap.len());
            for &d in aux_btreemap.keys() {
                let days = match i32::try_from(d - first_date) {
                    Ok(v) => v,
                    Err(_) => {
                        return Err(AtlasError::InvalidValueErr(
                            "Fixing day count does not fit in i32".into(),
                        ))
                    }
                };
                x.push(f64::from(days));
            }

            let y = aux_btreemap.values().copied().collect::<Vec<f64>>();

            let mut current_date = first_date;

            while current_date <= last_date {
                let exists = self
                    .values
                    .get(&index_key)
                    .map(|m| m.contains_key(&current_date))
                    .unwrap_or(false);

                if !exists {
                    let days = match i32::try_from(current_date - first_date) {
                        Ok(v) => v,
                        Err(_) => {
                            return Err(AtlasError::InvalidValueErr(
                                "Fixing day count does not fit in i32".into(),
                            ))
                        }
                    };
                    let days = f64::from(days);
                    let rate = interpolator.interpolate(days, &x, &y, false)?;
                    self.add_fixing(&index_key, current_date, rate);
                }
                current_date = current_date + Period::new(1, TimeUnit::Days);
            }
        }

        Ok(())
    }
}

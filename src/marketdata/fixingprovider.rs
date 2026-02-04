use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::{
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::{Interpolate, Interpolator},
    time::{date::Date, enums::TimeUnit, period::Period},
    utils::errors::{AtlasError, Result},
};

/// # `FixingProvider`
#[derive(Serialize)]
pub struct FixingProvider {
    values: HashMap<MarketIndex, BTreeMap<Date, f64>>,
}

impl FixingProvider {
    /// Returns the fixing rate for a given date.
    ///
    /// ## Errors
    /// Returns an error if the fixing is unavailable for the requested date.
    pub fn fixing(&self, market_index: &MarketIndex, date: Date) -> Result<f64> {
        let fixing = self
            .fixings(market_index)?
            .get(&date)
            .ok_or(AtlasError::NotFoundErr(format!(
                "Fixings of index {market_index} for date {date} not found in fixings data."
            )))?;
        Ok(*fixing)
    }
    /// Returns a reference to the map of all fixings.
    ///
    /// ## Errors
    /// Returns [`AtlasError`] if the [`MarketIndex`] is not found.
    pub fn fixings(&self, market_index: &MarketIndex) -> Result<&BTreeMap<Date, f64>> {
        self.values
            .get(market_index)
            .ok_or(AtlasError::NotFoundErr(format!(
                "Index {market_index} not found in fixings data."
            )))
    }
    /// Adds a fixing for a given date and rate.
    pub fn add_fixing(&mut self, market_index: &MarketIndex, date: Date, value: f64) {
        // ensure an entry exists and insert the fixing
        self.values
            .entry(market_index.clone())
            .or_default()
            .insert(date, value);
    }

    /// Fill missing fixings using interpolation.
    ///
    /// ## Errors
    /// Returns an error if interpolation fails during the filling process.
    #[allow(clippy::cast_precision_loss)]
    pub fn fill_missing_fixings(&mut self, interpolator: Interpolator) -> Result<()> {
        self.values
            .values_mut()
            .filter(|fixings| !fixings.is_empty())
            .try_for_each(|fixings| {
                // get start and end
                let mut curr_date: Date = fixings
                    .keys()
                    .min()
                    .copied()
                    .ok_or(AtlasError::UnexpectedErr(
                    "An error was found while getting the minimun date for filling missing indices."
                        .into(),
                ))?;
                let last_date: Date = fixings
                    .keys()
                    .max()
                    .copied()
                    .ok_or(AtlasError::UnexpectedErr(
                    "An error was found while getting the maximun date for filling missing indices."
                        .into(),
                ))?;

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

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::{
    indices::marketindex::MarketIndex,
    time::{date::Date, period::Period},
    utils::errors::{AtlasError, Result},
};

/// Float key wrapper for deterministic ordering of `f64` values.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct FloatKey(f64);

impl FloatKey {
    /// Creates a new ordered float key.
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self(value)
    }

    /// Returns the underlying float.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

impl Eq for FloatKey {}

impl Ord for FloatKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .total_cmp(&other.0)
            .then_with(|| self.0.to_bits().cmp(&other.0.to_bits()))
    }
}

/// Volatility surface keyed by maturity date and strike.
///
/// Values are stored in a nested map so the surface can be populated from
/// serialized market data in a deterministic order.
#[derive(Clone, Debug, Default)]
pub struct VolatilitySurface {
    instrument: MarketIndex,
    points: BTreeMap<Date, BTreeMap<FloatKey, f64>>,
}

impl VolatilitySurface {
    /// Creates an empty volatility surface for the given instrument.
    #[must_use]
    pub fn new(instrument: MarketIndex) -> Self {
        Self {
            instrument,
            points: BTreeMap::new(),
        }
    }

    /// Returns the instrument identifier for this surface.
    #[must_use]
    pub fn instrument(&self) -> &MarketIndex {
        &self.instrument
    }

    /// Inserts a volatility point indexed by maturity and strike.
    pub fn insert_point(&mut self, maturity: Date, strike: f64, volatility: f64) {
        self.points
            .entry(maturity)
            .or_insert_with(BTreeMap::new)
            .insert(FloatKey::new(strike), volatility);
    }

    /// Returns the volatility for the given maturity and strike.
    ///
    /// # Errors
    /// Returns an error if the point is not found.
    pub fn volatility(&self, maturity: Date, strike: f64) -> Result<f64> {
        let strike_map = self.points.get(&maturity).ok_or_else(|| {
            AtlasError::NotFoundErr(format!(
                "Volatility surface {instrument} missing maturity {maturity}",
                instrument = self.instrument
            ))
        })?;
        strike_map
            .get(&FloatKey::new(strike))
            .copied()
            .ok_or_else(|| {
            AtlasError::NotFoundErr(format!(
                "Volatility surface {instrument} missing strike {strike} for maturity {maturity}",
                instrument = self.instrument
            ))
        })
    }

    /// Returns all stored points in the surface.
    #[must_use]
    pub const fn points(&self) -> &BTreeMap<Date, BTreeMap<FloatKey, f64>> {
        &self.points
    }
}

/// Volatility cube keyed by maturity, tenor, and strike.
///
/// The tenor dimension uses the [`Period`] type so common tenors such as `1Y`
/// or `6M` can be stored consistently.
#[derive(Clone, Debug, Default)]
pub struct VolatilityCube {
    instrument: MarketIndex,
    points: BTreeMap<Date, BTreeMap<Period, BTreeMap<FloatKey, f64>>>,
}

impl VolatilityCube {
    /// Creates an empty volatility cube for the given instrument.
    #[must_use]
    pub fn new(instrument: MarketIndex) -> Self {
        Self {
            instrument,
            points: BTreeMap::new(),
        }
    }

    /// Returns the instrument identifier for this cube.
    #[must_use]
    pub fn instrument(&self) -> &MarketIndex {
        &self.instrument
    }

    /// Inserts a volatility point indexed by maturity, tenor, and strike.
    pub fn insert_point(&mut self, maturity: Date, tenor: Period, strike: f64, vol: f64) {
        self.points
            .entry(maturity)
            .or_insert_with(BTreeMap::new)
            .entry(tenor)
            .or_insert_with(BTreeMap::new)
            .insert(FloatKey::new(strike), vol);
    }

    /// Returns the volatility for the given maturity, tenor, and strike.
    ///
    /// # Errors
    /// Returns an error if the point is not found.
    pub fn volatility(&self, maturity: Date, tenor: Period, strike: f64) -> Result<f64> {
        let tenor_map = self.points.get(&maturity).ok_or_else(|| {
            AtlasError::NotFoundErr(format!(
                "Volatility cube {instrument} missing maturity {maturity}",
                instrument = self.instrument
            ))
        })?;
        let strike_map = tenor_map.get(&tenor).ok_or_else(|| {
            AtlasError::NotFoundErr(format!(
                "Volatility cube {instrument} missing tenor {tenor:?} for maturity {maturity}",
                instrument = self.instrument
            ))
        })?;
        strike_map
            .get(&FloatKey::new(strike))
            .copied()
            .ok_or_else(|| {
            AtlasError::NotFoundErr(format!(
                "Volatility cube {instrument} missing strike {strike} for maturity {maturity} tenor {tenor:?}",
                instrument = self.instrument
            ))
        })
    }

    /// Returns all stored points in the cube.
    #[must_use]
    pub const fn points(&self) -> &BTreeMap<Date, BTreeMap<Period, BTreeMap<FloatKey, f64>>> {
        &self.points
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_returns_inserted_point() {
        let mut surface = VolatilitySurface::new(MarketIndex::Other("SPX".to_string()));
        let maturity = Date::new(2026, 1, 1);
        surface.insert_point(maturity, 4500.0, 0.22);

        assert_eq!(surface.volatility(maturity, 4500.0).unwrap(), 0.22);
    }

    #[test]
    fn cube_returns_inserted_point() {
        let mut cube = VolatilityCube::new(MarketIndex::Other("SPX".to_string()));
        let maturity = Date::new(2026, 1, 1);
        let tenor = Period::new(1, crate::time::enums::TimeUnit::Years);
        cube.insert_point(maturity, tenor, 4500.0, 0.18);

        assert_eq!(cube.volatility(maturity, tenor, 4500.0).unwrap(), 0.18);
    }
}

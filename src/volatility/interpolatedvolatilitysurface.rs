use crate::{
    ad::adreal::{ADReal, IsReal},
    core::pillars::Pillars,
    indices::marketindex::MarketIndex,
    math::interpolation::bilinear::{BilinearInterpolator, BilinearPoint, BilinearValue},
    time::{date::Date, period::Period},
    utils::errors::{AtlasError, Result},
    volatility::volatilitysurface::VolatilitySurface,
    volatility::volatilityindexing::F64Key,
};
use std::collections::BTreeMap;

type SurfaceMap<T> = BTreeMap<Period, BTreeMap<F64Key, T>>;

/// # `InterpolatedVolatilitySurface`
///
/// Represents if the volatility surface.
///
/// ## Generics
/// - `T`: Numeric type for the volatility values (e.g., `f64`, `ADReal`).
pub struct InterpolatedVolatilitySurface<T: IsReal> {
    reference_date: Date,
    market_index: MarketIndex,
    points: SurfaceMap<T>,
    labels: Option<Vec<String>>,
}

impl<T: IsReal> InterpolatedVolatilitySurface<T> {
    /// Creates a new `VolatilitySurface`.
    #[must_use]
    pub const fn new(
        reference_date: Date,
        market_index: MarketIndex,
        points: SurfaceMap<T>,
    ) -> Self {
        Self {
            reference_date,
            market_index,
            points,
            labels: None,
        }
    }

    /// Attaches labels to each volatility pillar used in sensitivity reports.
    #[must_use]
    pub fn with_labels(mut self, labels: &[String]) -> Self {
        self.labels = Some(labels.to_owned());
        self
    }

    /// Returns the market index associated with the volatility surface.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }
}

impl<T: BilinearValue> VolatilitySurface<T> for InterpolatedVolatilitySurface<T> {
    fn volatility_from_period(&self, expiry: Period, key: f64) -> Result<T> {
        let points = self
            .points
            .iter()
            .flat_map(|(tenor, smile)| {
                smile.iter().map(move |(axis, value)| BilinearPoint {
                    x: tenor.period_in_year(),
                    y: axis.value(),
                    value: *value,
                })
            })
            .collect::<Vec<_>>();

        BilinearInterpolator::interpolate(expiry.period_in_year(), key, points).ok_or(
            AtlasError::InterpolationErr(
                "Could not bilinearly interpolate volatility for requested expiry/key".into(),
            ),
        )
    }

    fn volatility_type(&self) -> crate::volatility::volatilityindexing::VolatilityType {
        crate::volatility::volatilityindexing::VolatilityType::Black
    }

    fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    fn reference_date(&self) -> Date {
        self.reference_date
    }

    fn smile_type(&self) -> crate::volatility::volatilityindexing::SmileType {
        crate::volatility::volatilityindexing::SmileType::Strike
    }
}

impl Pillars<ADReal> for InterpolatedVolatilitySurface<ADReal> {
    fn pillars(&self) -> Option<Vec<(String, &ADReal)>> {
        self.labels.as_ref().map(|labels| {
            labels
                .iter()
                .zip(self.points.values().flat_map(|m| m.values()))
                .map(|(label, value)| (label.clone(), value))
                .collect()
        })
    }

    fn pillar_labels(&self) -> Option<Vec<String>> {
        self.labels.clone()
    }

    fn put_pillars_on_tape(&mut self) {
        for m in self.points.values_mut() {
            for value in m.values_mut() {
                value.put_on_tape();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        ad::{adreal::{ADReal, IsReal}, tape::Tape},
        indices::marketindex::MarketIndex,
        time::{date::Date, enums::TimeUnit, period::Period},
        volatility::{
            interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
            volatilityindexing::F64Key,
            volatilitysurface::VolatilitySurface,
        },
    };

    #[test]
    fn interpolated_surface_returns_bilinear_value_for_f64() {
        let mut points = BTreeMap::new();

        points.insert(
            Period::new(6, TimeUnit::Months),
            BTreeMap::from([(F64Key::new(90.0), 0.20), (F64Key::new(110.0), 0.30)]),
        );
        points.insert(
            Period::new(12, TimeUnit::Months),
            BTreeMap::from([(F64Key::new(90.0), 0.22), (F64Key::new(110.0), 0.34)]),
        );

        let surface = InterpolatedVolatilitySurface::new(
            Date::new(2025, 1, 1),
            MarketIndex::Equity("SPX".to_string()),
            points,
        );

        let vol = surface
            .volatility_from_period(Period::new(9, TimeUnit::Months), 100.0)
            .expect("surface interpolation should work");

        assert!((vol - 0.265).abs() < 1e-12);
    }

    #[test]
    fn interpolated_surface_returns_bilinear_value_for_adreal() {
        Tape::start_recording();

        let mut points = BTreeMap::new();
        points.insert(
            Period::new(6, TimeUnit::Months),
            BTreeMap::from([
                (F64Key::new(90.0), ADReal::from(0.20)),
                (F64Key::new(110.0), ADReal::from(0.30)),
            ]),
        );
        points.insert(
            Period::new(12, TimeUnit::Months),
            BTreeMap::from([
                (F64Key::new(90.0), ADReal::from(0.22)),
                (F64Key::new(110.0), ADReal::from(0.34)),
            ]),
        );

        let surface = InterpolatedVolatilitySurface::new(
            Date::new(2025, 1, 1),
            MarketIndex::Equity("SPX".to_string()),
            points,
        );

        let vol = surface
            .volatility_from_period(Period::new(9, TimeUnit::Months), 100.0)
            .expect("surface interpolation should work");

        assert!((vol.value() - 0.265).abs() < 1e-12);
    }

    #[test]
    fn interpolated_surface_out_of_grid_returns_error() {
        let mut points = BTreeMap::new();
        points.insert(
            Period::new(6, TimeUnit::Months),
            BTreeMap::from([(F64Key::new(90.0), 0.20), (F64Key::new(110.0), 0.30)]),
        );
        points.insert(
            Period::new(12, TimeUnit::Months),
            BTreeMap::from([(F64Key::new(90.0), 0.22), (F64Key::new(110.0), 0.34)]),
        );

        let surface = InterpolatedVolatilitySurface::new(
            Date::new(2025, 1, 1),
            MarketIndex::Equity("SPX".to_string()),
            points,
        );

        let vol = surface.volatility_from_period(Period::new(3, TimeUnit::Months), 100.0);
        assert!(vol.is_err());
    }
}

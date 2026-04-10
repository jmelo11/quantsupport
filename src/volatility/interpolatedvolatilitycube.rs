use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{elements::volatilitycubelement::ADVolatilityCubeElement, pillars::Pillars},
    indices::marketindex::MarketIndex,
    math::interpolation::trilinear::{TrilinearInterpolator, TrilinearPoint},
    time::{date::Date, period::Period},
    utils::errors::{QSError, Result},
    volatility::{
        volatilityindexing::{F64Key, SmileType, VolatilityType},
        volatilitycube::VolatilityCube,
    },
};
use std::collections::BTreeMap;

use crate::math::interpolation::bilinear::BilinearValue;

type CubeMap<T> = BTreeMap<Period, BTreeMap<Period, BTreeMap<F64Key, T>>>;

/// Concrete implementation of [`VolatilityCube`] backed by trilinear interpolation.
///
/// The three axes are: option expiry (x), underlying tenor/maturity (y), and
/// strike/delta/log-moneyness (z).
pub struct InterpolatedVolatilityCube<T: Scalar> {
    reference_date: Date,
    market_index: MarketIndex,
    points: CubeMap<T>,
    labels: Option<Vec<String>>,
    volatility_type: VolatilityType,
    smile_type: SmileType,
}

impl<T: Scalar> InterpolatedVolatilityCube<T> {
    /// Creates a new `InterpolatedVolatilityCube`.
    #[must_use]
    pub const fn new(
        reference_date: Date,
        market_index: MarketIndex,
        points: CubeMap<T>,
        volatility_type: VolatilityType,
        smile_type: SmileType,
    ) -> Self {
        Self {
            reference_date,
            market_index,
            points,
            labels: None,
            volatility_type,
            smile_type,
        }
    }

    /// Attaches labels to each volatility pillar used in sensitivity reports.
    #[must_use]
    pub fn with_labels(mut self, labels: &[String]) -> Self {
        self.labels = Some(labels.to_owned());
        self
    }

    /// Returns the market index associated with the volatility cube.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }
}

impl<T: BilinearValue> VolatilityCube<T> for InterpolatedVolatilityCube<T> {
    fn volatility_from_period(&self, expiry: Period, maturity: Period, key: f64) -> Result<T> {
        let points: Vec<TrilinearPoint<T>> = self
            .points
            .iter()
            .flat_map(|(exp, tenor_map)| {
                tenor_map.iter().flat_map(move |(tenor, smile)| {
                    smile.iter().map(move |(axis, value)| TrilinearPoint {
                        x: exp.period_in_year(),
                        y: tenor.period_in_year(),
                        z: axis.value(),
                        value: *value,
                    })
                })
            })
            .collect();

        TrilinearInterpolator::interpolate(
            expiry.period_in_year(),
            maturity.period_in_year(),
            key,
            points,
        )
        .ok_or_else(|| {
            QSError::InterpolationErr(
                "Could not trilinearly interpolate volatility for requested expiry/maturity/key"
                    .into(),
            )
        })
    }

    fn volatility_type(&self) -> VolatilityType {
        self.volatility_type.clone()
    }

    fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    fn reference_date(&self) -> Date {
        self.reference_date
    }

    fn smile_type(&self) -> SmileType {
        self.smile_type
    }
}

impl Pillars<DualFwd> for InterpolatedVolatilityCube<DualFwd> {
    fn pillars(&self) -> Option<Vec<(String, &DualFwd)>> {
        self.labels.as_ref().map(|labels| {
            labels
                .iter()
                .zip(
                    self.points
                        .values()
                        .flat_map(|t| t.values().flat_map(|m| m.values())),
                )
                .map(|(label, value)| (label.clone(), value))
                .collect()
        })
    }

    fn pillar_labels(&self) -> Option<Vec<String>> {
        self.labels.clone()
    }

    fn put_pillars_on_tape(&mut self) {
        for t in self.points.values_mut() {
            for m in t.values_mut() {
                for value in m.values_mut() {
                    value.put_on_tape();
                }
            }
        }
    }
}

impl ADVolatilityCubeElement for InterpolatedVolatilityCube<DualFwd> {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        ad::dual::DualFwd,
        indices::marketindex::MarketIndex,
        time::{date::Date, enums::TimeUnit, period::Period},
        volatility::{
            volatilitycube::VolatilityCube,
            volatilityindexing::{F64Key, SmileType, VolatilityType},
        },
    };

    use super::InterpolatedVolatilityCube;

    fn build_cube_f64() -> InterpolatedVolatilityCube<f64> {
        // 2 expiries × 2 tenors × 2 strikes
        let mut points = BTreeMap::new();
        for (exp_n, exp_unit) in [(6, TimeUnit::Months), (12, TimeUnit::Months)] {
            let mut tenor_map = BTreeMap::new();
            for (ten_n, ten_unit) in [(1, TimeUnit::Years), (5, TimeUnit::Years)] {
                let exp_y = Period::new(exp_n, exp_unit).period_in_year();
                let ten_y = Period::new(ten_n, ten_unit).period_in_year();
                // v = 0.20 + 0.02*exp_y + 0.01*ten_y + 0.001*strike
                let smile = BTreeMap::from([
                    (
                        F64Key::new(0.02),
                        0.20 + 0.02 * exp_y + 0.01 * ten_y + 0.001 * 0.02,
                    ),
                    (
                        F64Key::new(0.05),
                        0.20 + 0.02 * exp_y + 0.01 * ten_y + 0.001 * 0.05,
                    ),
                ]);
                tenor_map.insert(Period::new(ten_n, ten_unit), smile);
            }
            points.insert(Period::new(exp_n, exp_unit), tenor_map);
        }

        InterpolatedVolatilityCube::new(
            Date::new(2025, 1, 1),
            MarketIndex::SOFR,
            points,
            VolatilityType::Black,
            SmileType::Strike,
        )
    }

    #[test]
    fn cube_interpolates_interior_f64() {
        let cube = build_cube_f64();
        let vol = cube
            .volatility_from_period(
                Period::new(9, TimeUnit::Months),
                Period::new(3, TimeUnit::Years),
                0.035,
            )
            .expect("cube interpolation should work");
        // v = 0.20 + 0.02*0.75 + 0.01*3.0 + 0.001*0.035 = 0.245035
        assert!((vol - 0.245_035).abs() < 1e-10, "got {vol}");
    }

    #[test]
    fn cube_interpolates_interior_dualfwd() {
        let f64_cube = build_cube_f64();
        // rebuild with DualFwd values
        let mut points = BTreeMap::new();
        for (exp, tenor_map) in f64_cube.points.iter() {
            let mut dm = BTreeMap::new();
            for (tenor, smile) in tenor_map {
                let ds: BTreeMap<F64Key, DualFwd> = smile
                    .iter()
                    .map(|(k, v)| (k.clone(), DualFwd::from(*v)))
                    .collect();
                dm.insert(*tenor, ds);
            }
            points.insert(*exp, dm);
        }

        let cube = InterpolatedVolatilityCube::new(
            Date::new(2025, 1, 1),
            MarketIndex::SOFR,
            points,
            VolatilityType::Black,
            SmileType::Strike,
        );

        let vol = cube
            .volatility_from_period(
                Period::new(9, TimeUnit::Months),
                Period::new(3, TimeUnit::Years),
                0.035,
            )
            .expect("cube interpolation DualFwd");
        assert!((vol.value() - 0.245_035).abs() < 1e-10);
    }

    #[test]
    fn cube_out_of_grid_returns_error() {
        let cube = build_cube_f64();
        let vol = cube.volatility_from_period(
            Period::new(3, TimeUnit::Months),
            Period::new(3, TimeUnit::Years),
            0.035,
        );
        assert!(vol.is_err());
    }
}

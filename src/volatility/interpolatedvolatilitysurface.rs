use crate::core::marketdatahandling::constructedelementstore::SharedElement;
use crate::{
    ad::{dual::DualFwd, forward::ADForward, scalar::Scalar},
    core::{elements::volatilitysurfaceelement::ADVolatilitySurfaceElement, pillars::Pillars},
    indices::marketindex::MarketIndex,
    math::interpolation::{
        bilinear::{BilinearInterpolator, BilinearPoint, BilinearValue},
        extrapolation::ExtrapolationPolicy,
    },
    time::{date::Date, period::Period},
    utils::errors::{QSError, Result},
    volatility::{
        volatilityindexing::{F64Key, SmileType, VolatilityType},
        volatilitysurface::VolatilitySurface,
    },
};
use std::collections::BTreeMap;
use std::{cell::RefCell, rc::Rc};

type SurfaceMap<T> = BTreeMap<Period, BTreeMap<F64Key, T>>;

/// Represents an interpolated volatility surface.
///
/// Interpolates each expiry's smile before interpolating along expiry. Smiles
/// may have different strike grids. Out-of-range queries are rejected unless
/// [`Self::with_extrapolation_policy`] selects flat extrapolation.
/// Interpolation is linear in volatility, not total variance. Expiry coordinates
/// use `Period::period_in_year`; date queries consequently use calendar days/365.
/// Smile keys are used as supplied: metadata does not convert strikes to delta
/// or log-moneyness.
///
/// ## Generics
/// - `T`: Numeric type for the volatility values (e.g., `f64`, `DualFwd`).
#[derive(Clone)]
pub struct InterpolatedVolatilitySurface<T: Scalar> {
    reference_date: Date,
    market_index: MarketIndex,
    points: SurfaceMap<T>,
    labels: Option<Vec<String>>,
    calibration_instrument_ids: Option<Vec<String>>,
    volatility_type: VolatilityType,
    smile_type: SmileType,
    extrapolation_policy: ExtrapolationPolicy,
}

impl<T: Scalar> InterpolatedVolatilitySurface<T> {
    /// Creates a new `VolatilitySurface`.
    #[must_use]
    pub const fn new(
        reference_date: Date,
        market_index: MarketIndex,
        points: SurfaceMap<T>,
        volatility_type: VolatilityType,
        smile_type: SmileType,
    ) -> Self {
        Self {
            reference_date,
            market_index,
            points,
            labels: None,
            calibration_instrument_ids: None,
            volatility_type,
            smile_type,
            extrapolation_policy: ExtrapolationPolicy::Reject,
        }
    }

    /// Sets the policy for queries outside expiry or smile slices.
    #[must_use]
    pub const fn with_extrapolation_policy(mut self, policy: ExtrapolationPolicy) -> Self {
        self.extrapolation_policy = policy;
        self
    }

    /// Returns the policy for queries outside the grid.
    #[must_use]
    pub const fn extrapolation_policy(&self) -> ExtrapolationPolicy {
        self.extrapolation_policy
    }

    /// Attaches one sensitivity label to each differentiable grid value.
    ///
    /// These labels identify adjoints returned by automatic differentiation.
    /// They may differ from the source option identifiers used to select
    /// calibration instruments.
    #[must_use]
    pub fn with_labels(mut self, labels: &[String]) -> Self {
        self.labels = Some(labels.to_owned());
        self
    }

    /// Attaches source option identifiers for calibration-basket selection.
    ///
    /// The identifiers provide expiry, tenor, and strike metadata to model
    /// calibrators. [`Pillars`] defines the differentiable values reported by
    /// the risk system.
    #[must_use]
    pub fn with_calibration_instrument_ids(mut self, identifiers: &[String]) -> Self {
        self.calibration_instrument_ids = Some(identifiers.to_owned());
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

        BilinearInterpolator::interpolate_with_extrapolation(
            expiry.period_in_year(), key, points, self.extrapolation_policy,
        ).ok_or_else(|| {
            QSError::InterpolationErr(format!(
                "Could not bilinearly interpolate volatility for expiry {expiry} and key {key} on {}",
                self.market_index
            ))
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

    fn calibration_instrument_ids(&self) -> Option<Vec<String>> {
        self.calibration_instrument_ids.clone()
    }
}

impl Pillars<DualFwd> for InterpolatedVolatilitySurface<DualFwd> {
    fn pillars(&self) -> Option<Vec<(String, &DualFwd)>> {
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

impl ADVolatilitySurfaceElement for InterpolatedVolatilitySurface<DualFwd> {
    fn clone_surface(&self) -> Result<SharedElement<dyn ADVolatilitySurfaceElement>> {
        let mut market = self.clone();
        for value in market
            .points
            .values_mut()
            .flat_map(|values| values.values_mut())
        {
            *value = DualFwd::constant(ADForward::constant(value.value()));
        }
        Ok(Rc::new(RefCell::new(market)))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        ad::{dual::DualFwd, tape::Tape},
        core::pillars::Pillars,
        indices::marketindex::MarketIndex,
        math::interpolation::extrapolation::ExtrapolationPolicy,
        time::{date::Date, enums::TimeUnit, period::Period},
        volatility::{
            interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
            volatilityindexing::{F64Key, SmileType, VolatilityType},
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
            VolatilityType::Black,
            SmileType::Strike,
        );

        let vol = surface
            .volatility_from_period(Period::new(9, TimeUnit::Months), 100.0)
            .expect("surface interpolation should work");

        assert!((vol - 0.265).abs() < 1e-12);
    }

    #[test]
    fn interpolated_surface_returns_bilinear_value_for_adreal() {
        Tape::start_recording_fwd();

        let mut points = BTreeMap::new();
        points.insert(
            Period::new(6, TimeUnit::Months),
            BTreeMap::from([
                (F64Key::new(90.0), DualFwd::from(0.20)),
                (F64Key::new(110.0), DualFwd::from(0.30)),
            ]),
        );
        points.insert(
            Period::new(12, TimeUnit::Months),
            BTreeMap::from([
                (F64Key::new(90.0), DualFwd::from(0.22)),
                (F64Key::new(110.0), DualFwd::from(0.34)),
            ]),
        );

        let surface = InterpolatedVolatilitySurface::new(
            Date::new(2025, 1, 1),
            MarketIndex::Equity("SPX".to_string()),
            points,
            VolatilityType::Black,
            SmileType::Strike,
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
            VolatilityType::Black,
            SmileType::Strike,
        );

        let vol = surface.volatility_from_period(Period::new(3, TimeUnit::Months), 100.0);
        assert!(vol.is_err());
    }

    #[test]
    fn surface_supports_single_expiry_single_strike_and_single_node() {
        let six_months = Period::new(6, TimeUnit::Months);
        let one_year = Period::new(1, TimeUnit::Years);
        for (points, expiry, strike, expected) in [
            (
                BTreeMap::from([(
                    six_months,
                    BTreeMap::from([(F64Key::new(90.0), 0.20), (F64Key::new(110.0), 0.30)]),
                )]),
                six_months,
                100.0,
                0.25,
            ),
            (
                BTreeMap::from([
                    (six_months, BTreeMap::from([(F64Key::new(100.0), 0.20)])),
                    (one_year, BTreeMap::from([(F64Key::new(100.0), 0.30)])),
                ]),
                Period::new(9, TimeUnit::Months),
                100.0,
                0.25,
            ),
            (
                BTreeMap::from([(six_months, BTreeMap::from([(F64Key::new(100.0), 0.20)]))]),
                six_months,
                100.0,
                0.20,
            ),
        ] {
            let surface = InterpolatedVolatilitySurface::new(
                Date::new(2025, 1, 1),
                MarketIndex::SOFR,
                points,
                VolatilityType::Black,
                SmileType::Strike,
            );
            let value = surface
                .volatility_from_period(expiry, strike)
                .expect("degenerate surface");
            assert!((value - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn sparse_surface_preserves_quote_weights_after_tape_registration() {
        Tape::start_recording_fwd();
        let mut surface = InterpolatedVolatilitySurface::new(
            Date::new(2025, 1, 1),
            MarketIndex::SOFR,
            BTreeMap::from([
                (
                    Period::new(6, TimeUnit::Months),
                    BTreeMap::from([
                        (F64Key::new(90.0), DualFwd::from(0.20)),
                        (F64Key::new(110.0), DualFwd::from(0.30)),
                    ]),
                ),
                (
                    Period::new(12, TimeUnit::Months),
                    BTreeMap::from([
                        (F64Key::new(95.0), DualFwd::from(0.22)),
                        (F64Key::new(115.0), DualFwd::from(0.34)),
                    ]),
                ),
            ]),
            VolatilityType::Black,
            SmileType::Strike,
        )
        .with_labels(&[
            "6M_low".into(),
            "6M_high".into(),
            "1Y_low".into(),
            "1Y_high".into(),
        ])
        .with_extrapolation_policy(ExtrapolationPolicy::Flat);
        surface.put_pillars_on_tape();
        for (expiry, key, expected_value, expected_weights) in [
            (
                Period::new(9, TimeUnit::Months),
                100.0,
                0.25,
                [0.25, 0.25, 0.375, 0.125],
            ),
            (
                Period::new(12, TimeUnit::Months),
                115.0,
                0.34,
                [0.0, 0.0, 0.0, 1.0],
            ),
            (
                Period::new(18, TimeUnit::Months),
                120.0,
                0.34,
                [0.0, 0.0, 0.0, 1.0],
            ),
        ] {
            Tape::reset_adjoints_fwd();
            let value = surface
                .volatility_from_period(expiry, key)
                .expect("sparse surface query");
            assert!((value.value() - expected_value).abs() < 1e-12);
            value.backward().expect("surface backward sweep");
            for ((_, pillar), weight) in surface
                .pillars()
                .expect("labelled pillars")
                .iter()
                .zip(expected_weights)
            {
                assert!((pillar.adjoint().expect("quote adjoint").value() - weight).abs() < 1e-12);
            }
        }
        Tape::stop_recording_fwd();
    }
}

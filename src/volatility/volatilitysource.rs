//! Time-dependent volatility adapters for market-data components.
//!
//! Components that directly consume volatility use the
//! [`TimeDependentVolatility`] trait. This module connects that trait to flat
//! values and constructed volatility markets:
//!
//! - [`ConstantVolatility`] — a flat volatility, useful for debugging and
//!   simple setups.
//! - [`SurfaceTermVolatility`] — samples a constructed
//!   [`VolatilitySurfaceElement`] at a fixed smile key as a function of time.
//! - [`CubeTermVolatility`] — samples a constructed [`VolatilityCubeElement`]
//!   at a fixed tenor and smile key as a function of time.
//!
//! The serde-enabled [`VolatilitySourceConfiguration`] selects the source from
//! JSON and is resolved against a
//! [`ConstructedElementStore`]
//! once surfaces and cubes have been built.

use serde::{Deserialize, Serialize};

use std::str::FromStr;

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        elements::{
            volatilitycubelement::VolatilityCubeElement,
            volatilitysurfaceelement::VolatilitySurfaceElement,
        },
        marketdatahandling::constructedelementstore::ConstructedElementStore,
    },
    indices::marketindex::MarketIndex,
    models::montecarloengine::TimeDependentVolatility,
    quotes::quote::QuoteDetails,
    time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
    utils::errors::{QSError, Result},
    volatility::{
        modelcalibration::{CalibrationSource, ModelCalibrationConfiguration},
        volatilityindexing::{Strike, VolatilityType},
    },
};

/// Converts a year fraction into a [`Period`] expressed in days.
///
/// # Errors
/// Returns an error if `t` is negative or too large to represent.
pub fn period_from_year_fraction(t: f64) -> Result<Period> {
    let days = (t * 365.0).round();
    if !(0.0..=f64::from(i32::MAX)).contains(&days) {
        return Err(QSError::InvalidValueErr(format!(
            "Cannot convert year fraction {t} into a period"
        )));
    }
    #[allow(clippy::cast_possible_truncation)]
    Ok(Period::new(days as i32, TimeUnit::Days))
}

/// A flat, time-independent volatility. Useful for debugging models without
/// market data, and as a simple configuration option.
#[derive(Clone, Copy, Debug)]
pub struct ConstantVolatility<T: Scalar> {
    value: T,
}

impl<T: Scalar> ConstantVolatility<T> {
    /// Creates a new constant volatility.
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T: Scalar> TimeDependentVolatility<T> for ConstantVolatility<T> {
    fn vol(&self, _t: f64) -> Result<T> {
        Ok(self.value)
    }
}

/// A piecewise-constant time-dependent volatility.
///
/// Each `(year_fraction, sigma)` entry applies from its year fraction onward;
/// the first entry also applies before its year fraction.
#[derive(Clone, Debug)]
pub struct PiecewiseConstantVolatility<T: Scalar> {
    schedule: Vec<(f64, T)>,
}

impl<T: Scalar> PiecewiseConstantVolatility<T> {
    /// Creates a new piecewise-constant volatility from a schedule of
    /// `(year_fraction, sigma)` pairs.
    ///
    /// # Errors
    /// Returns an error if the schedule is empty or not strictly increasing
    /// in time.
    pub fn new(schedule: Vec<(f64, T)>) -> Result<Self> {
        if schedule.is_empty() {
            return Err(QSError::InvalidValueErr(
                "PiecewiseConstantVolatility: schedule cannot be empty".into(),
            ));
        }
        if !schedule.windows(2).all(|w| w[0].0 < w[1].0) {
            return Err(QSError::InvalidValueErr(
                "PiecewiseConstantVolatility: schedule times must be strictly increasing".into(),
            ));
        }
        Ok(Self { schedule })
    }

    /// Iterates over `(year_fraction, sigma)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = &(f64, T)> {
        self.schedule.iter()
    }
}

impl<T: Scalar> TimeDependentVolatility<T> for PiecewiseConstantVolatility<T> {
    fn vol(&self, t: f64) -> Result<T> {
        let mut val = self.schedule[0].1;
        for &(ti, vi) in &self.schedule {
            if ti > t {
                break;
            }
            val = vi;
        }
        Ok(val)
    }
}

/// Bootstraps a piecewise-constant forward volatility from Black implied vols.
///
/// The vols are read off a constructed surface or cube so that the total
/// variance at each quoted expiry is reproduced:
/// `∫₀^{T_i} σ(s)² ds = σ_impl(T_i)² T_i`.
///
/// The configuration's calibration basket selects surface or cube pillars
/// according to expiry, tenor, and strike. A common strike rule creates one
/// pillar for each selected expiry and tenor. This bootstrap receives no
/// forward curve, so it supports [`Strike::Absolute`] values. Curve-aware
/// calibrators such as Hull-White and LGM also resolve ATM and relative
/// moneyness against forward rates. This function supplies calibrated
/// volatility to lognormal models such as
/// [`BrownianMotion`](crate::models::brownianmotion::BrownianMotion).
///
/// # Errors
/// Returns an error if the surface/cube has not been constructed, if a quote
/// identifier lacks an option expiry or absolute strike, or if a forward
/// variance is negative (arbitrageable term structure).
pub fn bootstrap_black_term_volatility(
    configuration: &ModelCalibrationConfiguration,
    store: &ConstructedElementStore,
    reference_date: Date,
    day_counter: DayCounter,
) -> Result<PiecewiseConstantVolatility<f64>> {
    let quote_ids = configuration.resolve_instrument_ids(store)?;
    let mut seen_pillars: Vec<(Option<Period>, Option<Period>)> = Vec::new();
    let mut pillars: Vec<(f64, f64)> = Vec::with_capacity(quote_ids.len());
    for id in &quote_ids {
        let details = QuoteDetails::from_str(id)?;
        if matches!(details.vol_type(), Some(VolatilityType::Normal)) {
            return Err(QSError::InvalidValueErr(format!(
                "Black term-vol bootstrap requires Black (lognormal) vols, got a Normal \
                 quote: {id}"
            )));
        }
        let expiry = details.option_expiry().ok_or_else(|| {
            QSError::InvalidValueErr(format!("Calibration quote {id} has no option expiry"))
        })?;
        if configuration.strike().is_some() {
            let pillar = (details.option_expiry(), details.tenor());
            if seen_pillars.contains(&pillar) {
                continue;
            }
            seen_pillars.push(pillar);
        }
        let strike_spec = configuration.strike().or_else(|| details.strike());
        let key = match strike_spec {
            Some(Strike::Absolute(k)) => k,
            other => {
                return Err(QSError::InvalidValueErr(format!(
                    "Black term-vol bootstrap requires absolute strikes (no forward curve \
                     available to resolve moneyness), got {other:?} for {id}"
                )));
            }
        };
        let implied = match configuration.source() {
            CalibrationSource::Surface { market_index } => {
                let element = store.volatility_surface(market_index).ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Volatility surface not found for index {market_index}"
                    ))
                })?;
                let surface = element.surface();
                if surface.volatility_type() == VolatilityType::Normal {
                    return Err(QSError::InvalidValueErr(format!(
                        "Black term-vol bootstrap requires a Black (lognormal) surface, but \
                         the surface for {market_index} is Normal"
                    )));
                }
                surface.volatility_from_period(expiry, key)?.value()
            }
            CalibrationSource::Cube { market_index } => {
                let tenor = details.tenor().ok_or_else(|| {
                    QSError::InvalidValueErr(format!(
                        "Calibration quote {id} has no tenor (required for cube lookup)"
                    ))
                })?;
                let element = store.volatility_cube(market_index).ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Volatility cube not found for index {market_index}"
                    ))
                })?;
                let cube = element.cube();
                if cube.volatility_type() == VolatilityType::Normal {
                    return Err(QSError::InvalidValueErr(format!(
                        "Black term-vol bootstrap requires a Black (lognormal) cube, but \
                         the cube for {market_index} is Normal"
                    )));
                }
                cube.volatility_from_period(expiry, tenor, key)?.value()
            }
        };
        let t = day_counter.year_fraction(reference_date, reference_date + expiry);
        if t <= 0.0 {
            return Err(QSError::InvalidValueErr(format!(
                "Calibration quote {id} has non-positive expiry {t}"
            )));
        }
        pillars.push((t, implied));
    }
    pillars.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut schedule = Vec::with_capacity(pillars.len());
    let mut prev_t = 0.0_f64;
    let mut prev_var = 0.0_f64;
    for (t, implied) in pillars {
        let dt = t - prev_t;
        if dt <= 0.0 {
            return Err(QSError::InvalidValueErr(format!(
                "Duplicate calibration expiry at t = {t}"
            )));
        }
        let total_var = implied * implied * t;
        let forward_var = total_var - prev_var;
        if forward_var < 0.0 {
            return Err(QSError::InvalidValueErr(format!(
                "Negative forward variance {forward_var} on ({prev_t}, {t}]"
            )));
        }
        schedule.push((prev_t, (forward_var / dt).sqrt()));
        prev_t = t;
        prev_var = total_var;
    }
    PiecewiseConstantVolatility::new(schedule)
}

/// Term volatility sampled from a constructed volatility surface.
///
/// The surface is evaluated at a fixed smile key (strike, delta, or
/// log-moneyness depending on the surface's
/// [`SmileType`](crate::volatility::volatilityindexing::SmileType)).
///
/// `vol(t)` bilinearly interpolates the surface at expiry `t` (in years) and
/// the configured key.
#[derive(Clone)]
pub struct SurfaceTermVolatility {
    element: VolatilitySurfaceElement,
    key: f64,
}

impl SurfaceTermVolatility {
    /// Creates a new surface-backed term volatility sampled at `key`.
    #[must_use]
    pub const fn new(element: VolatilitySurfaceElement, key: f64) -> Self {
        Self { element, key }
    }
}

impl TimeDependentVolatility<f64> for SurfaceTermVolatility {
    fn vol(&self, t: f64) -> Result<f64> {
        let period = period_from_year_fraction(t)?;
        Ok(self
            .element
            .surface()
            .volatility_from_period(period, self.key)?
            .value())
    }
}

impl TimeDependentVolatility<DualFwd> for SurfaceTermVolatility {
    fn vol(&self, t: f64) -> Result<DualFwd> {
        let period = period_from_year_fraction(t)?;
        self.element
            .surface()
            .volatility_from_period(period, self.key)
    }
}

/// Term volatility sampled from a constructed volatility cube at a fixed
/// underlying tenor and smile key.
///
/// `vol(t)` trilinearly interpolates the cube at expiry `t` (in years), the
/// configured tenor, and the configured key.
#[derive(Clone)]
pub struct CubeTermVolatility {
    element: VolatilityCubeElement,
    tenor: Period,
    key: f64,
}

impl CubeTermVolatility {
    /// Creates a new cube-backed term volatility sampled at `tenor` and `key`.
    #[must_use]
    pub const fn new(element: VolatilityCubeElement, tenor: Period, key: f64) -> Self {
        Self {
            element,
            tenor,
            key,
        }
    }
}

impl TimeDependentVolatility<f64> for CubeTermVolatility {
    fn vol(&self, t: f64) -> Result<f64> {
        let period = period_from_year_fraction(t)?;
        Ok(self
            .element
            .cube()
            .volatility_from_period(period, self.tenor, self.key)?
            .value())
    }
}

impl TimeDependentVolatility<DualFwd> for CubeTermVolatility {
    fn vol(&self, t: f64) -> Result<DualFwd> {
        let period = period_from_year_fraction(t)?;
        self.element
            .cube()
            .volatility_from_period(period, self.tenor, self.key)
    }
}

/// Serde-enabled configuration for a direct volatility adapter.
///
/// ## JSON examples
/// ```json
/// { "Constant": { "value": 0.2 } }
/// { "Surface": { "market_index": "SOFR", "key": 0.03 } }
/// { "Cube": { "market_index": "SOFR", "tenor": "5Y", "key": 0.03 } }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum VolatilitySourceConfiguration {
    /// A flat volatility (useful for debugging).
    Constant {
        /// The constant volatility value.
        value: f64,
    },
    /// Sample a constructed volatility surface at a fixed smile key.
    Surface {
        /// Market index identifying the surface.
        market_index: MarketIndex,
        /// Smile key (strike, delta, or log-moneyness) to sample at.
        key: f64,
    },
    /// Sample a constructed volatility cube at a fixed tenor and smile key.
    Cube {
        /// Market index identifying the cube.
        market_index: MarketIndex,
        /// Underlying tenor to sample at.
        tenor: Period,
        /// Smile key (strike, delta, or log-moneyness) to sample at.
        key: f64,
    },
}

impl VolatilitySourceConfiguration {
    /// Resolves the configuration into a [`TimeDependentVolatility`] using the
    /// constructed elements store.
    ///
    /// # Errors
    /// Returns an error if the referenced surface or cube is missing from the
    /// constructed-element store.
    pub fn resolve(
        &self,
        store: &ConstructedElementStore,
    ) -> Result<Box<dyn TimeDependentVolatility<f64>>> {
        match self {
            Self::Constant { value } => Ok(Box::new(ConstantVolatility::new(*value))),
            Self::Surface { market_index, key } => {
                let element = store.volatility_surface(market_index).ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Volatility surface not found for index {market_index}"
                    ))
                })?;
                Ok(Box::new(SurfaceTermVolatility::new(element.clone(), *key)))
            }
            Self::Cube {
                market_index,
                tenor,
                key,
            } => {
                let element = store.volatility_cube(market_index).ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Volatility cube not found for index {market_index}"
                    ))
                })?;
                Ok(Box::new(CubeTermVolatility::new(
                    element.clone(),
                    *tenor,
                    *key,
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

    use super::*;
    use crate::{
        ad::dual::DualFwd,
        time::date::Date,
        volatility::{
            interpolatedvolatilitycube::InterpolatedVolatilityCube,
            interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
            modelcalibration::CalibrationSource,
            volatilityindexing::{F64Key, SmileType, VolatilityType},
        },
    };

    fn sample_surface_element() -> VolatilitySurfaceElement {
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
        VolatilitySurfaceElement::new(
            MarketIndex::Equity("SPX".to_string()),
            Rc::new(RefCell::new(surface)),
        )
    }

    fn sample_cube_element() -> VolatilityCubeElement {
        let mut points = BTreeMap::new();
        for (exp_n, exp_v) in [(6, 0.20), (12, 0.24)] {
            let mut tenor_map = BTreeMap::new();
            for ten_n in [1, 5] {
                let smile = BTreeMap::from([
                    (F64Key::new(0.02), DualFwd::from(exp_v)),
                    (F64Key::new(0.05), DualFwd::from(exp_v)),
                ]);
                tenor_map.insert(Period::new(ten_n, TimeUnit::Years), smile);
            }
            points.insert(Period::new(exp_n, TimeUnit::Months), tenor_map);
        }
        let cube = InterpolatedVolatilityCube::new(
            Date::new(2025, 1, 1),
            MarketIndex::SOFR,
            points,
            VolatilityType::Black,
            SmileType::Strike,
        );
        VolatilityCubeElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(cube)))
    }

    #[test]
    fn period_from_year_fraction_converts_days() -> Result<()> {
        assert_eq!(
            period_from_year_fraction(1.0)?,
            Period::new(365, TimeUnit::Days)
        );
        assert_eq!(
            period_from_year_fraction(0.5)?,
            Period::new(183, TimeUnit::Days)
        );
        assert!(period_from_year_fraction(-0.1).is_err());
        Ok(())
    }

    #[test]
    fn constant_volatility_is_flat() -> Result<()> {
        let vol = ConstantVolatility::new(0.2_f64);
        assert!((vol.vol(0.1)? - 0.2).abs() < 1e-15);
        assert!((vol.vol(10.0)? - 0.2).abs() < 1e-15);
        Ok(())
    }

    #[test]
    fn surface_term_volatility_samples_surface() -> Result<()> {
        let source = SurfaceTermVolatility::new(sample_surface_element(), 100.0);
        // t = 0.75y ≈ 9M: bilinear between 6M (0.25 at key 100) and 12M (0.28).
        let vol: f64 = TimeDependentVolatility::<f64>::vol(&source, 0.75)?;
        assert!((vol - 0.265).abs() < 5e-3, "got {vol}");
        let vol_ad: DualFwd = TimeDependentVolatility::<DualFwd>::vol(&source, 0.75)?;
        assert!((vol_ad.value() - vol).abs() < 1e-15);
        Ok(())
    }

    #[test]
    fn cube_term_volatility_samples_cube() -> Result<()> {
        let source = CubeTermVolatility::new(
            sample_cube_element(),
            Period::new(3, TimeUnit::Years),
            0.035,
        );
        // Flat 0.20 at 6M and 0.24 at 12M → ~0.22 at 9M.
        let vol: f64 = TimeDependentVolatility::<f64>::vol(&source, 0.75)?;
        assert!((vol - 0.22).abs() < 5e-3, "got {vol}");
        Ok(())
    }

    #[test]
    fn configuration_resolves_constant_surface_and_cube() -> Result<()> {
        let mut store = ConstructedElementStore::default();
        store.volatility_surfaces_mut().insert(
            MarketIndex::Equity("SPX".to_string()),
            sample_surface_element(),
        );
        store
            .volatility_cubes_mut()
            .insert(MarketIndex::SOFR, sample_cube_element());

        let constant = VolatilitySourceConfiguration::Constant { value: 0.3 };
        assert!((constant.resolve(&store)?.vol(1.0)? - 0.3).abs() < 1e-15);

        let surface = VolatilitySourceConfiguration::Surface {
            market_index: MarketIndex::Equity("SPX".to_string()),
            key: 100.0,
        };
        assert!((surface.resolve(&store)?.vol(0.75)? - 0.265).abs() < 5e-3);

        let cube = VolatilitySourceConfiguration::Cube {
            market_index: MarketIndex::SOFR,
            tenor: Period::new(3, TimeUnit::Years),
            key: 0.035,
        };
        assert!((cube.resolve(&store)?.vol(0.75)? - 0.22).abs() < 5e-3);
        Ok(())
    }

    #[test]
    fn configuration_resolve_errors() {
        let store = ConstructedElementStore::default();
        let missing = VolatilitySourceConfiguration::Surface {
            market_index: MarketIndex::SOFR,
            key: 0.03,
        };
        assert!(missing.resolve(&store).is_err());
    }

    #[test]
    fn configuration_serde_round_trip() -> Result<()> {
        let configs = [
            VolatilitySourceConfiguration::Constant { value: 0.2 },
            VolatilitySourceConfiguration::Surface {
                market_index: MarketIndex::SOFR,
                key: 0.03,
            },
            VolatilitySourceConfiguration::Cube {
                market_index: MarketIndex::SOFR,
                tenor: Period::new(5, TimeUnit::Years),
                key: 0.03,
            },
        ];
        for config in configs {
            let json = serde_json::to_string(&config)
                .map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
            let parsed: VolatilitySourceConfiguration =
                serde_json::from_str(&json).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
            let round_trip = serde_json::to_string(&parsed)
                .map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
            assert_eq!(json, round_trip);
        }
        Ok(())
    }

    #[test]
    fn piecewise_constant_volatility_lookup() -> Result<()> {
        let vol = PiecewiseConstantVolatility::new(vec![(0.0, 0.2_f64), (1.0, 0.3)])?;
        assert!((vol.vol(0.5)? - 0.2).abs() < 1e-15);
        assert!((vol.vol(1.0)? - 0.3).abs() < 1e-15);
        assert!((vol.vol(5.0)? - 0.3).abs() < 1e-15);
        assert!(PiecewiseConstantVolatility::<f64>::new(vec![]).is_err());
        assert!(PiecewiseConstantVolatility::new(vec![(1.0, 0.2_f64), (1.0, 0.3)]).is_err());
        Ok(())
    }

    fn term_surface_store_with_labels(
        vol_6m: f64,
        vol_1y: f64,
        labels: &[String],
    ) -> ConstructedElementStore {
        let smile = |v: f64| {
            BTreeMap::from([
                (F64Key::new(0.0), DualFwd::from(v)),
                (F64Key::new(0.10), DualFwd::from(v)),
            ])
        };
        let mut points = BTreeMap::new();
        points.insert(Period::new(6, TimeUnit::Months), smile(vol_6m));
        points.insert(Period::new(1, TimeUnit::Years), smile(vol_1y));
        points.insert(Period::new(2, TimeUnit::Years), smile(vol_1y));
        let surface = InterpolatedVolatilitySurface::new(
            Date::new(2025, 1, 1),
            MarketIndex::SOFR,
            points,
            VolatilityType::Black,
            SmileType::Strike,
        )
        .with_calibration_instrument_ids(labels);
        let mut store = ConstructedElementStore::default();
        store.volatility_surfaces_mut().insert(
            MarketIndex::SOFR,
            VolatilitySurfaceElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(surface))),
        );
        store
    }

    fn term_surface_store(vol_6m: f64, vol_1y: f64) -> ConstructedElementStore {
        let labels = vec![
            "CapletFloorlet_USD_SOFR_3M_6M_Absolute_0.045_Straddle_Black".to_string(),
            "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black".to_string(),
        ];
        term_surface_store_with_labels(vol_6m, vol_1y, &labels)
    }

    fn bootstrap_configuration() -> ModelCalibrationConfiguration {
        ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        })
    }

    #[test]
    fn bootstrap_flat_surface_gives_flat_forward_vol() -> Result<()> {
        let store = term_surface_store(0.20, 0.20);
        let vol = bootstrap_black_term_volatility(
            &bootstrap_configuration(),
            &store,
            Date::new(2025, 1, 1),
            DayCounter::Actual365,
        )?;
        assert!((vol.vol(0.25)? - 0.20).abs() < 1e-12);
        assert!((vol.vol(0.9)? - 0.20).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn bootstrap_reproduces_total_variance() -> Result<()> {
        let reference_date = Date::new(2025, 1, 1);
        let day_counter = DayCounter::Actual365;
        let store = term_surface_store(0.20, 0.25);
        let vol = bootstrap_black_term_volatility(
            &bootstrap_configuration(),
            &store,
            reference_date,
            day_counter,
        )?;

        let t1 = day_counter.year_fraction(
            reference_date,
            reference_date + Period::new(6, TimeUnit::Months),
        );
        let t2 = day_counter.year_fraction(
            reference_date,
            reference_date + Period::new(1, TimeUnit::Years),
        );

        // Cumulative variance from the piecewise forward vols must match the
        // implied total variance at each pillar.
        let sigma_1 = vol.vol(t1 / 2.0)?;
        let sigma_2 = vol.vol((t1 + t2) / 2.0)?;
        let var_t1 = sigma_1 * sigma_1 * t1;
        let var_t2 = var_t1 + sigma_2 * sigma_2 * (t2 - t1);
        assert!((var_t1 - 0.20 * 0.20 * t1).abs() < 1e-12);
        assert!((var_t2 - 0.25 * 0.25 * t2).abs() < 1e-12);
        assert!(sigma_2 > 0.25, "forward vol must exceed implied: {sigma_2}");
        Ok(())
    }

    #[test]
    fn bootstrap_rejects_negative_forward_variance() {
        let store = term_surface_store(0.30, 0.10);
        let result = bootstrap_black_term_volatility(
            &bootstrap_configuration(),
            &store,
            Date::new(2025, 1, 1),
            DayCounter::Actual365,
        );
        assert!(result.is_err());
    }

    #[test]
    fn bootstrap_strike_override_dedupes_pillars() -> Result<()> {
        // All available market quotes: several strikes per expiry.
        let all_ids: Vec<String> = ["6M", "1Y"]
            .iter()
            .flat_map(|expiry| {
                ["0.035", "0.045", "0.055"].iter().map(move |k| {
                    format!("CapletFloorlet_USD_SOFR_3M_{expiry}_Absolute_{k}_Straddle_Black")
                })
            })
            .collect();
        let store = term_surface_store_with_labels(0.20, 0.20, &all_ids);
        let source = CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        };

        // Without a strike override, duplicate expiries are ambiguous.
        let ambiguous = ModelCalibrationConfiguration::new(source.clone());
        assert!(bootstrap_black_term_volatility(
            &ambiguous,
            &store,
            Date::new(2025, 1, 1),
            DayCounter::Actual365,
        )
        .is_err());

        // An absolute strike override collapses to one pillar per expiry.
        let config =
            ModelCalibrationConfiguration::new(source.clone()).with_strike(Strike::Absolute(0.045));
        let vol = bootstrap_black_term_volatility(
            &config,
            &store,
            Date::new(2025, 1, 1),
            DayCounter::Actual365,
        )?;
        assert_eq!(vol.iter().count(), 2);
        assert!((vol.vol(0.25)? - 0.20).abs() < 1e-12);

        // ATM/relative moneyness cannot be resolved without a forward curve.
        let atm_config = ModelCalibrationConfiguration::new(source).with_strike(Strike::Atm);
        assert!(bootstrap_black_term_volatility(
            &atm_config,
            &store,
            Date::new(2025, 1, 1),
            DayCounter::Actual365,
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn bootstrap_rejects_normal_vol_quotes() {
        let ids = vec![
            "CapletFloorlet_USD_SOFR_3M_6M_Absolute_0.045_Straddle_Normal".to_string(),
            "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Normal".to_string(),
        ];
        let store = term_surface_store_with_labels(0.20, 0.20, &ids);
        let config = ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        });
        let result = bootstrap_black_term_volatility(
            &config,
            &store,
            Date::new(2025, 1, 1),
            DayCounter::Actual365,
        );
        assert!(
            result.is_err(),
            "Normal vol quotes must be rejected by the Black bootstrap"
        );
    }
}

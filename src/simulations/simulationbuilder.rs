//! Builds Monte Carlo simulations from [`SimulationConfiguration`]s.
//!
//! For each configuration the builder:
//! 1. constructs the simulation date grid (reference date → horizon),
//! 2. instantiates the configured model against the constructed market data
//!    (discount curves, volatility surfaces/cubes, fixings),
//! 3. resolves or calibrates the model volatility,
//! 4. generates the paths, and
//! 5. wraps them in a [`MonteCarloSimulationElement`] keyed by market index.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use rand::{rngs::StdRng, SeedableRng};

use crate::{
    core::{
        elements::montecarlosimulationelement::MonteCarloSimulationElement,
        marketdatahandling::constructedelementstore::ConstructedElementStore,
    },
    indices::marketindex::MarketIndex,
    math::random::fill_std_normal,
    models::{
        brownianmotion::BrownianMotion,
        hullwhite::hullwhitemodel::HullWhite,
        lgm::lgmcomponents::LgmRateModel,
        modelconfiguration::{ModelConfiguration, ParameterSource, SimulationConfiguration},
        montecarloengine::{PathGenerator, TimeDependentVolatility},
    },
    quotes::{fixingstore::FixingStore, quote::Level, quoteselector::QuoteSelector},
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    simulations::generatedsimulation::GeneratedMonteCarloSimulation,
    time::{date::Date, schedule::MakeSchedule},
    utils::errors::{QSError, Result},
    volatility::volatilitysource::{bootstrap_black_term_volatility, ConstantVolatility},
};

/// Builds [`MonteCarloSimulationElement`]s from serde-enabled
/// [`SimulationConfiguration`]s.
pub struct SimulationBuilder {
    specs: Vec<SimulationConfiguration>,
}

impl SimulationBuilder {
    /// Creates a new simulation builder.
    #[must_use]
    pub const fn new(specs: Vec<SimulationConfiguration>) -> Self {
        Self { specs }
    }

    /// Builds one simulation per configuration, keyed by market index.
    ///
    /// # Errors
    /// Returns an error if required market data (curves, surfaces, cubes,
    /// fixings) is missing, if a model/volatility-source combination is
    /// unsupported, or if path generation fails.
    pub fn build(
        &self,
        store: &ConstructedElementStore,
        selector: &dyn QuoteSelector,
        fixing_store: &FixingStore,
        level: Level,
    ) -> Result<HashMap<MarketIndex, MonteCarloSimulationElement>> {
        let mut simulations = HashMap::new();
        for spec in &self.specs {
            let element = Self::build_one(spec, store, selector, fixing_store, level)?;
            simulations.insert(spec.market_index().clone(), element);
        }
        Ok(simulations)
    }

    fn build_one(
        spec: &SimulationConfiguration,
        store: &ConstructedElementStore,
        selector: &dyn QuoteSelector,
        fixing_store: &FixingStore,
        level: Level,
    ) -> Result<MonteCarloSimulationElement> {
        let reference_date = selector.reference_date();
        let index = spec.market_index().clone();
        let day_counter = spec.day_counter();

        // Simulation date grid: strictly after the reference date.
        let end_date = reference_date + spec.horizon();
        let schedule = MakeSchedule::new(reference_date, end_date)
            .with_frequency(spec.frequency())
            .build()?;
        let dates: Vec<Date> = schedule
            .dates()
            .iter()
            .copied()
            .filter(|d| *d > reference_date)
            .collect();
        if dates.is_empty() {
            return Err(QSError::InvalidValueErr(format!(
                "Simulation for {index} has an empty date grid"
            )));
        }
        let times: Vec<f64> = dates
            .iter()
            .map(|d| day_counter.year_fraction(reference_date, *d))
            .collect();

        let curve_element = store.discount_curve(&index).ok_or_else(|| {
            QSError::NotFoundErr(format!("Discount curve not found for index {index}"))
        })?;
        let curve = curve_element.to_f64_term_structure(day_counter)?;

        let paths = match spec.model() {
            ModelConfiguration::HullWhite {
                alpha,
                parameter_source,
            } => {
                let mut hw = HullWhite::new(*alpha, &curve);
                match parameter_source {
                    ParameterSource::Fixed(fixed) => {
                        fixed.validate()?;
                        hw = hw.with_constant_volatility(fixed.sigma);
                    }
                    ParameterSource::Calibrated(configuration) => {
                        hw.calibrate_with_configuration(
                            configuration,
                            store,
                            selector,
                            &curve,
                            level,
                        )?;
                    }
                }
                generate_paths(&hw, &times, spec.n_paths(), spec.seed())?
            }
            ModelConfiguration::BrownianMotion {
                parameter_source,
                dividend_rate,
            } => {
                let spot = fixing_store.fixing(&index, reference_date)?;
                let t_end = times.last().copied().unwrap_or(1.0);
                let rate = -curve.discount_factor_from_time(t_end)?.ln() / t_end;
                let vol_func: Box<dyn TimeDependentVolatility<f64>> = match parameter_source {
                    ParameterSource::Fixed(fixed) => {
                        fixed.validate()?;
                        Box::new(ConstantVolatility::new(fixed.volatility))
                    }
                    ParameterSource::Calibrated(configuration) => {
                        Box::new(bootstrap_black_term_volatility(
                            configuration,
                            store,
                            reference_date,
                            day_counter,
                        )?)
                    }
                };
                let bm = BrownianMotion::new(spot, rate, vol_func, *dividend_rate);
                generate_paths(&bm, &times, spec.n_paths(), spec.seed())?
            }
            ModelConfiguration::Lgm { .. } => {
                let lgm =
                    LgmRateModel::from_configuration(spec.model(), &curve, store, selector, level)?;
                generate_paths(&lgm, &times, spec.n_paths(), spec.seed())?
            }
        };

        #[allow(clippy::cast_precision_loss)]
        let dt = times.last().copied().unwrap_or(0.0) / times.len() as f64;
        let simulation = GeneratedMonteCarloSimulation::new(index.clone(), dates, paths, dt);
        Ok(MonteCarloSimulationElement::new(
            index,
            Rc::new(RefCell::new(simulation)),
        ))
    }
}

/// Generates `n_paths` scenarios from the model using deterministic,
/// seed-driven standard-normal draws.
fn generate_paths(
    model: &dyn PathGenerator<f64>,
    times: &[f64],
    n_paths: usize,
    seed: u64,
) -> Result<Vec<Vec<f64>>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut paths = Vec::with_capacity(n_paths);
    let mut draws = vec![0.0_f64; times.len()];
    for _ in 0..n_paths {
        fill_std_normal(&mut rng, &mut draws);
        let mut scenario = vec![0.0_f64; times.len()];
        model.generate(times, &draws, &mut scenario)?;
        paths.push(scenario);
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

    use super::*;
    use crate::{
        ad::dual::DualFwd,
        core::elements::{
            curveelement::DiscountCurveElement, volatilitysurfaceelement::VolatilitySurfaceElement,
        },
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        time::{daycounter::DayCounter, enums::Frequency, enums::TimeUnit, period::Period},
        volatility::{
            interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
            modelcalibration::{CalibrationSource, ModelCalibrationConfiguration},
            volatilityindexing::{F64Key, SmileType, Strike, VolatilityType},
        },
    };
    use crate::{
        math::interpolation::interpolator::Interpolator,
        models::modelconfiguration::{GaussianRateModelParameters, LognormalModelParameters},
        quotes::quotestore::QuoteStore,
        time::date::Date,
    };

    fn reference_date() -> Date {
        Date::new(2025, 1, 2)
    }

    fn setup_store(index: &MarketIndex) -> Result<ConstructedElementStore> {
        let dates = vec![
            reference_date(),
            reference_date() + Period::new(1, TimeUnit::Years),
            reference_date() + Period::new(10, TimeUnit::Years),
        ];
        let rate = 0.03_f64;
        let dc = DayCounter::Actual365;
        let dfs: Vec<DualFwd> = dates
            .iter()
            .map(|d| DualFwd::new((-rate * dc.year_fraction(reference_date(), *d)).exp()))
            .collect();
        let curve =
            DiscountTermStructure::<DualFwd>::new(dates, dfs, dc, Interpolator::LogLinear, true)?;
        let mut store = ConstructedElementStore::default();
        store.discount_curves_mut().insert(
            index.clone(),
            DiscountCurveElement::new(index.clone(), Rc::new(RefCell::new(curve))),
        );
        Ok(store)
    }

    fn spec(index: &MarketIndex, model: ModelConfiguration) -> SimulationConfiguration {
        SimulationConfiguration::new(
            index.clone(),
            model,
            25,
            7,
            Period::new(2, TimeUnit::Years),
            Frequency::Monthly,
        )
    }

    fn add_flat_surface(store: &mut ConstructedElementStore, index: &MarketIndex, vol: f64) {
        let labels = vec![
            "EquityCall_USD_SPX_6M_Absolute_0.05".to_string(),
            "EquityCall_USD_SPX_1Y_Absolute_0.05".to_string(),
        ];
        let smile = BTreeMap::from([
            (F64Key::new(0.0), DualFwd::from(vol)),
            (F64Key::new(0.10), DualFwd::from(vol)),
        ]);
        let mut points = BTreeMap::new();
        points.insert(Period::new(1, TimeUnit::Months), smile.clone());
        points.insert(Period::new(2, TimeUnit::Years), smile);
        let surface = InterpolatedVolatilitySurface::new(
            reference_date(),
            index.clone(),
            points,
            VolatilityType::Black,
            SmileType::Strike,
        )
        .with_calibration_instrument_ids(&labels);
        store.volatility_surfaces_mut().insert(
            index.clone(),
            VolatilitySurfaceElement::new(index.clone(), Rc::new(RefCell::new(surface))),
        );
    }

    fn caplet_calibration(index: &MarketIndex) -> ModelCalibrationConfiguration {
        ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: index.clone(),
        })
    }

    #[test]
    fn builds_brownian_motion_simulation_with_constant_vol() -> Result<()> {
        let index = MarketIndex::Equity("SPX".to_string());
        let store = setup_store(&index)?;
        let quotes = QuoteStore::new(reference_date());
        let mut fixings = FixingStore::default();
        fixings.add_fixing(&index, reference_date(), 100.0);

        let builder = SimulationBuilder::new(vec![spec(
            &index,
            ModelConfiguration::BrownianMotion {
                parameter_source: ParameterSource::Fixed(LognormalModelParameters::new(0.2)),
                dividend_rate: None,
            },
        )]);
        let simulations = builder.build(&store, &quotes, &fixings, Level::Mid)?;
        let element = simulations
            .get(&index)
            .ok_or_else(|| QSError::NotFoundErr("simulation missing".into()))?;

        let simulation = element.simulation().borrow();
        assert_eq!(simulation.n_paths(), 25);
        assert_eq!(simulation.path().len(), 25);
        assert_eq!(simulation.dates().len(), 24);
        for path in simulation.path() {
            assert_eq!(path.len(), 24);
            for spot in path {
                assert!(spot.value() > 0.0, "GBM spot must stay positive");
            }
        }
        Ok(())
    }

    #[test]
    fn builds_hull_white_simulation_with_constant_vol() -> Result<()> {
        let index = MarketIndex::SOFR;
        let store = setup_store(&index)?;
        let quotes = QuoteStore::new(reference_date());
        let fixings = FixingStore::default();

        let builder = SimulationBuilder::new(vec![spec(
            &index,
            ModelConfiguration::HullWhite {
                alpha: 0.1,
                parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(0.01)),
            },
        )]);
        let simulations = builder.build(&store, &quotes, &fixings, Level::Mid)?;
        let element = simulations
            .get(&index)
            .ok_or_else(|| QSError::NotFoundErr("simulation missing".into()))?;
        let simulation = element.simulation().borrow();
        assert_eq!(simulation.n_paths(), 25);
        assert_eq!(simulation.path().len(), 25);
        assert!(simulation.dt() > 0.0);
        Ok(())
    }

    #[test]
    fn builds_brownian_motion_simulation_with_calibrated_vol() -> Result<()> {
        let index = MarketIndex::Equity("SPX".to_string());
        let mut store = setup_store(&index)?;
        add_flat_surface(&mut store, &index, 0.2);
        let quotes = QuoteStore::new(reference_date());
        let mut fixings = FixingStore::default();
        fixings.add_fixing(&index, reference_date(), 100.0);

        let builder = SimulationBuilder::new(vec![spec(
            &index,
            ModelConfiguration::BrownianMotion {
                parameter_source: ParameterSource::Calibrated(caplet_calibration(&index)),
                dividend_rate: None,
            },
        )]);
        let simulations = builder.build(&store, &quotes, &fixings, Level::Mid)?;
        let element = simulations
            .get(&index)
            .ok_or_else(|| QSError::NotFoundErr("simulation missing".into()))?;
        let simulation = element.simulation().borrow();
        assert_eq!(simulation.n_paths(), 25);
        for path in simulation.path() {
            for spot in path {
                assert!(spot.value() > 0.0, "GBM spot must stay positive");
            }
        }
        Ok(())
    }

    #[test]
    fn builds_lgm_simulation_with_constant_vol() -> Result<()> {
        let index = MarketIndex::SOFR;
        let store = setup_store(&index)?;
        let quotes = QuoteStore::new(reference_date());
        let fixings = FixingStore::default();

        let builder = SimulationBuilder::new(vec![spec(
            &index,
            ModelConfiguration::Lgm {
                lambda: 0.05,
                parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(0.01)),
            },
        )]);
        let simulations = builder.build(&store, &quotes, &fixings, Level::Mid)?;
        let element = simulations
            .get(&index)
            .ok_or_else(|| QSError::NotFoundErr("simulation missing".into()))?;
        let simulation = element.simulation().borrow();
        assert_eq!(simulation.n_paths(), 25);
        for path in simulation.path() {
            for rate in path {
                assert!(
                    (rate.value() - 0.03).abs() < 0.05,
                    "short rate {} far from curve level",
                    rate.value()
                );
            }
        }
        Ok(())
    }

    #[test]
    fn calibrated_asset_model_requires_constructed_volatility_market() -> Result<()> {
        let index = MarketIndex::SOFR;
        let store = setup_store(&index)?;
        let quotes = QuoteStore::new(reference_date());
        let fixings = FixingStore::default();

        // Brownian motion calibration requires the surface to be constructed.
        let bm_calibrated = SimulationBuilder::new(vec![spec(
            &index,
            ModelConfiguration::BrownianMotion {
                parameter_source: ParameterSource::Calibrated(caplet_calibration(&index)),
                dividend_rate: None,
            },
        )]);
        assert!(bm_calibrated
            .build(&store, &quotes, &fixings, Level::Mid)
            .is_err());

        Ok(())
    }

    #[test]
    fn simulation_configuration_serde_round_trip() -> Result<()> {
        let json = r#"{
            "market_index": "SOFR",
            "model": {
                "HullWhite": {
                    "alpha": 0.1,
                    "parameter_source": { "Fixed": { "sigma": 0.01 } }
                }
            },
            "horizon": "5Y"
        }"#;
        let parsed: SimulationConfiguration =
            serde_json::from_str(json).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
        assert_eq!(parsed.n_paths(), 1000);
        assert_eq!(parsed.seed(), 42);
        assert_eq!(parsed.market_index(), &MarketIndex::SOFR);
        let round =
            serde_json::to_string(&parsed).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
        let reparsed: SimulationConfiguration =
            serde_json::from_str(&round).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
        assert_eq!(reparsed.horizon(), parsed.horizon());
        Ok(())
    }

    #[test]
    fn calibrated_configuration_uses_explicit_section_names() -> Result<()> {
        let json = r#"{
            "market_index": "SOFR",
            "model": {
                "HullWhite": {
                    "alpha": 0.1,
                    "parameter_source": {
                        "Calibrated": {
                            "source": { "Surface": { "market_index": "SOFR" } },
                            "calibration_basket": {
                                "expiries": ["1Y", "2Y"],
                                "tenors": ["3M"],
                                "strike": "Atm"
                            }
                        }
                    }
                }
            },
            "horizon": "5Y"
        }"#;
        let parsed: SimulationConfiguration =
            serde_json::from_str(json).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
        let ModelConfiguration::HullWhite {
            parameter_source: ParameterSource::Calibrated(calibration),
            ..
        } = parsed.model()
        else {
            return Err(QSError::UnexpectedErr(
                "expected calibrated Hull-White configuration".into(),
            ));
        };
        assert_eq!(
            calibration
                .calibration_basket()
                .expiries()
                .map(|values| values.len()),
            Some(2)
        );
        assert_eq!(
            calibration
                .calibration_basket()
                .tenors()
                .map(|values| values.len()),
            Some(1)
        );
        assert_eq!(calibration.calibration_basket().strike(), Some(Strike::Atm));

        let ambiguous_name = json.replace("calibration_basket", "basket");
        assert!(serde_json::from_str::<SimulationConfiguration>(&ambiguous_name).is_err());
        let ambiguous_source = json.replace("parameter_source", "parameters");
        assert!(serde_json::from_str::<SimulationConfiguration>(&ambiguous_source).is_err());
        Ok(())
    }
}

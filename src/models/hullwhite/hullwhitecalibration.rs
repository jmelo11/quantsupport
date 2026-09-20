use crate::{
    ad::scalar::Scalar,
    calibration::{
        calibrationpricer::CalibrationInstrumentPricer, calibrationprocess::CalibrationProcess,
    },
    core::marketdatahandling::constructedelementstore::ConstructedElementStore,
    math::solvers::{bisection::Bisection, solvertraits::ContFunc},
    models::{
        montecarloengine::TimeDependentVolatility,
        utils::{bachelier_call, black_call, swap_annuity_from_curve},
    },
    quotes::{
        calibrationinstrument::CalibrationInstrument,
        quote::{CalibrationInstrumentType, Level, Quote},
        quoteselector::QuoteSelector,
    },
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
    utils::errors::{QSError, Result},
    volatility::{
        modelcalibration::{CalibrationSource, ModelCalibrationConfiguration},
        volatilityindexing::{Strike, VolatilityType},
    },
};

use super::{
    hullwhitecalibrationquality::{HullWhiteCalibrationQuality, HullWhiteCalibrationRecord},
    hullwhitemodel::HullWhite,
};

/// Piecewise-constant time-dependent volatility for the Hull-White model.
#[derive(Clone, Default)]
pub struct HullWhiteTimeDependentVolatility<T: Scalar> {
    schedule: Vec<(f64, T)>,
    pillar_labels: Option<Vec<String>>,
    ift_sensitivities: Option<Vec<Vec<f64>>>,
}

impl HullWhiteTimeDependentVolatility<f64> {
    /// Creates a new time-dependent volatility function from a schedule of
    /// `(year_fraction, sigma)` pairs.
    #[must_use]
    pub const fn new(schedule: Vec<(f64, f64)>) -> Self {
        Self {
            schedule,
            pillar_labels: None,
            ift_sensitivities: None,
        }
    }

    /// Attaches pillar labels (vol quote identifiers used during calibration).
    #[must_use]
    pub fn with_pillar_labels(mut self, labels: Vec<String>) -> Self {
        self.pillar_labels = Some(labels);
        self
    }

    /// Attaches the IFT sensitivity matrix `d(sigma_HW_i) / d(vol_quote_j)`.
    #[must_use]
    pub fn with_ift_sensitivities(mut self, sens: Vec<Vec<f64>>) -> Self {
        self.ift_sensitivities = Some(sens);
        self
    }

    /// Returns the IFT sensitivity matrix, if present.
    #[must_use]
    pub const fn ift_sensitivities(&self) -> Option<&Vec<Vec<f64>>> {
        self.ift_sensitivities.as_ref()
    }

    /// Returns the number of schedule entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.schedule.len()
    }

    /// Returns true if the schedule is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.schedule.is_empty()
    }

    /// Iterates over `(year_fraction, sigma)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = &(f64, f64)> {
        self.schedule.iter()
    }
}

impl TimeDependentVolatility<f64> for HullWhiteTimeDependentVolatility<f64> {
    fn vol(&self, t: f64) -> Result<f64> {
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

/// Pricer for HW calibration: holds the HW model, a trial sigma, the
/// discount curve, and date/day-count context.  `price()` returns the
/// HW model price for a given instrument at the current sigma.
struct HullWhiteCalibration<'a, 'b> {
    hw: &'a HullWhite<'b, f64>,
    sigma: f64,
    reference_date: Date,
    day_counter: DayCounter,
    curve: &'a dyn InterestRatesTermStructure<f64>,
}

impl HullWhiteCalibration<'_, '_> {
    /// Computes the market price from a quoted vol for the given calibration
    /// instrument, honouring the quote's volatility convention: Black
    /// (lognormal) vols are priced with Black's formula, Normal vols with
    /// Bachelier's. This is the target that the model price must match, so
    /// the calibrated sigma is correct for either quoting convention.
    fn market_price(&self, instrument: &CalibrationInstrument, market_vol: f64) -> Result<f64> {
        let vol_type = instrument
            .quote()
            .details()
            .vol_type()
            .cloned()
            .unwrap_or(VolatilityType::Black);
        let vanilla_call = |fwd: f64, strike: f64, vol: f64, tau: f64| match vol_type {
            VolatilityType::Black => black_call(fwd, strike, vol, tau),
            VolatilityType::Normal => bachelier_call(fwd, strike, vol, tau),
        };

        // CapFloor: flat-vol price = sum of individual caplet prices.
        if let CalibrationInstrumentType::CapFloor(cf) = instrument.built() {
            let strike = cf.strike();
            let mut total = 0.0;
            for cfl in cf.caplet_floorlets() {
                let t = self
                    .day_counter
                    .year_fraction(self.reference_date, cfl.start_accrual_date());
                if t <= 0.0 {
                    continue;
                }
                let big_t = self
                    .day_counter
                    .year_fraction(self.reference_date, cfl.end_accrual_date());
                let tau = big_t - t;
                let df_start = self.curve.discount_factor_from_time(t)?;
                let df_end = self.curve.discount_factor_from_time(big_t)?;
                let fwd = (df_start / df_end - 1.0) / tau;
                total = (df_end * tau).mul_add(
                    vanilla_call(fwd, strike.resolve(fwd), market_vol, t)?,
                    total,
                );
            }
            return Ok(total);
        }

        let (t, big_t, fwd, strike, annuity) = extract_calibration_params(
            instrument,
            self.reference_date,
            self.day_counter,
            self.curve,
        )?;
        match instrument.built() {
            CalibrationInstrumentType::CapletFloorlet(_) => {
                let tau = big_t - t;
                let df_end = self.curve.discount_factor_from_time(big_t)?;
                Ok(df_end * tau * vanilla_call(fwd, strike, market_vol, t)?)
            }
            CalibrationInstrumentType::EuropeanSwaption(_) => {
                Ok(annuity * vanilla_call(fwd, strike, market_vol, t)?)
            }
            _ => Err(QSError::InvalidValueErr(
                "market_price: unsupported instrument type".into(),
            )),
        }
    }
}

impl CalibrationInstrumentPricer for HullWhiteCalibration<'_, '_> {
    /// Returns the HW model price for the given calibration instrument at the
    /// current trial sigma, using the model's bond-option representation
    /// (caplets) or Jamshidian decomposition (swaptions). This keeps the
    /// calibrated sigma in genuine short-rate volatility units, consistent
    /// with [`HullWhite`]'s path generation and the LGM Gaussian factor.
    fn price(&self, instrument: &CalibrationInstrument) -> Result<f64> {
        // CapFloor: HW model price = sum of individual caplet HW prices.
        if let CalibrationInstrumentType::CapFloor(cf) = instrument.built() {
            let strike = cf.strike();
            let mut total = 0.0;
            for cfl in cf.caplet_floorlets() {
                let t = self
                    .day_counter
                    .year_fraction(self.reference_date, cfl.start_accrual_date());
                if t <= 0.0 {
                    continue;
                }
                let big_t = self
                    .day_counter
                    .year_fraction(self.reference_date, cfl.end_accrual_date());
                let tau = big_t - t;
                let df_start = self.curve.discount_factor_from_time(t)?;
                let df_end = self.curve.discount_factor_from_time(big_t)?;
                let fwd = (df_start / df_end - 1.0) / tau;
                total +=
                    self.hw
                        .caplet_price(strike.resolve(fwd), t, big_t, self.sigma, self.curve)?;
            }
            return Ok(total);
        }

        let (t, big_t, _fwd, strike, _annuity) = extract_calibration_params(
            instrument,
            self.reference_date,
            self.day_counter,
            self.curve,
        )?;

        match instrument.built() {
            CalibrationInstrumentType::CapletFloorlet(_) => self
                .hw
                .caplet_price(strike, t, big_t, self.sigma, self.curve),
            CalibrationInstrumentType::EuropeanSwaption(_) => {
                let details = instrument.quote().details();
                let option_expiry = details.option_expiry().ok_or_else(|| {
                    QSError::InvalidValueErr("EuropeanSwaption: missing option_expiry".into())
                })?;
                let swap_tenor = details.tenor().ok_or_else(|| {
                    QSError::InvalidValueErr("EuropeanSwaption: missing swap tenor".into())
                })?;
                let exp_date = self.reference_date + option_expiry;
                let swap_end = exp_date + swap_tenor;
                let schedule =
                    annual_swap_schedule(self.reference_date, exp_date, swap_end, self.day_counter);
                self.hw
                    .swaption_price(strike, t, &schedule, self.sigma, self.curve)
            }
            _ => Err(QSError::InvalidValueErr(format!(
                "HW pricer: unsupported instrument type {:?}",
                instrument.built()
            ))),
        }
    }

    fn sensitivity(&self, instrument: &CalibrationInstrument) -> Result<f64> {
        let eps = 1e-6;
        let up = HullWhiteCalibration {
            hw: self.hw,
            sigma: self.sigma + eps,
            reference_date: self.reference_date,
            day_counter: self.day_counter,
            curve: self.curve,
        };
        Ok((up.price(instrument)? - self.price(instrument)?) / eps)
    }
}

impl CalibrationProcess for HullWhiteCalibration<'_, '_> {
    fn residual(&self, instruments: &[CalibrationInstrument]) -> Result<Vec<f64>> {
        instruments
            .iter()
            .map(|inst| {
                let model = self.price(inst)?;
                let market = self.market_price(inst, inst.quote_value())?;
                Ok(model - market)
            })
            .collect()
    }
}

/// Extracts `(t, big_t, forward, effective_strike, annuity)` from a
/// calibration instrument.  `annuity` is only meaningful for swaptions.
fn extract_calibration_params(
    ci: &CalibrationInstrument,
    reference_date: Date,
    day_counter: DayCounter,
    curve: &dyn InterestRatesTermStructure<f64>,
) -> Result<(f64, f64, f64, f64, f64)> {
    let details = ci.quote().details();
    match ci.built() {
        CalibrationInstrumentType::CapletFloorlet(cfl) => {
            let t = day_counter.year_fraction(reference_date, cfl.start_accrual_date());
            let big_t = day_counter.year_fraction(reference_date, cfl.end_accrual_date());
            let tau = big_t - t;
            let df_start = curve.discount_factor_from_time(t)?;
            let df_end = curve.discount_factor_from_time(big_t)?;
            let fwd = (df_start / df_end - 1.0) / tau;
            let effective_strike = details.strike().unwrap_or(Strike::Atm).resolve(fwd);
            Ok((t, big_t, fwd, effective_strike, 0.0))
        }
        CalibrationInstrumentType::EuropeanSwaption(_) => {
            let option_expiry = details.option_expiry().ok_or_else(|| {
                QSError::InvalidValueErr("EuropeanSwaption: missing option_expiry".into())
            })?;
            let swap_tenor = details.tenor().ok_or_else(|| {
                QSError::InvalidValueErr("EuropeanSwaption: missing swap tenor".into())
            })?;
            let exp_date = reference_date + option_expiry;
            let swap_end = exp_date + swap_tenor;
            let t = day_counter.year_fraction(reference_date, exp_date);
            let big_t = day_counter.year_fraction(reference_date, swap_end);

            let annuity =
                swap_annuity_from_curve(curve, reference_date, exp_date, swap_end, day_counter)?;
            let df_start = curve.discount_factor_from_time(t)?;
            let df_end = curve.discount_factor_from_time(big_t)?;
            let fwd_swap = (df_start - df_end) / annuity;
            let effective_strike = details.strike().unwrap_or(Strike::Atm).resolve(fwd_swap);
            Ok((t, big_t, fwd_swap, effective_strike, annuity))
        }
        other => Err(QSError::InvalidValueErr(format!(
            "extract_calibration_params: unsupported instrument type {other:?}"
        ))),
    }
}

/// Builds an annual `(payment_time, accrual_fraction)` swap schedule from
/// `start` to `end`, consistent with [`swap_annuity_from_curve`].
fn annual_swap_schedule(
    reference_date: Date,
    start: Date,
    end: Date,
    day_counter: DayCounter,
) -> Vec<(f64, f64)> {
    let mut schedule = Vec::new();
    let mut date = start;
    let one_year = Period::new(1, TimeUnit::Years);
    while date < end {
        let next = std::cmp::min(date + one_year, end);
        let t = day_counter.year_fraction(reference_date, next);
        let tau = day_counter.year_fraction(date, next);
        schedule.push((t, tau));
        date = next;
    }
    schedule
}

/// Objective for HW calibration: f(sigma) = `model_price(sigma_p)` - `market_price`.
struct HwCalibrationObjective<'a, 'b> {
    hw: &'a HullWhite<'b, f64>,
    instrument: &'a CalibrationInstrument,
    market_vol: f64,
    reference_date: Date,
    day_counter: DayCounter,
    curve: &'a dyn InterestRatesTermStructure<f64>,
}

impl ContFunc<f64> for HwCalibrationObjective<'_, '_> {
    fn call(&self, sigma: &f64) -> Result<f64> {
        let pricer = HullWhiteCalibration {
            hw: self.hw,
            sigma: *sigma,
            reference_date: self.reference_date,
            day_counter: self.day_counter,
            curve: self.curve,
        };
        let model = pricer.price(self.instrument)?;
        let market = pricer.market_price(self.instrument, self.market_vol)?;
        Ok(model - market)
    }
}

/// Builds calibration instruments from quote identifiers, using the raw quote
/// value (a market vol) as the calibration target.
///
/// When `strike_override` is provided, quotes are collapsed to one instrument
/// per (option expiry, tenor) pillar and each instrument's strike is replaced
/// by the override (resolved against the pillar forward downstream). This lets
/// callers pass all available market quotes and select moneyness separately.
/// Without an override, each expiry-tenor pillar must identify a single smile
/// node.
/// Instruments are returned sorted by pillar date.
fn build_calibration_instruments(
    quote_ids: &[String],
    selector: &dyn QuoteSelector,
    level: Level,
    reference_date: Date,
    strike_override: Option<Strike>,
) -> Result<Vec<CalibrationInstrument>> {
    let mut seen_pillars: Vec<(Option<Period>, Option<Period>)> = Vec::new();
    let mut cal_instruments = Vec::with_capacity(quote_ids.len());
    for id in quote_ids {
        let quote = selector
            .select(id)
            .ok_or_else(|| QSError::NotFoundErr(format!("Calibration quote not found: {id}")))?;
        let quote = strike_override.map_or_else(
            || quote.clone(),
            |k| Quote::new(quote.details().clone().with_strike(k), *quote.levels()),
        );
        let pillar = (quote.details().option_expiry(), quote.details().tenor());
        if seen_pillars.contains(&pillar) {
            if strike_override.is_some() {
                continue;
            }
            return Err(QSError::InvalidValueErr(format!(
                "Calibration selected multiple smile nodes for expiry {:?} and tenor {:?}. Set calibration_basket.strike to choose one strike rule per pillar",
                pillar.0, pillar.1
            )));
        }
        seen_pillars.push(pillar);
        let mkt_vol = quote.levels().value(level)?;
        let built = quote.build_instrument(reference_date, level, None)?;
        let pillar_date = built.pillar_date()?;
        cal_instruments.push(CalibrationInstrument::new(
            quote,
            level,
            built,
            mkt_vol,
            pillar_date,
        ));
    }
    cal_instruments.sort_by_key(CalibrationInstrument::pillar_date);
    Ok(cal_instruments)
}

impl HullWhite<'_, f64> {
    #[cfg(test)]
    pub(crate) fn calibrate(
        &mut self,
        quote_ids: &[String],
        selector: &dyn QuoteSelector,
        curve: &dyn InterestRatesTermStructure<f64>,
        level: Level,
    ) -> Result<()> {
        let reference_date = selector.reference_date();
        let cal_instruments =
            build_calibration_instruments(quote_ids, selector, level, reference_date, None)?;
        self.calibrate_to_instruments(&cal_instruments, reference_date, curve)
    }

    /// Calibrates the short-rate volatility sigma(t) to market vols read from
    /// a constructed volatility surface or cube, as specified by the
    /// configuration's [`CalibrationSource`].
    ///
    /// The surface or cube provides the available option instrument
    /// identifiers. The calibration basket selects a subset of those
    /// instruments, and the calibrator samples their market volatilities from
    /// the same constructed object used by option pricers.
    ///
    /// A common [`strike`](ModelCalibrationConfiguration::strike) rule groups
    /// the selected smile nodes into one instrument for each expiry and tenor.
    /// The calibrator resolves ATM and relative strikes against the pillar
    /// forward from `curve`, then samples the surface or cube at the resulting
    /// strike. When the rule is omitted, every selected expiry-tenor pair must
    /// contain a single smile node.
    ///
    /// # Errors
    /// Returns an error when the surface or cube is unavailable, calibration
    /// instruments are missing, several smile nodes remain at one pillar, or
    /// the curve data is invalid.
    pub fn calibrate_with_configuration(
        &mut self,
        configuration: &ModelCalibrationConfiguration,
        store: &ConstructedElementStore,
        selector: &dyn QuoteSelector,
        curve: &dyn InterestRatesTermStructure<f64>,
        level: Level,
    ) -> Result<()> {
        let reference_date = selector.reference_date();
        let day_counter = curve
            .day_counter()
            .ok_or_else(|| QSError::InvalidValueErr("Curve has no day counter".to_string()))?;
        let quote_ids = configuration.resolve_instrument_ids(store)?;
        let mut cal_instruments = build_calibration_instruments(
            &quote_ids,
            selector,
            level,
            reference_date,
            configuration.strike(),
        )?;

        // Override each instrument's market vol with the value interpolated
        // from the configured surface/cube at the instrument's expiry (and
        // tenor, for cubes) and effective strike. The surface/cube quoting
        // convention (Black or Normal) must match the instrument's, since
        // market_price prices the target according to that convention.
        for ci in &mut cal_instruments {
            let instrument_vol_type = ci
                .quote()
                .details()
                .vol_type()
                .cloned()
                .unwrap_or(VolatilityType::Black);
            let check_vol_type = |source_type: VolatilityType| {
                if source_type == instrument_vol_type {
                    Ok(())
                } else {
                    Err(QSError::InvalidValueErr(format!(
                        "Volatility convention mismatch for {}: quote is {instrument_vol_type:?} \
                         but the calibration source is {source_type:?}",
                        ci.pillar_label()
                    )))
                }
            };
            let (_t, _big_t, _fwd, effective_strike, _annuity) =
                extract_calibration_params(ci, reference_date, day_counter, curve)?;
            let expiry = ci.quote().details().option_expiry().ok_or_else(|| {
                QSError::InvalidValueErr(format!(
                    "Calibration instrument {} has no option expiry",
                    ci.pillar_label()
                ))
            })?;
            let vol = match configuration.source() {
                CalibrationSource::Surface { market_index } => {
                    let element = store.volatility_surface(market_index).ok_or_else(|| {
                        QSError::NotFoundErr(format!(
                            "Volatility surface not found for index {market_index}"
                        ))
                    })?;
                    let surface = element.surface();
                    check_vol_type(surface.volatility_type())?;
                    surface
                        .volatility_from_period(expiry, effective_strike)?
                        .value()
                }
                CalibrationSource::Cube { market_index } => {
                    let tenor = ci.quote().details().tenor().ok_or_else(|| {
                        QSError::InvalidValueErr(format!(
                            "Calibration instrument {} has no tenor (required for cube lookup)",
                            ci.pillar_label()
                        ))
                    })?;
                    let element = store.volatility_cube(market_index).ok_or_else(|| {
                        QSError::NotFoundErr(format!(
                            "Volatility cube not found for index {market_index}"
                        ))
                    })?;
                    let cube = element.cube();
                    check_vol_type(cube.volatility_type())?;
                    cube.volatility_from_period(expiry, tenor, effective_strike)?
                        .value()
                }
            };
            ci.set_quote_value(vol);
        }

        self.calibrate_to_instruments(&cal_instruments, reference_date, curve)
    }

    /// Core calibration routine: bootstraps a piecewise-constant sigma(t)
    /// schedule by solving, for each instrument, the sigma that reprices the
    /// instrument's market vol.
    fn calibrate_to_instruments(
        &mut self,
        cal_instruments: &[CalibrationInstrument],
        reference_date: Date,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<()> {
        let day_counter = curve
            .day_counter()
            .ok_or_else(|| QSError::InvalidValueErr("Curve has no day counter".to_string()))?;
        let n = cal_instruments.len();

        let mut schedule = Vec::with_capacity(n);
        let mut labels = Vec::with_capacity(n);
        let mut sigma_values = Vec::with_capacity(n);
        let mut market_vols = Vec::with_capacity(n);
        let mut records = Vec::with_capacity(n);

        for ci in cal_instruments {
            let mkt_vol = ci.quote_value();
            let id = ci.pillar_label();

            let (t, big_t, fwd, effective_strike, _annuity) =
                extract_calibration_params(ci, reference_date, day_counter, curve)?;

            let objective = HwCalibrationObjective {
                hw: self,
                instrument: ci,
                market_vol: mkt_vol,
                reference_date,
                day_counter,
                curve,
            };
            let solver = Bisection::<HwCalibrationObjective>::new(1e-8, 2.0, 200);
            let solution = solver.solve(&objective)?;
            let calibrated_sigma = solution.x;

            let pricer = HullWhiteCalibration {
                hw: self,
                sigma: calibrated_sigma,
                reference_date,
                day_counter,
                curve,
            };
            let model_price = pricer.price(ci)?;
            let market_price = pricer.market_price(ci, mkt_vol)?;

            let expiry_period = ci
                .quote()
                .details()
                .option_expiry()
                .unwrap_or(Period::new(0, TimeUnit::Days));

            records.push(HullWhiteCalibrationRecord {
                identifier: id.clone(),
                expiry: expiry_period,
                t,
                big_t,
                market_vol: mkt_vol,
                market_price,
                model_price,
                calibrated_sigma,
                forward_rate: fwd,
                effective_strike,
            });

            schedule.push((t, calibrated_sigma));
            sigma_values.push(calibrated_sigma);
            market_vols.push(mkt_vol);
            labels.push(id);
        }

        // IFT sensitivity matrix: d(sigma_HW_i) / d(vol_quote_j).
        let eps = 1e-6;
        let mut ift_matrix = vec![vec![0.0; n]; n];

        for j in 0..n {
            let bumped_vol = market_vols[j] + eps;
            let objective = HwCalibrationObjective {
                hw: self,
                instrument: &cal_instruments[j],
                market_vol: bumped_vol,
                reference_date,
                day_counter,
                curve,
            };
            let solver = Bisection::<HwCalibrationObjective>::new(1e-8, 2.0, 200);
            let bumped_sigma = solver.solve(&objective)?.x;
            ift_matrix[j][j] = (bumped_sigma - sigma_values[j]) / eps;
        }

        let result = HullWhiteTimeDependentVolatility::new(schedule)
            .with_pillar_labels(labels)
            .with_ift_sensitivities(ift_matrix);

        let quality = HullWhiteCalibrationQuality { records };
        self.vol_func = Some(result);
        self.calibration_quality = Some(quality);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc, str::FromStr};

    use super::*;
    use crate::{
        ad::dual::DualFwd,
        core::{
            elements::volatilitysurfaceelement::VolatilitySurfaceElement,
            marketdatahandling::constructedelementstore::ConstructedElementStore,
        },
        indices::marketindex::MarketIndex,
        math::interpolation::interpolator::Interpolator,
        models::hullwhite::hullwhitemodel::HullWhite,
        quotes::{
            quote::{Quote, QuoteDetails, QuoteLevels},
            quotestore::QuoteStore,
        },
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        volatility::{
            interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
            modelcalibration::{CalibrationBasket, CalibrationSource},
            volatilityindexing::{F64Key, SmileType, VolatilityType},
        },
    };

    const QUOTE_IDS: [&str; 2] = [
        "CapletFloorlet_USD_SOFR_3M_6M_Absolute_0.045_Straddle_Black",
        "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black",
    ];
    const MARKET_VOL: f64 = 0.20;

    fn flat_surface_store(reference_date: Date, labels: &[String]) -> ConstructedElementStore {
        let smile = BTreeMap::from([
            (F64Key::new(0.0), DualFwd::from(MARKET_VOL)),
            (F64Key::new(0.10), DualFwd::from(MARKET_VOL)),
        ]);
        let mut points = BTreeMap::new();
        points.insert(Period::new(1, TimeUnit::Months), smile.clone());
        points.insert(Period::new(2, TimeUnit::Years), smile);
        let surface = InterpolatedVolatilitySurface::new(
            reference_date,
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

    fn setup() -> Result<(
        QuoteStore,
        DiscountTermStructure<f64>,
        ConstructedElementStore,
    )> {
        let reference_date = Date::new(2025, 1, 2);

        let mut quote_store = QuoteStore::new(reference_date);
        for id in QUOTE_IDS {
            let details = QuoteDetails::from_str(id)?;
            quote_store.add_quote(Quote::new(details, QuoteLevels::with_mid(MARKET_VOL)));
        }

        let rate = 0.045_f64;
        let dc = DayCounter::Actual365;
        let dates = vec![
            reference_date,
            reference_date + Period::new(1, TimeUnit::Years),
            reference_date + Period::new(10, TimeUnit::Years),
        ];
        let dfs: Vec<f64> = dates
            .iter()
            .map(|d| (-rate * dc.year_fraction(reference_date, *d)).exp())
            .collect();
        let curve =
            DiscountTermStructure::<f64>::new(dates, dfs, dc, Interpolator::LogLinear, true)?;

        let labels = QUOTE_IDS
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let store = flat_surface_store(reference_date, &labels);

        Ok((quote_store, curve, store))
    }

    #[test]
    fn calibrate_with_flat_surface_matches_quote_calibration() -> Result<()> {
        let (quote_store, curve, store) = setup()?;
        let quote_ids: Vec<String> = QUOTE_IDS.iter().map(ToString::to_string).collect();

        let mut hw_quotes = HullWhite::new(0.1, &curve);
        hw_quotes.calibrate(&quote_ids, &quote_store, &curve, Level::Mid)?;
        let schedule_quotes: Vec<(f64, f64)> = hw_quotes
            .vol_func()
            .ok_or_else(|| QSError::UnexpectedErr("no vol func".into()))?
            .iter()
            .copied()
            .collect();

        let configuration = ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        });
        let mut hw_surface = HullWhite::new(0.1, &curve);
        hw_surface.calibrate_with_configuration(
            &configuration,
            &store,
            &quote_store,
            &curve,
            Level::Mid,
        )?;
        let schedule_surface: Vec<(f64, f64)> = hw_surface
            .vol_func()
            .ok_or_else(|| QSError::UnexpectedErr("no vol func".into()))?
            .iter()
            .copied()
            .collect();

        assert_eq!(schedule_quotes.len(), schedule_surface.len());
        assert_eq!(schedule_quotes.len(), 2);
        for ((t_q, s_q), (t_s, s_s)) in schedule_quotes.iter().zip(&schedule_surface) {
            assert!((t_q - t_s).abs() < 1e-12);
            assert!(
                (s_q - s_s).abs() < 1e-10,
                "sigma from surface {s_s} should match sigma from quotes {s_q}"
            );
            assert!(*s_q > 0.0);
        }
        Ok(())
    }

    #[test]
    fn calibrate_with_configuration_errors_without_surface() -> Result<()> {
        let (quote_store, curve, _) = setup()?;
        let configuration = ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        });
        let empty_store = ConstructedElementStore::default();
        let mut hw = HullWhite::new(0.1, &curve);
        let result = hw.calibrate_with_configuration(
            &configuration,
            &empty_store,
            &quote_store,
            &curve,
            Level::Mid,
        );
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn strike_override_dedupes_pillars_and_resolves_moneyness() -> Result<()> {
        let (mut quote_store, curve, _) = setup()?;

        // Pass ALL available market quotes: 3 strikes per expiry, shuffled order.
        let all_ids: Vec<String> = ["1Y", "6M"]
            .iter()
            .flat_map(|expiry| {
                ["0.035", "0.045", "0.055"].iter().map(move |k| {
                    format!("CapletFloorlet_USD_SOFR_3M_{expiry}_Absolute_{k}_Straddle_Black")
                })
            })
            .collect();
        for id in &all_ids {
            let details = QuoteDetails::from_str(id)?;
            quote_store.add_quote(Quote::new(details, QuoteLevels::with_mid(MARKET_VOL)));
        }
        let store = flat_surface_store(Date::new(2025, 1, 2), &all_ids);

        let ambiguous = ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        });
        let mut ambiguous_hw = HullWhite::new(0.1, &curve);
        assert!(
            ambiguous_hw
                .calibrate_with_configuration(&ambiguous, &store, &quote_store, &curve, Level::Mid,)
                .is_err(),
            "a smile requires an explicit calibration strike rule"
        );

        // ATM moneyness: the system collapses to one pillar per expiry and
        // resolves the strike from the curve forward.
        let configuration = ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        })
        .with_strike(Strike::Atm);
        let mut hw = HullWhite::new(0.1, &curve);
        hw.calibrate_with_configuration(&configuration, &store, &quote_store, &curve, Level::Mid)?;

        let schedule: Vec<(f64, f64)> = hw
            .vol_func()
            .ok_or_else(|| QSError::UnexpectedErr("no vol func".into()))?
            .iter()
            .copied()
            .collect();
        assert_eq!(schedule.len(), 2, "6 quotes should collapse to 2 pillars");
        assert!(schedule[0].0 < schedule[1].0, "pillars must be ascending");
        for (_, sigma) in &schedule {
            assert!(*sigma > 0.0 && *sigma < 0.05, "sigma {sigma} not sane");
        }

        // ATM resolution: effective strike equals the pillar forward.
        let quality = hw
            .calibration_quality()
            .ok_or_else(|| QSError::UnexpectedErr("no quality".into()))?;
        for rec in &quality.records {
            assert!(
                (rec.effective_strike - rec.forward_rate).abs() < 1e-14,
                "ATM strike should resolve to the forward"
            );
        }

        // Absolute moneyness override matches an explicit per-quote calibration.
        let abs_configuration = ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        })
        .with_strike(Strike::Absolute(0.045));
        let mut hw_abs = HullWhite::new(0.1, &curve);
        hw_abs.calibrate_with_configuration(
            &abs_configuration,
            &store,
            &quote_store,
            &curve,
            Level::Mid,
        )?;

        let explicit_configuration =
            ModelCalibrationConfiguration::new(CalibrationSource::Surface {
                market_index: MarketIndex::SOFR,
            })
            .with_strike(Strike::Absolute(0.045));
        let mut hw_explicit = HullWhite::new(0.1, &curve);
        hw_explicit.calibrate_with_configuration(
            &explicit_configuration,
            &store,
            &quote_store,
            &curve,
            Level::Mid,
        )?;

        let sched_abs: Vec<(f64, f64)> = hw_abs
            .vol_func()
            .ok_or_else(|| QSError::UnexpectedErr("no vol func".into()))?
            .iter()
            .copied()
            .collect();
        let sched_explicit: Vec<(f64, f64)> = hw_explicit
            .vol_func()
            .ok_or_else(|| QSError::UnexpectedErr("no vol func".into()))?
            .iter()
            .copied()
            .collect();
        assert_eq!(sched_abs.len(), sched_explicit.len());
        for ((t_a, s_a), (t_e, s_e)) in sched_abs.iter().zip(&sched_explicit) {
            assert!((t_a - t_e).abs() < 1e-12);
            assert!((s_a - s_e).abs() < 1e-12);
        }
        Ok(())
    }

    #[test]
    fn calibration_basket_filters_expiry_and_tenor() -> Result<()> {
        let (mut quote_store, curve, _) = setup()?;
        let all_ids: Vec<String> = ["6M", "1Y"]
            .iter()
            .flat_map(|expiry| {
                ["0.035", "0.045", "0.055"].iter().map(move |strike| {
                    format!("CapletFloorlet_USD_SOFR_3M_{expiry}_Absolute_{strike}_Straddle_Black")
                })
            })
            .collect();
        for id in &all_ids {
            quote_store.add_quote(Quote::new(
                QuoteDetails::from_str(id)?,
                QuoteLevels::with_mid(MARKET_VOL),
            ));
        }
        let store = flat_surface_store(Date::new(2025, 1, 2), &all_ids);
        let calibration_basket = CalibrationBasket::all()
            .with_expiries(vec![Period::new(1, TimeUnit::Years)])
            .with_tenors(vec![Period::new(3, TimeUnit::Months)])
            .with_strike(Strike::Atm);
        let configuration = ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        })
        .with_calibration_basket(calibration_basket);

        let selected = configuration.resolve_instrument_ids(&store)?;
        assert_eq!(selected.len(), 3, "all three 1Y smile nodes are selected");
        assert!(selected.iter().all(|id| id.contains("_1Y_")));

        let missing_tenor = ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        })
        .with_calibration_basket(
            CalibrationBasket::all().with_tenors(vec![Period::new(6, TimeUnit::Months)]),
        );
        let error = missing_tenor.resolve_instrument_ids(&store).unwrap_err();
        assert!(matches!(
            error,
            QSError::NotFoundErr(message)
                if message.contains("tenor 6M") && message.contains("contains no instrument")
        ));

        let mut hw = HullWhite::new(0.1, &curve);
        hw.calibrate_with_configuration(&configuration, &store, &quote_store, &curve, Level::Mid)?;
        assert_eq!(
            hw.calibration_quality()
                .ok_or_else(|| QSError::UnexpectedErr("no quality".into()))?
                .records
                .len(),
            1,
            "ATM override collapses the smile to one calibration pillar"
        );
        Ok(())
    }

    #[test]
    fn normal_and_black_quotes_calibrate_to_same_sigma() -> Result<()> {
        use crate::models::utils::bachelier_call;

        let (mut quote_store, curve, _) = setup()?;
        let black_ids: Vec<String> = QUOTE_IDS.iter().map(ToString::to_string).collect();

        // Calibrate to the Black quotes.
        let mut hw_black = HullWhite::new(0.1, &curve);
        hw_black.calibrate(&black_ids, &quote_store, &curve, Level::Mid)?;
        let quality = hw_black
            .calibration_quality()
            .ok_or_else(|| QSError::UnexpectedErr("no quality".into()))?
            .clone();

        // Build price-equivalent Normal quotes: invert Bachelier on each
        // record's market price.
        let mut normal_ids = Vec::new();
        for rec in &quality.records {
            let tau = rec.big_t - rec.t;
            let df_end = curve.discount_factor_from_time(rec.big_t)?;
            let target = rec.market_price / (df_end * tau);
            let (mut lo, mut hi) = (1e-8_f64, 1.0_f64);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if bachelier_call(rec.forward_rate, rec.effective_strike, mid, rec.t)? < target {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let normal_vol = 0.5 * (lo + hi);

            let id = rec.identifier.replace("_Black", "_Normal");
            let details = QuoteDetails::from_str(&id)?;
            quote_store.add_quote(Quote::new(details, QuoteLevels::with_mid(normal_vol)));
            normal_ids.push(id);
        }

        // Calibrating to the equivalent Normal quotes must give the same sigma.
        let mut hw_normal = HullWhite::new(0.1, &curve);
        hw_normal.calibrate(&normal_ids, &quote_store, &curve, Level::Mid)?;

        let sched_black: Vec<(f64, f64)> = hw_black
            .vol_func()
            .ok_or_else(|| QSError::UnexpectedErr("no vol func".into()))?
            .iter()
            .copied()
            .collect();
        let sched_normal: Vec<(f64, f64)> = hw_normal
            .vol_func()
            .ok_or_else(|| QSError::UnexpectedErr("no vol func".into()))?
            .iter()
            .copied()
            .collect();
        assert_eq!(sched_black.len(), sched_normal.len());
        for ((t_b, s_b), (t_n, s_n)) in sched_black.iter().zip(&sched_normal) {
            assert!((t_b - t_n).abs() < 1e-12);
            assert!(
                (s_b - s_n).abs() < 1e-6,
                "Normal-quote sigma {s_n} should match Black-quote sigma {s_b}"
            );
        }
        Ok(())
    }

    #[test]
    fn calibrate_with_configuration_rejects_vol_convention_mismatch() -> Result<()> {
        // Black surface in the store, but Normal calibration quotes.
        let (mut quote_store, curve, _) = setup()?;
        let normal_ids: Vec<String> = QUOTE_IDS
            .iter()
            .map(|id| id.replace("_Black", "_Normal"))
            .collect();
        for id in &normal_ids {
            let details = QuoteDetails::from_str(id)?;
            quote_store.add_quote(Quote::new(details, QuoteLevels::with_mid(0.01)));
        }
        let store = flat_surface_store(Date::new(2025, 1, 2), &normal_ids);
        let configuration = ModelCalibrationConfiguration::new(CalibrationSource::Surface {
            market_index: MarketIndex::SOFR,
        });
        let mut hw = HullWhite::new(0.1, &curve);
        let result = hw.calibrate_with_configuration(
            &configuration,
            &store,
            &quote_store,
            &curve,
            Level::Mid,
        );
        assert!(result.is_err(), "Black surface + Normal quotes must error");
        Ok(())
    }
}

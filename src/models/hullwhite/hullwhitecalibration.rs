use crate::{
    ad::scalar::Scalar,
    calibration::{
        calibrationpricer::CalibrationInstrumentPricer, calibrationprocess::CalibrationProcess,
    },
    math::solvers::{bisection::Bisection, solvertraits::ContFunc},
    models::{
        montecarloengine::TimeDependentVolatility,
        utils::{black_call, swap_annuity_from_curve},
    },
    quotes::{
        calibrationinstrument::CalibrationInstrument,
        quote::{CalibrationInstrumentType, Level},
        quoteselector::QuoteSelector,
    },
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
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
    /// Computes the market price from a Black vol for the given calibration
    /// instrument.  This is the target that the model price must match.
    fn market_price(&self, instrument: &CalibrationInstrument, market_vol: f64) -> Result<f64> {
        // CapFloor: flat-vol Black price = sum of individual caplet Black prices.
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
                total += df_end * tau * black_call(fwd, strike.resolve(fwd), market_vol, t)?;
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
                Ok(df_end * tau * black_call(fwd, strike, market_vol, t)?)
            }
            CalibrationInstrumentType::EuropeanSwaption(_) => {
                Ok(annuity * black_call(fwd, strike, market_vol, t)?)
            }
            _ => Err(QSError::InvalidValueErr(
                "market_price: unsupported instrument type".into(),
            )),
        }
    }
}

impl CalibrationInstrumentPricer for HullWhiteCalibration<'_, '_> {
    /// Returns the HW model price for the given calibration instrument at the
    /// current trial sigma.  Uses the HW-implied ZCB-price volatility
    /// (`sigma_p`) in Black's formula.
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
                let sigma_p = self.hw.zcb_price_volatility(self.sigma, t, big_t);
                total += df_end * tau * black_call(fwd, strike.resolve(fwd), sigma_p, t)?;
            }
            return Ok(total);
        }

        let (t, big_t, fwd, strike, annuity) = extract_calibration_params(
            instrument,
            self.reference_date,
            self.day_counter,
            self.curve,
        )?;
        let sigma_p = self.hw.zcb_price_volatility(self.sigma, t, big_t);

        match instrument.built() {
            CalibrationInstrumentType::CapletFloorlet(_) => {
                let tau = big_t - t;
                let df_end = self.curve.discount_factor_from_time(big_t)?;
                Ok(df_end * tau * black_call(fwd, strike, sigma_p, t)?)
            }
            CalibrationInstrumentType::EuropeanSwaption(_) => {
                Ok(annuity * black_call(fwd, strike, sigma_p, t)?)
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

impl HullWhite<'_, f64> {
    /// Calibrates the short-rate volatility sigma(t) to market vol quotes,
    /// updating the internal volatility function and calibration quality.
    ///
    /// # Errors
    /// Returns an error if calibration quotes are missing or curve data is invalid.
    pub fn calibrate(
        &mut self,
        quote_ids: &[String],
        selector: &dyn QuoteSelector,
        curve: &dyn InterestRatesTermStructure<f64>,
        level: Level,
    ) -> Result<()> {
        let reference_date = selector.reference_date();
        let day_counter = curve
            .day_counter()
            .ok_or_else(|| QSError::InvalidValueErr("Curve has no day counter".to_string()))?;
        let n = quote_ids.len();

        let mut cal_instruments = Vec::with_capacity(n);
        for id in quote_ids {
            let quote = selector.select(id).ok_or_else(|| {
                QSError::NotFoundErr(format!("Calibration quote not found: {id}"))
            })?;
            let mkt_vol = quote.levels().value(level)?;
            let built = quote.build_instrument(reference_date, level, None)?;
            let pillar_date = built.pillar_date()?;
            cal_instruments.push(CalibrationInstrument::new(
                quote.clone(),
                level,
                built,
                mkt_vol,
                pillar_date,
            ));
        }

        let mut schedule = Vec::with_capacity(n);
        let mut labels = Vec::with_capacity(n);
        let mut sigma_values = Vec::with_capacity(n);
        let mut market_vols = Vec::with_capacity(n);
        let mut records = Vec::with_capacity(n);

        for ci in &cal_instruments {
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

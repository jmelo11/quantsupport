//! Resolved Hull–White parameters and differentiable calibration state.
use crate::rates::yieldtermstructure::primalcurve::PrimalCurve;
use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::marketdatahandling::marketdata::MarketData,
    indices::marketindex::MarketIndex,
    models::{
        hullwhite::{
            hullwhitecalibrationquality::HullWhiteCalibrationQuality, hullwhitemodel::HullWhite,
        },
        modelconfiguration::{ModelConfiguration, ParameterSource},
    },
    quotes::quote::QuoteDetails,
    time::daycounter::DayCounter,
    utils::errors::{QSError, Result},
    volatility::modelcalibration::{CalibrationSource, ModelCalibrationConfiguration},
};
/// Resolved Hull-White mean reversion, volatility schedule and calibration state.
#[derive(Clone)]
pub struct HullWhiteParameters<T: Scalar = f64> {
    model: ModelConfiguration,
    alpha: f64,
    sigma_schedule: Vec<(f64, T)>,
    calibration: Option<(HullWhiteCalibrationQuality, Vec<Vec<f64>>)>,
    day_counter: Option<DayCounter>,
}

/// Resolves the supported mean reversion and any fixed sigma schedule.
fn hull_white_parameters(configuration: &ModelConfiguration) -> Result<(f64, Vec<(f64, f64)>)> {
    match configuration {
        ModelConfiguration::HullWhite {
            alpha,
            parameter_source: ParameterSource::Fixed(parameters),
        } => {
            parameters.validate()?;
            Ok((*alpha, vec![(0.0, parameters.sigma)]))
        }
        ModelConfiguration::HullWhite {
            alpha,
            parameter_source: ParameterSource::Calibrated(_),
        } => Ok((*alpha, Vec::new())),
        ModelConfiguration::Lgm { .. } => Err(QSError::InvalidValueErr(
            "Callable-bond lattices currently support HullWhite; Lgm support is not yet implemented"
                .into(),
        )),
        ModelConfiguration::BrownianMotion { .. } => Err(QSError::InvalidValueErr(
            "BrownianMotion is not a short-rate callable-bond model".into(),
        )),
    }
}

impl HullWhiteParameters {
    /// Resolves the model's mean reversion and any fixed volatility schedule.
    ///
    /// # Errors
    /// Returns an error for unsupported models or invalid fixed parameters.
    pub fn new(model: ModelConfiguration) -> Result<Self> {
        let (alpha, sigma_schedule) = hull_white_parameters(&model)?;
        Ok(Self {
            model,
            alpha,
            sigma_schedule,
            calibration: None,
            day_counter: None,
        })
    }

    /// Returns the requested calibration when the volatility is calibrated.
    #[must_use]
    pub fn calibration_request(&self) -> Option<ModelCalibrationConfiguration> {
        match &self.model {
            ModelConfiguration::HullWhite {
                parameter_source: ParameterSource::Calibrated(calibration),
                ..
            } => Some(calibration.clone()),
            _ => None,
        }
    }

    /// Calibrates the volatility schedule and stores its quote sensitivities.
    ///
    /// # Errors
    /// Returns an error for missing market data, inconsistent reference dates
    /// or an invalid calibration result.
    pub fn resolve_calibration(
        &mut self,
        market_data: &MarketData,
        discount_index: &MarketIndex,
    ) -> Result<()> {
        let ModelConfiguration::HullWhite {
            parameter_source: ParameterSource::Calibrated(calibration),
            ..
        } = &self.model
        else {
            return Ok(());
        };
        let quotes = market_data.calibration_quotes().ok_or_else(|| {
            QSError::NotFoundErr(
                "Hull-White calibration quote details were not supplied by the market-data provider"
                    .into(),
            )
        })?;
        let curve = market_data
            .constructed_elements()
            .discount_curve(discount_index)
            .ok_or_else(|| {
                QSError::NotFoundErr(format!("Calibration curve not found for {discount_index}"))
            })?
            .curve();
        if curve.reference_date() != quotes.reference_date() {
            return Err(QSError::InvalidValueErr(
                "Calibrated Hull-White model and quotes must share the curve reference date".into(),
            ));
        }
        self.day_counter = Some(
            curve
                .day_counter()
                .ok_or_else(|| QSError::ValueNotSetErr("Calibration curve day counter".into()))?,
        );
        let primal = PrimalCurve::new(&*curve);
        let mut model = HullWhite::new(self.alpha, &primal);
        model.calibrate_with_configuration(
            calibration,
            market_data.constructed_elements(),
            quotes,
            &primal,
            market_data.quote_level(),
        )?;
        let volatility = model.vol_func().ok_or_else(|| {
            QSError::UnexpectedErr("Hull-White calibration produced no sigma schedule".into())
        })?;
        self.sigma_schedule = volatility.iter().copied().collect();
        if self.sigma_schedule.is_empty()
            || !self
                .sigma_schedule()
                .windows(2)
                .all(|entries| entries[0].0 < entries[1].0)
        {
            return Err(QSError::InvalidValueErr("Calibrated Hull-White sigma schedule must be nonempty and strictly ordered; select one tenor per expiry".into()));
        }
        self.calibration = Some((
            model.calibration_quality().ok_or_else(|| {
                QSError::UnexpectedErr("Missing Hull-White calibration quality".into())
            })?,
            volatility
                .ift_sensitivities()
                .ok_or_else(|| {
                    QSError::UnexpectedErr(
                        "Missing Hull-White calibration volatility sensitivities".into(),
                    )
                })?
                .clone(),
        ));
        Ok(())
    }

    /// Connects the resolved volatility schedule to AD market-volatility values.
    ///
    /// # Errors
    /// Returns an error for missing calibration data or invalid Jacobian dimensions.
    pub fn ad_parameters(&self, market_data: &MarketData) -> Result<HullWhiteParameters<DualFwd>> {
        let mut schedule = self
            .sigma_schedule()
            .iter()
            .map(|&(time, sigma)| (time, DualFwd::scalar(sigma)))
            .collect::<Vec<_>>();
        if let Some((quality, sensitivities)) = &self.calibration {
            let ModelConfiguration::HullWhite {
                parameter_source: ParameterSource::Calibrated(calibration),
                ..
            } = &self.model
            else {
                return Err(QSError::UnexpectedErr(
                    "Missing calibrated Hull-White configuration".into(),
                ));
            };
            let mut sampled_vols = Vec::with_capacity(quality.records.len());
            for record in &quality.records {
                let volatility = match calibration.source() {
                    CalibrationSource::Surface { market_index } => market_data
                        .constructed_elements()
                        .volatility_surface(market_index)
                        .ok_or_else(|| {
                            QSError::NotFoundErr(format!(
                                "Calibration surface not found for {market_index}"
                            ))
                        })?
                        .surface()
                        .volatility_from_period(record.expiry, record.effective_strike)?,
                    CalibrationSource::Cube { market_index } => {
                        let tenor = record
                            .identifier
                            .parse::<QuoteDetails>()?
                            .tenor()
                            .ok_or_else(|| {
                                QSError::NotFoundErr(format!(
                                    "Calibration tenor not found for {}",
                                    record.identifier
                                ))
                            })?;
                        market_data
                            .constructed_elements()
                            .volatility_cube(market_index)
                            .ok_or_else(|| {
                                QSError::NotFoundErr(format!(
                                    "Calibration cube not found for {market_index}"
                                ))
                            })?
                            .cube()
                            .volatility_from_period(record.expiry, tenor, record.effective_strike)?
                    }
                };
                sampled_vols.push(volatility.sub_val(DualFwd::scalar(record.market_vol)));
            }
            if sensitivities.len() != schedule.len()
                || sensitivities
                    .iter()
                    .any(|row| row.len() != sampled_vols.len())
            {
                return Err(QSError::UnexpectedErr(
                    "Invalid calibrated Hull-White volatility Jacobian dimensions".into(),
                ));
            }
            for ((_, sigma), row) in schedule.iter_mut().zip(sensitivities) {
                for (&delta, &derivative) in sampled_vols.iter().zip(row) {
                    *sigma = sigma.add_val(delta.mul_val(DualFwd::scalar(derivative)));
                }
            }
        }
        Ok(HullWhiteParameters {
            model: self.model.clone(),
            alpha: self.alpha,
            sigma_schedule: schedule,
            calibration: self.calibration.clone(),
            day_counter: self.day_counter,
        })
    }
}

impl<T: Scalar> HullWhiteParameters<T> {
    /// Returns the mean-reversion speed.
    #[must_use]
    pub const fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Returns the resolved time-dependent volatility schedule.
    #[must_use]
    pub fn sigma_schedule(&self) -> &[(f64, T)] {
        &self.sigma_schedule
    }

    /// Returns the day counter used by calibration, when available.
    #[must_use]
    pub const fn day_counter(&self) -> Option<DayCounter> {
        self.day_counter
    }
}

/// Returns the exact conditional variance of the Hull-White Gaussian state
/// over one uniform time step.
///
/// # Errors
/// Returns an error for an empty volatility schedule or an out-of-range time index.
pub fn transition_variances<T: Scalar>(
    alpha: f64,
    schedule: &[(f64, T)],
    steps: usize,
    time_step: f64,
) -> Result<Vec<T>> {
    if schedule.is_empty() {
        return Err(QSError::ValueNotSetErr(
            "Resolved Hull-White sigma schedule".into(),
        ));
    }
    (0..steps)
        .map(|step| {
            let start = f64::from(u32::try_from(step).map_err(|_| {
                QSError::InvalidValueErr("Hull-White time index exceeds u32".into())
            })?) * time_step;
            Ok(hull_white_piecewise_variance(
                alpha,
                schedule,
                start,
                start + time_step,
            ))
        })
        .collect()
}

/// Integrates the OU innovation variance, including every sigma boundary
/// inside the propagation interval. The first sigma also applies from zero.
pub fn hull_white_piecewise_variance<T: Scalar>(
    alpha: f64,
    schedule: &[(f64, T)],
    start: f64,
    end: f64,
) -> T {
    let mut variance = T::zero();
    for (index, &(time, sigma)) in schedule.iter().enumerate() {
        let lower = if index == 0 { start } else { start.max(time) };
        let upper = schedule.get(index + 1).map_or(end, |next| end.min(next.0));
        if upper <= lower {
            continue;
        }
        let weight = if alpha.abs() < 1.0e-12 {
            upper - lower
        } else {
            (-2.0 * alpha * (end - upper)).exp() * (-(-2.0 * alpha * (upper - lower)).exp_m1())
                / (2.0 * alpha)
        };
        variance = variance.add_val(sigma.mul_val(sigma).mul_val(T::scalar(weight)));
    }
    variance
}

#[cfg(test)]
#[test]
fn piecewise_hull_white_variance_splits_transition_boundaries() {
    let schedule = [(0.5, 0.01_f64), (1.0, 0.02)];
    let variance = hull_white_piecewise_variance(0.05, &schedule, 0.75, 1.25);
    let expected = 0.01_f64.powi(2) * (-0.025_f64).exp() * (1.0 - (-0.025_f64).exp()) / 0.1
        + 0.02_f64.powi(2) * (1.0 - (-0.025_f64).exp()) / 0.1;
    assert!((variance - expected).abs() < 1.0e-15);
    assert!((hull_white_piecewise_variance(0.0, &schedule, 0.0, 1.25) - 0.0002).abs() < 1.0e-15);
}

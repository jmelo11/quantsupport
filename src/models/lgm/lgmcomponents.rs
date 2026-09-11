//! LGM rate, FX, and equity model components.
//!
//! # Rate-model convention
//!
//! `LgmRateModel` uses a
//! one-factor Linear Gaussian Markov representation
//! fitted exactly to the supplied time-zero discount curve. With time measured
//! in years from the curve reference date, the implementation defines
//!
//! ```text
//! H(t)     = (1 - exp(-lambda * t)) / lambda
//! alpha(t) = sigma(t) * exp(lambda * t)
//! zeta(t)  = integral(0, t, alpha(s)^2 ds)
//! ```
//!
//! The Gaussian state follows `dz(t) = alpha(t) dW(t)` under the LGM numeraire
//! measure and starts at `z(0) = 0`. The short rate is obtained from
//! `r(t) = f(0,t) + H'(t) * (H(t) * zeta(t) + z(t))`. Bond prices combine this
//! state with the initial curve, giving an exact time-zero fit for every
//! `lambda` and `sigma`.
//!
//! `lambda` is a mean-reversion speed in inverse years. `sigma` is an
//! annualized absolute rate volatility in rate units per square-root year:
//! `0.01` means 100 basis points per square-root year.
//! The implementation supports the continuous `lambda = 0` limit.

use crate::{
    ad::scalar::Scalar,
    core::marketdatahandling::constructedelementstore::ConstructedElementStore,
    models::{
        hullwhite::hullwhitemodel::HullWhite, modelconfiguration::ModelConfiguration,
        montecarloengine::PathGenerator,
    },
    quotes::{quote::Level, quoteselector::QuoteSelector},
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    utils::errors::{QSError, Result},
    volatility::{
        modelcalibration::ModelCalibrationConfiguration,
        volatilitysource::VolatilitySourceConfiguration,
    },
};

/// One-factor LGM interest-rate model.
///
/// The two model parameters have distinct roles:
///
/// - `lambda` controls how quickly shocks lose influence at longer maturities
///   through the loading `H(T) - H(t)`. It is expressed in inverse years.
/// - `sigma(t)` controls the instantaneous absolute short-rate volatility and
///   is expressed in rate units per square-root year.
///
/// The borrowed discount curve supplies `P(0, t)` and the initial forward
/// curve. It is therefore both the calibration anchor and the source of the
/// model's exact time-zero curve fit. Use [`f64`] for ordinary valuation and an
/// AD scalar such as [`DualFwd`](crate::ad::dual::DualFwd) when parameter or
/// curve sensitivities are required.
pub struct LgmRateModel<'a, T: Scalar> {
    lambda: T,
    sigma_schedule: Vec<(f64, T)>,
    discount_curve: &'a dyn InterestRatesTermStructure<T>,
}

impl<'a, T: Scalar> LgmRateModel<'a, T> {
    /// Creates an LGM rate model with constant short-rate volatility.
    ///
    /// # Arguments
    ///
    /// - `lambda` — mean-reversion speed in inverse years. Larger positive
    ///   values reduce long-maturity factor loadings; zero selects the
    ///   non-mean-reverting limit.
    /// - `sigma` — annualized absolute short-rate volatility in rate units per
    ///   square-root year. For example, `0.01` is 100 bp/√year.
    /// - `discount_curve` — initial term structure used for `P(0, t)` and
    ///   forwards. Model-method times use the curve's time origin and
    ///   year-fraction convention.
    pub fn new(lambda: T, sigma: T, discount_curve: &'a dyn InterestRatesTermStructure<T>) -> Self {
        Self {
            lambda,
            sigma_schedule: vec![(0.0, sigma)],
            discount_curve,
        }
    }

    /// Creates an LGM rate model with piecewise-constant short-rate volatility.
    ///
    /// # Arguments
    ///
    /// - `lambda` — mean-reversion speed in inverse years, with the same
    ///   convention as [`Self::new`].
    /// - `sigma_schedule` — ordered `(start_time, sigma)` pairs. Times are year
    ///   fractions from the curve reference date and each value applies on
    ///   `[start_time, next_start_time)`. The first value also applies from
    ///   time zero when its stated start is positive. Each `sigma` is an
    ///   annualized absolute rate volatility.
    /// - `discount_curve` — initial term structure to reproduce exactly at
    ///   time zero.
    ///
    /// # Errors
    /// Returns an error for an empty schedule or schedule times outside strict
    /// ascending order.
    pub fn new_piecewise(
        lambda: T,
        sigma_schedule: Vec<(f64, T)>,
        discount_curve: &'a dyn InterestRatesTermStructure<T>,
    ) -> Result<Self> {
        if sigma_schedule.is_empty() {
            return Err(QSError::InvalidValueErr(
                "LgmRateModel: sigma schedule cannot be empty".into(),
            ));
        }
        if !sigma_schedule.windows(2).all(|w| w[0].0 < w[1].0) {
            return Err(QSError::InvalidValueErr(
                "LgmRateModel: sigma schedule times must be strictly increasing".into(),
            ));
        }
        Ok(Self {
            lambda,
            sigma_schedule,
            discount_curve,
        })
    }

    /// Returns the Gaussian-state drift under the LGM numeraire measure.
    ///
    /// The drift is identically zero. `t` is accepted to satisfy the common
    /// model interface and is a year fraction from the reference date.
    #[must_use]
    pub fn self_drift(&self, _t: f64) -> T {
        T::zero()
    }
}

impl<'a> LgmRateModel<'a, f64> {
    /// Creates an LGM rate model whose short-rate volatility is calibrated to
    /// market vols read from a constructed volatility surface or cube, as
    /// specified by the configuration's
    /// [`CalibrationSource`](crate::volatility::modelcalibration::CalibrationSource).
    ///
    /// The calibration is performed with the (mathematically equivalent)
    /// Hull-White parametrisation via
    /// [`HullWhite::calibrate_with_configuration`], and the resulting
    /// piecewise-constant sigma schedule is transferred to the LGM model.
    /// Market Black or normal volatilities define instrument-price calibration
    /// targets, and calibration solves the corresponding LGM sigma values.
    ///
    /// # Arguments
    ///
    /// - `lambda` — fixed mean-reversion speed in inverse years. Calibration
    ///   solves the volatility schedule while holding this value constant.
    /// - `discount_curve` — curve used both to price calibration instruments
    ///   and to anchor the LGM model.
    /// - `configuration` — calibration instruments, quote identifiers,
    ///   volatility convention, and calibration source.
    /// - `store` — constructed volatility surfaces or cubes referenced by the
    ///   configuration.
    /// - `selector` — market quote source used by the calibration instruments.
    /// - `level` — bid, mid, or ask quote level selected from the quote source.
    ///
    /// # Errors
    /// Returns an error when the configured surface or cube is unavailable,
    /// calibration quotes are missing, or calibration fails.
    pub fn calibrated(
        lambda: f64,
        discount_curve: &'a dyn InterestRatesTermStructure<f64>,
        configuration: &ModelCalibrationConfiguration,
        store: &ConstructedElementStore,
        selector: &dyn QuoteSelector,
        level: Level,
    ) -> Result<Self> {
        let mut hw = HullWhite::new(lambda, discount_curve);
        hw.calibrate_with_configuration(configuration, store, selector, discount_curve, level)?;
        let schedule: Vec<(f64, f64)> = hw
            .vol_func()
            .ok_or_else(|| {
                QSError::UnexpectedErr("LgmRateModel: calibration produced no vol function".into())
            })?
            .iter()
            .copied()
            .collect();
        Self::new_piecewise(lambda, schedule, discount_curve)
    }

    /// Creates an LGM rate model from a serde-enabled [`ModelConfiguration`].
    ///
    /// Supported volatility sources are `Constant` and `Calibrated`.
    /// `Constant` supplies LGM sigma directly. `Calibrated` derives LGM sigma
    /// from prices of instruments quoted through a volatility surface or cube.
    ///
    /// `configuration` supplies `lambda` and the volatility policy;
    /// `discount_curve`, `store`, `selector`, and `level` have the same roles
    /// as in [`Self::calibrated`]. A constant source interprets its value as
    /// absolute short-rate volatility in rate units per square-root year.
    ///
    /// # Errors
    /// Returns an error for another model type, an unsupported volatility
    /// source, or a calibration failure.
    pub fn from_configuration(
        configuration: &ModelConfiguration,
        discount_curve: &'a dyn InterestRatesTermStructure<f64>,
        store: &ConstructedElementStore,
        selector: &dyn QuoteSelector,
        level: Level,
    ) -> Result<Self> {
        let ModelConfiguration::Lgm { lambda, volatility } = configuration else {
            return Err(QSError::InvalidValueErr(format!(
                "LgmRateModel::from_configuration expects an Lgm model, got {configuration:?}"
            )));
        };
        match volatility {
            VolatilitySourceConfiguration::Constant { value } => {
                Ok(Self::new(*lambda, *value, discount_curve))
            }
            VolatilitySourceConfiguration::Calibrated(calibration) => {
                Self::calibrated(*lambda, discount_curve, calibration, store, selector, level)
            }
            VolatilitySourceConfiguration::Surface { .. }
            | VolatilitySourceConfiguration::Cube { .. } => Err(QSError::InvalidValueErr(
                "Lgm supports Constant or Calibrated volatility sources; sampling a \
                 surface/cube directly would misuse Black vols as short-rate vols"
                    .into(),
            )),
        }
    }
}

impl<T: Scalar> LgmRateModel<'_, T> {
    /// Returns the piecewise-constant short-rate volatility schedule.
    ///
    /// Each tuple is `(start_time_in_years, sigma)`. `sigma` is annualized
    /// absolute rate volatility, and the first entry also governs the interval
    /// from time zero to its stated start.
    #[must_use]
    pub fn sigma_schedule(&self) -> &[(f64, T)] {
        &self.sigma_schedule
    }

    /// Returns the piecewise-constant short-rate volatility at time `t`.
    fn sigma_at(&self, t: f64) -> T {
        let mut val = self.sigma_schedule[0].1;
        for &(ti, vi) in &self.sigma_schedule {
            if ti > t {
                break;
            }
            val = vi;
        }
        val
    }

    /// Returns the LGM maturity-loading function
    /// `H(t) = (1 - exp(-lambda * t)) / lambda`.
    ///
    /// `t` is a year fraction from the reference date. The function returns
    /// `t` when `lambda` is zero. Bond-price exposure to the state between
    /// observation `t` and maturity `T` is governed by `H(T) - H(t)`.
    #[allow(non_snake_case)]
    #[must_use]
    pub fn H(&self, t: f64) -> T {
        if self.lambda.value().abs() < 1e-14 {
            T::scalar(t)
        } else {
            // (1 - exp(-lambda * t)) / lambda
            let neg_lt = self.lambda.neg_val().mul_val(T::scalar(t));
            T::one().sub_val(neg_lt.exp()).div_val(self.lambda)
        }
    }

    /// Returns `dH(t)/dt = exp(-lambda * t)`.
    ///
    /// `t` is a year fraction from the reference date.
    #[allow(non_snake_case)]
    #[must_use]
    pub fn H_dot(&self, t: f64) -> T {
        if self.lambda.value().abs() < 1e-14 {
            T::one()
        } else {
            self.lambda.neg_val().mul_val(T::scalar(t)).exp()
        }
    }

    /// Returns the instantaneous Gaussian-state volatility
    /// `alpha(t) = sigma(t) * exp(lambda * t)`.
    ///
    /// `t` is a year fraction from the reference date. `alpha(t)` is the
    /// diffusion coefficient in `dz(t) = alpha(t) dW(t)`.
    #[must_use]
    pub fn alpha(&self, t: f64) -> T {
        let sigma = self.sigma_at(t);
        if self.lambda.value().abs() < 1e-14 {
            sigma
        } else {
            sigma.mul_val(self.lambda.mul_val(T::scalar(t)).exp())
        }
    }

    /// Returns the integrated state variance
    /// `zeta(t) = integral(0, t, alpha(s)^2 ds)`.
    ///
    /// `t` is a year fraction from the reference date. The integration is
    /// analytic on every piecewise-constant `sigma` interval.
    #[must_use]
    pub fn zeta(&self, t: f64) -> T {
        let n = self.sigma_schedule.len();
        let mut total = T::zero();
        for (j, &(tj, sigma)) in self.sigma_schedule.iter().enumerate() {
            // The first sigma also applies before its schedule time.
            let start = if j == 0 { 0.0 } else { tj };
            if start >= t {
                break;
            }
            let end = if j + 1 < n {
                self.sigma_schedule[j + 1].0.min(t)
            } else {
                t
            };
            if end <= start {
                continue;
            }
            let sigma_sq = sigma.mul_val(sigma);
            let piece = if self.lambda.value().abs() < 1e-14 {
                sigma_sq.mul_val(T::scalar(end - start))
            } else {
                // σ² (exp(2λ end) - exp(2λ start)) / (2λ)
                let two_lambda = self.lambda.mul_val(T::scalar(2.0));
                let e_end = two_lambda.mul_val(T::scalar(end)).exp();
                let e_start = two_lambda.mul_val(T::scalar(start)).exp();
                sigma_sq.mul_val(e_end.sub_val(e_start)).div_val(two_lambda)
            };
            total = total.add_val(piece);
        }
        total
    }

    /// Computes the model discount factor `P(t, maturity | z_t)`.
    ///
    /// # Arguments
    ///
    /// - `t` — observation time as a year fraction from the curve reference
    ///   date.
    /// - `T` — bond maturity time in the same year-fraction convention.
    /// - `z_t` — Gaussian LGM state observed at `t`, produced by integrating
    ///   `dz(s) = alpha(s) dW(s)` from time zero to `t`.
    ///
    /// At `t = 0` and `z_t = 0`, the result equals the initial curve's
    /// `P(0, T)`. The pricing domain is `T >= t`.
    ///
    /// # Errors
    /// Returns an error if discount factor lookup fails.
    #[allow(non_snake_case)]
    pub fn P_discount(&self, t: f64, T: f64, z_t: T) -> Result<T> {
        let p0_t = self.discount_curve.discount_factor_from_time(t)?;
        let p0_T = self.discount_curve.discount_factor_from_time(T)?;
        let h_t = self.H(t);
        let h_T = self.H(T);
        let zeta_t = self.zeta(t);
        // exponent = -(H(T) - H(t)) * z_t - 0.5 * (H(T)² - H(t)²) * ζ(t)
        let dh = h_T.sub_val(h_t);
        let h_sq_diff = h_T.mul_val(h_T).sub_val(h_t.mul_val(h_t));
        let exponent = dh
            .neg_val()
            .mul_val(z_t)
            .sub_val(T::scalar(0.5).mul_val(h_sq_diff).mul_val(zeta_t));
        Ok(p0_T.div_val(p0_t).mul_val(exponent.exp()))
    }

    /// Returns the LGM numeraire associated with the driftless state measure.
    ///
    /// Dividing a pathwise cashflow by this value expresses the cashflow in the
    /// LGM numeraire measure. The expectation can then be converted to its
    /// time-zero domestic value.
    ///
    /// # Arguments
    ///
    /// - `t` — numeraire observation time in years from the reference date.
    /// - `z_t` — Gaussian LGM state at `t`.
    ///
    /// # Errors
    /// Returns an error when the initial-curve lookup for `P(0, t)` fails.
    pub fn numeraire(&self, t: f64, z_t: T) -> Result<T> {
        let p0_t = self.discount_curve.discount_factor_from_time(t)?;
        let h_t = self.H(t);
        let exponent = h_t.mul_val(z_t).add_val(
            T::scalar(0.5)
                .mul_val(h_t)
                .mul_val(h_t)
                .mul_val(self.zeta(t)),
        );
        Ok(exponent.exp().div_val(p0_t))
    }

    /// Computes the instantaneous forward rate `f(t, maturity | z_t)`.
    ///
    /// # Arguments
    ///
    /// - `t` — state observation time in years from the reference date.
    /// - `T` — forward maturity time in the same convention.
    /// - `z_t` — Gaussian LGM state at `t`.
    ///
    /// The initial curve supplies `f(0, T)`; the remaining terms apply the
    /// stochastic LGM displacement.
    ///
    /// # Errors
    /// Returns an error if forward rate lookup fails.
    #[allow(non_snake_case)]
    pub fn instantaneous_forward_rate(&self, t: f64, T: f64, z_t: T) -> Result<T> {
        let f0_T = self.discount_curve.forward_rate_from_time(0.0, T)?;
        let h_T = self.H(T);
        let h_T_dot = self.H_dot(T);
        let zeta_t = self.zeta(t);
        // H'(T) * H(T) * ζ(t) + H'(T) * z_t + f(0,T)
        Ok(h_T_dot
            .mul_val(h_T)
            .mul_val(zeta_t)
            .add_val(h_T_dot.mul_val(z_t))
            .add_val(f0_T))
    }

    /// Computes the instantaneous short rate `r(t | z_t) = f(t, t | z_t)`.
    ///
    /// `t` is measured in years from the reference date and `z_t` is the
    /// Gaussian state at that time.
    ///
    /// # Errors
    /// Returns an error if forward rate computation fails.
    pub fn short_rate(&self, t: f64, z_t: T) -> Result<T> {
        self.instantaneous_forward_rate(t, t, z_t)
    }

    /// Computes the foreign-rate state drift under the domestic measure.
    ///
    /// # Arguments
    ///
    /// - `t` — current simulation time in years.
    /// - `domestic_rate_model` — LGM model for the domestic discount curve.
    /// - `fx_vol` — annualized lognormal FX volatility as a decimal; `0.12`
    ///   means 12% per square-root year.
    /// - `rho_zx_self_fx` — correlation between this foreign-rate Brownian
    ///   factor and the FX-spot Brownian factor.
    /// - `rho_zz_self_dom` — correlation between this foreign-rate Brownian
    ///   factor and the domestic-rate Brownian factor.
    ///
    /// Both correlations lie in `[-1, 1]` and equal the corresponding entries
    /// in the path-generation correlation matrix.
    #[must_use]
    pub fn gamma_under_domestic_measure(
        &self,
        t: f64,
        domestic_rate_model: &Self,
        fx_vol: f64,
        rho_zx_self_fx: f64,
        rho_zz_self_dom: f64,
    ) -> T {
        let alpha_i = self.alpha(t);
        let alpha_0 = domestic_rate_model.alpha(t);
        let h_i = self.H(t);
        let h_0 = domestic_rate_model.H(t);
        // rho_zz * α_i * α_0 * H_0 - α_i² * H_i - rho_zx * σ_fx * α_i
        T::scalar(rho_zz_self_dom)
            .mul_val(alpha_i)
            .mul_val(alpha_0)
            .mul_val(h_0)
            .sub_val(alpha_i.mul_val(alpha_i).mul_val(h_i))
            .sub_val(
                T::scalar(rho_zx_self_fx)
                    .mul_val(T::scalar(fx_vol))
                    .mul_val(alpha_i),
            )
    }

    /// Advances a Gaussian factor by one Euler step.
    ///
    /// # Arguments
    ///
    /// - `t` — start time of the step, in years.
    /// - `z_t` — factor value at `t`.
    /// - `dt` — positive step length in years.
    /// - `drift` — instantaneous factor drift at `t`.
    /// - `dw_z` — Brownian increment for the step, computed as a standard-normal
    ///   draw times `sqrt(dt)`.
    #[must_use]
    pub fn evolve_factor_euler(&self, t: f64, z_t: T, dt: f64, drift: T, dw_z: f64) -> T {
        // z + drift * dt + alpha(t) * dW
        z_t.add_val(drift.mul_val(T::scalar(dt)))
            .add_val(self.alpha(t).mul_val(T::scalar(dw_z)))
    }

    /// Advances the domestic Gaussian factor under its zero-drift LGM measure.
    ///
    /// `t`, `z_t`, and `dt` follow [`Self::evolve_factor_euler`]; `dw_z` is a
    /// Brownian increment already multiplied by `sqrt(dt)`.
    #[must_use]
    pub fn evolve_domestic_factor_euler(&self, t: f64, z_t: T, dt: f64, dw_z: f64) -> T {
        self.evolve_factor_euler(t, z_t, dt, T::zero(), dw_z)
    }

    /// Advances a foreign Gaussian factor under the domestic measure.
    ///
    /// `t`, `z_t`, `dt`, and `dw_z` follow [`Self::evolve_factor_euler`]. The
    /// remaining arguments follow [`Self::gamma_under_domestic_measure`].
    /// `fx_vol` is a decimal lognormal volatility; the correlation arguments
    /// are the entries used to correlate the path shocks.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn evolve_foreign_factor_under_domestic_measure_euler(
        &self,
        t: f64,
        z_t: T,
        dt: f64,
        dw_z: f64,
        domestic_rate_model: &Self,
        fx_vol: f64,
        rho_zx_self_fx: f64,
        rho_zz_self_dom: f64,
    ) -> T {
        let gamma = self.gamma_under_domestic_measure(
            t,
            domestic_rate_model,
            fx_vol,
            rho_zx_self_fx,
            rho_zz_self_dom,
        );
        self.evolve_factor_euler(t, z_t, dt, gamma, dw_z)
    }
}

impl PathGenerator<f64> for LgmRateModel<'_, f64> {
    /// Generates a short-rate path by evolving the Gaussian factor under its
    /// own measure with Euler steps and mapping it to `r(t|z_t)`.
    fn generate(&self, times: &[f64], draws: &[f64], scenario: &mut [f64]) -> Result<()> {
        if times.len() != draws.len() || times.len() != scenario.len() {
            return Err(QSError::InvalidValueErr(
                "LgmRateModel::generate: times, draws and scenario must have equal length".into(),
            ));
        }
        let mut z = 0.0;
        let mut prev_t = 0.0;
        for i in 0..times.len() {
            let t = times[i];
            let dt = t - prev_t;
            if dt <= 0.0 {
                return Err(QSError::InvalidValueErr(
                    "LgmRateModel::generate: times must be positive and strictly increasing".into(),
                ));
            }
            z = self.evolve_domestic_factor_euler(prev_t, z, dt, draws[i] * dt.sqrt());
            scenario[i] = self.short_rate(t, z)?;
            prev_t = t;
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  LgmFxModel
// ═══════════════════════════════════════════════════════════════════════════

/// Lognormal FX spot model coupled to domestic and foreign LGM rate factors.
///
/// The spot convention is domestic-currency units per one unit of foreign
/// currency. Under the domestic pricing measure, the spot drift includes the
/// stochastic domestic and foreign short rates plus the domestic-rate/FX
/// quanto adjustment. `fx_vol` is a flat annualized lognormal volatility.
pub struct LgmFxModel<'a, T: Scalar> {
    domestic: &'a LgmRateModel<'a, T>,
    foreign: &'a LgmRateModel<'a, T>,
    fx_vol: T,
    spot_0: T,
    rho_zx_dom_fx: T, // rho_{0i}^{zx}
}

impl<'a, T: Scalar> LgmFxModel<'a, T> {
    /// Creates a coupled LGM FX model.
    ///
    /// # Arguments
    ///
    /// - `domestic` — rate model for the currency in which the FX spot and
    ///   payoff are valued.
    /// - `foreign` — rate model for the foreign currency.
    /// - `fx_vol` — annualized lognormal spot volatility as a decimal;
    ///   `0.12` means 12% per square-root year.
    /// - `spot_0` — time-zero spot in domestic currency per one unit of
    ///   foreign currency. For a USD-domestic EUR model, this is EUR/USD.
    /// - `rho_zx_dom_fx` — correlation between domestic-rate and FX-spot
    ///   Brownian shocks. It controls the drift adjustment and equals the
    ///   corresponding entry in [`LgmMarketModel`](super::lgmmarketmodel::LgmMarketModel)'s
    ///   path correlation matrix.
    #[must_use]
    pub const fn new(
        domestic: &'a LgmRateModel<'a, T>,
        foreign: &'a LgmRateModel<'a, T>,
        fx_vol: T,
        spot_0: T,
        rho_zx_dom_fx: T,
    ) -> Self {
        Self {
            domestic,
            foreign,
            fx_vol,
            spot_0,
            rho_zx_dom_fx,
        }
    }

    /// Returns the annualized lognormal FX volatility as a decimal.
    #[must_use]
    pub const fn fx_vol(&self) -> T {
        self.fx_vol
    }

    /// Returns the initial spot in domestic-currency units per foreign unit.
    #[must_use]
    pub const fn initial_spot(&self) -> T {
        self.spot_0
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  LgmFxModel — generic T: Scalar instantiation
// ═══════════════════════════════════════════════════════════════════════════

impl<T: Scalar> LgmFxModel<'_, T> {
    /// Computes the proportional FX-spot drift under the domestic measure.
    ///
    /// `t` is the current time in years, while `z_dom` and `z_for` are the
    /// domestic and foreign Gaussian rate states. The result is the drift in
    /// `dX / X`, before subtracting half the FX variance for log evolution.
    ///
    /// # Errors
    /// Returns an error if short rate computation fails.
    pub fn fx_drift(&self, t: f64, z_dom: T, z_for: T) -> Result<T> {
        let r_0 = self.domestic.short_rate(t, z_dom)?;
        let r_i = self.foreign.short_rate(t, z_for)?;
        let alpha_0 = self.domestic.alpha(t);
        let h_0 = self.domestic.H(t);
        // rho * α_0 * H_0 * σ_fx + r_0 - r_i
        Ok(self
            .rho_zx_dom_fx
            .mul_val(alpha_0)
            .mul_val(h_0)
            .mul_val(self.fx_vol)
            .add_val(r_0)
            .sub_val(r_i))
    }

    /// Computes the drift of `log(X)` under the domestic measure.
    ///
    /// `t`, `z_dom`, and `z_for` follow [`Self::fx_drift`]. The result is the
    /// proportional spot drift less `0.5 * fx_vol^2`.
    ///
    /// # Errors
    /// Returns an error if FX drift computation fails.
    pub fn log_fx_drift(&self, t: f64, z_dom: T, z_for: T) -> Result<T> {
        let drift = self.fx_drift(t, z_dom, z_for)?;
        // drift - 0.5 * σ_fx²
        Ok(drift.sub_val(T::scalar(0.5).mul_val(self.fx_vol).mul_val(self.fx_vol)))
    }

    /// Evolves the FX spot using a positivity-preserving log-Euler step.
    ///
    /// # Arguments
    ///
    /// - `t` — step start time in years.
    /// - `x_t` — FX spot at `t`, using the domestic-per-foreign convention.
    /// - `z_dom` — domestic Gaussian rate state at `t`.
    /// - `z_for` — foreign Gaussian rate state at `t`.
    /// - `dt` — positive step length in years.
    /// - `dw_x` — FX Brownian increment already scaled by `sqrt(dt)`.
    ///
    /// # Errors
    /// Returns an error if log FX drift computation fails.
    pub fn evolve_fx_spot_log_euler(
        &self,
        t: f64,
        x_t: T,
        z_dom: T,
        z_for: T,
        dt: f64,
        dw_x: f64,
    ) -> Result<T> {
        let mu_log = self.log_fx_drift(t, z_dom, z_for)?;
        // x * exp(mu_log * dt + σ_fx * dW)
        let exponent = mu_log
            .mul_val(T::scalar(dt))
            .add_val(self.fx_vol.mul_val(T::scalar(dw_x)));
        Ok(x_t.mul_val(exponent.exp()))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  LgmEquityModel
// ═══════════════════════════════════════════════════════════════════════════

/// LGM equity model: lognormal spot with stochastic (LGM) domestic short rate
/// and a constant continuous dividend yield.
///
/// Structurally identical to [`LgmFxModel`] with the foreign short rate
/// replaced by the dividend yield `q`: under the domestic measure the spot
/// drift is `rho·α₀·H₀·σ + r_dom(t) − q`.
///
/// `vol` and `dividend_yield` are continuously compounded annualized decimals.
/// The spot remains positive because it is advanced with log-Euler steps.
pub struct LgmEquityModel<'a, T: Scalar> {
    domestic: &'a LgmRateModel<'a, T>,
    vol: T,
    spot_0: T,
    dividend_yield: T,
    rho_zs_dom: T, // corr(dz_dom, dW_S)
}

impl<'a, T: Scalar> LgmEquityModel<'a, T> {
    /// Creates an equity spot model coupled to the domestic LGM rate factor.
    ///
    /// # Arguments
    ///
    /// - `domestic` — LGM model for the payoff currency's rate curve.
    /// - `vol` — annualized lognormal equity volatility as a decimal; `0.20`
    ///   means 20% per square-root year.
    /// - `spot_0` — strictly positive time-zero equity spot.
    /// - `dividend_yield` — continuously compounded annual dividend yield as a
    ///   decimal; `0.02` means 2% per year.
    /// - `rho_zs_dom` — correlation between the domestic-rate and equity-spot
    ///   Brownian shocks. It controls the drift adjustment and equals the
    ///   corresponding path correlation-matrix entry.
    #[must_use]
    pub const fn new(
        domestic: &'a LgmRateModel<'a, T>,
        vol: T,
        spot_0: T,
        dividend_yield: T,
        rho_zs_dom: T,
    ) -> Self {
        Self {
            domestic,
            vol,
            spot_0,
            dividend_yield,
            rho_zs_dom,
        }
    }

    /// Returns the annualized lognormal equity volatility as a decimal.
    #[must_use]
    pub const fn vol(&self) -> T {
        self.vol
    }

    /// Returns the strictly positive time-zero equity spot.
    #[must_use]
    pub const fn initial_spot(&self) -> T {
        self.spot_0
    }

    /// Computes the proportional equity-spot drift under the domestic measure.
    ///
    /// `t` is the current time in years and `z_dom` is the domestic Gaussian
    /// rate state. The result is the drift in `dS / S`, before subtracting half
    /// the equity variance for log evolution.
    ///
    /// # Errors
    /// Returns an error if short rate computation fails.
    pub fn drift(&self, t: f64, z_dom: T) -> Result<T> {
        let r_0 = self.domestic.short_rate(t, z_dom)?;
        let alpha_0 = self.domestic.alpha(t);
        let h_0 = self.domestic.H(t);
        // rho * α_0 * H_0 * σ + r_0 - q
        Ok(self
            .rho_zs_dom
            .mul_val(alpha_0)
            .mul_val(h_0)
            .mul_val(self.vol)
            .add_val(r_0)
            .sub_val(self.dividend_yield))
    }

    /// Evolves the equity spot using a positivity-preserving log-Euler step.
    ///
    /// # Arguments
    ///
    /// - `t` — step start time in years.
    /// - `s_t` — equity spot at `t`.
    /// - `z_dom` — domestic Gaussian rate state at `t`.
    /// - `dt` — positive step length in years.
    /// - `dw_s` — equity Brownian increment already scaled by `sqrt(dt)`.
    ///
    /// # Errors
    /// Returns an error if drift computation fails.
    pub fn evolve_spot_log_euler(&self, t: f64, s_t: T, z_dom: T, dt: f64, dw_s: f64) -> Result<T> {
        let mu = self.drift(t, z_dom)?;
        // mu - 0.5 σ²
        let mu_log = mu.sub_val(T::scalar(0.5).mul_val(self.vol).mul_val(self.vol));
        // s * exp(mu_log * dt + σ * dW)
        let exponent = mu_log
            .mul_val(T::scalar(dt))
            .add_val(self.vol.mul_val(T::scalar(dw_s)));
        Ok(s_t.mul_val(exponent.exp()))
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc, str::FromStr};

    use super::*;
    use crate::{
        ad::dual::DualFwd,
        core::elements::volatilitysurfaceelement::VolatilitySurfaceElement,
        indices::marketindex::MarketIndex,
        math::interpolation::interpolator::Interpolator,
        quotes::{
            quote::{Quote, QuoteDetails, QuoteLevels},
            quotestore::QuoteStore,
        },
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
        volatility::{
            interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
            modelcalibration::CalibrationSource,
            volatilityindexing::{F64Key, SmileType, VolatilityType},
        },
    };

    const QUOTE_IDS: [&str; 2] = [
        "CapletFloorlet_USD_SOFR_3M_6M_Absolute_0.045_Straddle_Black",
        "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black",
    ];
    const MARKET_VOL: f64 = 0.20;

    fn flat_curve(reference_date: Date) -> Result<DiscountTermStructure<f64>> {
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
        DiscountTermStructure::<f64>::new(dates, dfs, dc, Interpolator::LogLinear, true)
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

        let curve = flat_curve(reference_date)?;

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
        );
        let mut store = ConstructedElementStore::default();
        store.volatility_surfaces_mut().insert(
            MarketIndex::SOFR,
            VolatilitySurfaceElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(surface))),
        );

        Ok((quote_store, curve, store))
    }

    #[test]
    fn constant_and_single_piece_schedules_agree() -> Result<()> {
        let curve = flat_curve(Date::new(2025, 1, 2))?;
        let constant = LgmRateModel::new(0.05_f64, 0.01, &curve);
        let piecewise = LgmRateModel::new_piecewise(0.05_f64, vec![(0.0, 0.01)], &curve)?;
        for t in [0.1, 1.0, 5.0, 10.0] {
            assert!((constant.alpha(t) - piecewise.alpha(t)).abs() < 1e-15);
            assert!((constant.zeta(t) - piecewise.zeta(t)).abs() < 1e-15);
        }
        Ok(())
    }

    #[test]
    fn piecewise_zeta_sums_segment_variances() -> Result<()> {
        let curve = flat_curve(Date::new(2025, 1, 2))?;
        let lambda = 0.05_f64;
        let (s1, s2, t1, t2) = (0.010_f64, 0.014_f64, 1.0_f64, 3.0_f64);
        let model = LgmRateModel::new_piecewise(lambda, vec![(0.0, s1), (t1, s2)], &curve)?;

        let two_lambda = 2.0 * lambda;
        let seg = |sigma: f64, a: f64, b: f64| {
            sigma * sigma * ((two_lambda * b).exp() - (two_lambda * a).exp()) / two_lambda
        };
        let expected = seg(s1, 0.0, t1) + seg(s2, t1, t2);
        assert!((model.zeta(t2) - expected).abs() < 1e-14);

        // Before the second piece kicks in, zeta matches the constant model.
        let constant = LgmRateModel::new(lambda, s1, &curve);
        assert!((model.zeta(0.7) - constant.zeta(0.7)).abs() < 1e-15);

        // alpha uses the piecewise sigma.
        assert!((model.alpha(2.0) - s2 * (lambda * 2.0_f64).exp()).abs() < 1e-15);
        Ok(())
    }

    #[test]
    fn calibrated_matches_hull_white_schedule() -> Result<()> {
        let (quote_store, curve, store) = setup()?;
        let quote_ids: Vec<String> = QUOTE_IDS.iter().map(ToString::to_string).collect();
        let lambda = 0.1_f64;
        let configuration = ModelCalibrationConfiguration::new(
            CalibrationSource::Surface {
                market_index: MarketIndex::SOFR,
            },
            quote_ids.clone(),
            lambda,
        );

        let lgm = LgmRateModel::calibrated(
            lambda,
            &curve,
            &configuration,
            &store,
            &quote_store,
            Level::Mid,
        )?;

        let mut hw = HullWhite::new(lambda, &curve);
        hw.calibrate(&quote_ids, &quote_store, &curve, Level::Mid)?;
        let hw_schedule: Vec<(f64, f64)> = hw
            .vol_func()
            .ok_or_else(|| QSError::UnexpectedErr("no vol func".into()))?
            .iter()
            .copied()
            .collect();

        assert_eq!(lgm.sigma_schedule().len(), hw_schedule.len());
        for ((t_l, s_l), (t_h, s_h)) in lgm.sigma_schedule().iter().zip(&hw_schedule) {
            assert!((t_l - t_h).abs() < 1e-12);
            assert!((s_l - s_h).abs() < 1e-12);
        }
        Ok(())
    }

    #[test]
    fn from_configuration_supports_constant_and_calibrated_only() -> Result<()> {
        let (quote_store, curve, store) = setup()?;

        let constant = ModelConfiguration::Lgm {
            lambda: 0.05,
            volatility: VolatilitySourceConfiguration::Constant { value: 0.01 },
        };
        let model =
            LgmRateModel::from_configuration(&constant, &curve, &store, &quote_store, Level::Mid)?;
        assert!((model.alpha(0.0) - 0.01).abs() < 1e-15);

        let calibrated = ModelConfiguration::Lgm {
            lambda: 0.1,
            volatility: VolatilitySourceConfiguration::Calibrated(
                ModelCalibrationConfiguration::new(
                    CalibrationSource::Surface {
                        market_index: MarketIndex::SOFR,
                    },
                    QUOTE_IDS.iter().map(ToString::to_string).collect(),
                    0.1,
                ),
            ),
        };
        let model = LgmRateModel::from_configuration(
            &calibrated,
            &curve,
            &store,
            &quote_store,
            Level::Mid,
        )?;
        assert_eq!(model.sigma_schedule().len(), 2);

        let surface_sampled = ModelConfiguration::Lgm {
            lambda: 0.05,
            volatility: VolatilitySourceConfiguration::Surface {
                market_index: MarketIndex::SOFR,
                key: 0.045,
            },
        };
        assert!(LgmRateModel::from_configuration(
            &surface_sampled,
            &curve,
            &store,
            &quote_store,
            Level::Mid
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn path_generator_produces_finite_short_rates() -> Result<()> {
        let curve = flat_curve(Date::new(2025, 1, 2))?;
        let model = LgmRateModel::new(0.05_f64, 0.01, &curve);
        let times = [0.5, 1.0, 1.5, 2.0];
        let draws = [0.3, -0.5, 1.2, -0.1];
        let mut scenario = [0.0_f64; 4];
        model.generate(&times, &draws, &mut scenario)?;
        for r in scenario {
            assert!(r.is_finite());
            assert!((r - 0.045).abs() < 0.05, "short rate {r} far from curve");
        }

        let mut short = [0.0_f64; 2];
        assert!(model.generate(&times, &draws, &mut short).is_err());
        Ok(())
    }
}

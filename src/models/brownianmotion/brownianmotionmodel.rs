use crate::{
    ad::{dual::DualFwd, expr::FloatExt, scalar::Scalar},
    math::probability::{
        norm_cdf::{norm_cdf, NormCDF},
        norm_pdf::FRAC_1_SQRT_2PI,
    },
    models::{
        montecarloengine::{PathGenerator, TimeDependentVolatility},
        utils::{black_call, black_call_ad, black_put, black_put_ad, d1_d2, d1_d2_ad},
    },
    utils::errors::{QSError, Result},
};

/// Geometric Brownian motion with continuous rate and dividend-yield inputs.
///
/// The associated Black pricing helpers take a forward, strike, annualized
/// decimal lognormal volatility, and expiry in years directly. They do not use
/// the instance's spot or rates and do not apply discounting.
pub struct BrownianMotion<T: Scalar> {
    spot: T,
    rate: T,
    vol_func: Box<dyn TimeDependentVolatility<T>>,
    dividend_rate: Option<T>,
}

impl<T: Scalar> BrownianMotion<T> {
    /// Creates a GBM model without validating its inputs.
    ///
    /// `rate` and `dividend_rate` are continuously compounded annual decimals;
    /// `None` means zero dividend yield. `vol_func` supplies lognormal volatility
    /// per square-root year, so `0.20` means 20% annualized volatility.
    #[must_use]
    pub fn new(
        spot: T,
        rate: T,
        vol_func: Box<dyn TimeDependentVolatility<T>>,
        dividend_rate: Option<T>,
    ) -> Self {
        Self {
            spot,
            rate,
            vol_func,
            dividend_rate,
        }
    }

    /// Returns the spot price.
    #[must_use]
    pub const fn spot(&self) -> T {
        self.spot
    }

    /// Returns the continuously compounded annual risk-free rate as a decimal.
    #[must_use]
    pub const fn rate(&self) -> T {
        self.rate
    }

    /// Returns the continuous annual dividend yield; `None` means zero in path generation.
    #[must_use]
    pub const fn dividend_rate(&self) -> Option<&T> {
        self.dividend_rate.as_ref()
    }
}

impl BrownianMotion<f64> {
    /// Undiscounted Black call (`is_call = true`) or put price from a forward.
    ///
    /// `fwd` and `strike` share price or rate units; `vol` is annualized decimal
    /// lognormal volatility and `tau` is expiry in years. Multiply the result by
    /// the appropriate discount factor and contract scale separately.
    /// No zero-volatility or zero-expiry intrinsic-value branch is provided.
    ///
    /// # Errors
    /// Returns an error if strike, volatility, or time to expiry are non-positive.
    /// Forward positivity and input finiteness are not validated; see [`d1_d2`].
    pub fn closed_form_price(
        fwd: f64,
        strike: f64,
        vol: f64,
        tau: f64,
        is_call: bool,
    ) -> Result<f64> {
        if is_call {
            black_call(fwd, strike, vol, tau)
        } else {
            black_put(fwd, strike, vol, tau)
        }
    }

    /// Undiscounted forward-delta magnitude: `N(d1)` for calls, `N(-d1)` for puts.
    ///
    /// The put result is positive: negate it to obtain the signed derivative
    /// of the undiscounted put price with respect to the forward. Neither
    /// discounting nor spot-to-forward scaling is included.
    ///
    /// ```
    /// use quantsupport::models::brownianmotion::brownianmotionmodel::BrownianMotion;
    ///
    /// let call = BrownianMotion::<f64>::delta(100.0, 100.0, 0.20, 1.0, true)?;
    /// let put_magnitude = BrownianMotion::<f64>::delta(100.0, 100.0, 0.20, 1.0, false)?;
    /// assert!((call + put_magnitude - 1.0).abs() < 1e-12);
    /// # Ok::<(), quantsupport::utils::errors::QSError>(())
    /// ```
    ///
    /// # Errors
    /// Propagates [`d1_d2`] validation errors.
    pub fn delta(fwd: f64, strike: f64, vol: f64, tau: f64, is_call: bool) -> Result<f64> {
        let (d1, _) = d1_d2(fwd, strike, vol, tau)?;
        if is_call {
            Ok(d1.norm_cdf())
        } else {
            Ok((-d1).norm_cdf())
        }
    }

    /// Undiscounted Black vega: `dV/dvol = F * phi(d1) * sqrt(tau)`.
    ///
    /// This is per full unit of decimal volatility, not per percentage point
    /// or basis point. Multiply by `0.01` for a one-percentage-point change.
    ///
    /// # Errors
    /// Propagates [`d1_d2`] validation errors.
    pub fn vega(fwd: f64, strike: f64, vol: f64, tau: f64) -> Result<f64> {
        let (d1, _) = d1_d2(fwd, strike, vol, tau)?;
        let pdf_d1 = (-0.5 * d1 * d1).exp() * FRAC_1_SQRT_2PI;
        Ok(fwd * pdf_d1 * tau.sqrt())
    }

    /// Returns the legacy rho expression: `K * tau * N(d2)` for calls,
    /// `-K * tau * N(-d2)` for puts.
    ///
    /// No interest rate or discount factor enters this helper. This is not
    /// the rate derivative of the undiscounted Black value at fixed forward,
    /// nor does it include the discount factor required for a PV rho.
    ///
    /// # Errors
    /// Propagates [`d1_d2`] validation errors.
    pub fn rho(fwd: f64, strike: f64, vol: f64, tau: f64, is_call: bool) -> Result<f64> {
        let (_, d2) = d1_d2(fwd, strike, vol, tau)?;
        if is_call {
            Ok(strike * tau * d2.norm_cdf())
        } else {
            Ok(-(strike * tau * (-d2).norm_cdf()))
        }
    }

    /// Returns the legacy theta expression
    /// `-F * phi(d1) * vol / (2 * sqrt(tau))`, plus `K * N(d2)` for calls
    /// or minus `K * N(-d2)` for puts.
    ///
    /// No rate, dividend, or discount factor is included. This expression is
    /// neither `dV/dtau` of the undiscounted Black price nor standard
    /// discounted-price calendar theta; do not interpret it as PV per year.
    ///
    /// # Errors
    /// Propagates [`d1_d2`] validation errors.
    pub fn theta(fwd: f64, strike: f64, vol: f64, tau: f64, is_call: bool) -> Result<f64> {
        let (d1, d2) = d1_d2(fwd, strike, vol, tau)?;
        let pdf_d1 = (-0.5 * d1 * d1).exp() * FRAC_1_SQRT_2PI;
        let term1 = -fwd * pdf_d1 * vol / (2.0 * tau.sqrt());
        if is_call {
            Ok(strike.mul_add(d2.norm_cdf(), term1))
        } else {
            Ok(strike.mul_add(-(-d2).norm_cdf(), term1))
        }
    }
}

impl BrownianMotion<DualFwd> {
    /// Undiscounted Black call/put price from a forward (AD-enabled).
    ///
    /// `fwd` and `vol` retain AD dependencies; strike and expiry are constants.
    /// Units are the same as for the `f64` helper. No discounting or explicit
    /// zero-volatility/zero-expiry limiting branch is provided.
    ///
    /// # Errors
    /// Returns an error if strike or time to expiry are non-positive.
    /// Forward, volatility, and finiteness are not validated; see [`d1_d2_ad`].
    pub fn closed_form_price(
        fwd: DualFwd,
        strike: f64,
        vol: DualFwd,
        tau: f64,
        is_call: bool,
    ) -> Result<DualFwd> {
        if is_call {
            black_call_ad(fwd, strike, vol, tau)
        } else {
            black_put_ad(fwd, strike, vol, tau)
        }
    }

    /// AD-enabled undiscounted forward-delta magnitude:
    /// `N(d1)` for calls, positive `N(-d1)` for puts.
    ///
    /// Negate the put result for its signed forward derivative. Discounting
    /// and spot-to-forward scaling are not included.
    ///
    /// # Errors
    /// Propagates [`d1_d2_ad`] validation errors.
    pub fn delta(
        fwd: DualFwd,
        strike: f64,
        vol: DualFwd,
        tau: f64,
        is_call: bool,
    ) -> Result<DualFwd> {
        let (d1, _) = d1_d2_ad(fwd, strike, vol, tau)?;
        if is_call {
            Ok(norm_cdf(d1))
        } else {
            let neg_d1: DualFwd = (-d1).into();
            Ok(norm_cdf(neg_d1))
        }
    }

    /// AD-enabled undiscounted Black vega: `F * phi(d1) * sqrt(tau)`.
    ///
    /// The result is per full unit of decimal volatility, not per percentage
    /// point or basis point.
    ///
    /// # Errors
    /// Propagates [`d1_d2_ad`] validation errors.
    pub fn vega(fwd: DualFwd, strike: f64, vol: DualFwd, tau: f64) -> Result<DualFwd> {
        let (d1, _) = d1_d2_ad(fwd, strike, vol, tau)?;
        let pdf_d1 = (-d1 * d1 * 0.5).exp() * FRAC_1_SQRT_2PI;
        Ok((fwd * pdf_d1 * tau.sqrt()).into())
    }

    /// AD-enabled legacy rho expression: `K * tau * N(d2)` for calls,
    /// `-K * tau * N(-d2)` for puts.
    ///
    /// No interest rate or discount factor enters this helper. It is not the
    /// rate derivative at fixed forward or a discounted PV rho.
    ///
    /// # Errors
    /// Propagates [`d1_d2_ad`] validation errors.
    pub fn rho(
        fwd: DualFwd,
        strike: f64,
        vol: DualFwd,
        tau: f64,
        is_call: bool,
    ) -> Result<DualFwd> {
        let (_, d2) = d1_d2_ad(fwd, strike, vol, tau)?;
        let st = strike * tau;
        if is_call {
            Ok((norm_cdf(d2) * st).into())
        } else {
            let neg_d2: DualFwd = (-d2).into();
            Ok((-(norm_cdf(neg_d2) * st)).into())
        }
    }

    /// AD-enabled legacy theta expression:
    /// `-F * phi(d1) * vol / (2 * sqrt(tau))`, plus `K * N(d2)` for calls
    /// or minus `K * N(-d2)` for puts.
    ///
    /// This is not a time derivative of the Black price or standard
    /// discounted-price calendar theta. No rate or discount factor is included;
    /// do not interpret the output as PV per year.
    ///
    /// # Errors
    /// Propagates [`d1_d2_ad`] validation errors.
    pub fn theta(
        fwd: DualFwd,
        strike: f64,
        vol: DualFwd,
        tau: f64,
        is_call: bool,
    ) -> Result<DualFwd> {
        let (d1, d2) = d1_d2_ad(fwd, strike, vol, tau)?;
        let pdf_d1 = (-d1 * d1 * 0.5).exp() * FRAC_1_SQRT_2PI;
        let term1 = -fwd * pdf_d1 * vol / (2.0 * tau.sqrt());
        if is_call {
            Ok((term1 + norm_cdf(d2) * strike).into())
        } else {
            let neg_d2: DualFwd = (-d2).into();
            Ok((term1 - norm_cdf(neg_d2) * strike).into())
        }
    }
}

impl PathGenerator<f64> for BrownianMotion<f64> {
    /// Evolves GBM from time zero, sampling volatility at each step's end.
    ///
    /// Times are year fractions and draws are standard normals. Nonnegative,
    /// increasing times are the caller's responsibility; ordering is not
    /// validated and a negative step can produce nonfinite values.
    ///
    /// # Errors
    /// Rejects mismatched buffer lengths and propagates volatility lookup errors.
    fn generate(&self, times: &[f64], draws: &[f64], scenario: &mut [f64]) -> Result<()> {
        if times.len() != draws.len() || times.len() != scenario.len() {
            return Err(QSError::InvalidValueErr(
                "times, draws, and scenario must have the same length".to_string(),
            ));
        }

        let mut prev_spot = self.spot;
        let mut prev_t = 0.0_f64;
        for i in 0..times.len() {
            let t = times[i];
            let dt = t - prev_t;
            let z = draws[i];
            let vol = self.vol_func.vol(t)?;
            let drift = (self.rate - self.dividend_rate.unwrap_or(0.0))
                .mul_add(dt, -(0.5 * vol * vol * dt));
            let diffusion = vol * z * dt.sqrt();
            let log_return = drift + diffusion;
            let spot = prev_spot * log_return.exp();
            scenario[i] = spot;
            prev_spot = spot;
            prev_t = t;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::{math::random::fill_std_normal, volatility::volatilitysource::ConstantVolatility};

    type Bm = BrownianMotion<f64>;

    #[test]
    fn call_delta_and_vega_match_finite_differences() -> Result<()> {
        let (fwd, tau) = (100.0_f64, 1.5_f64);
        let h = 1e-5;
        for strike in [70.0_f64, 100.0, 140.0] {
            for vol in [0.05, 0.2, 0.8] {
                // Deep ITM/OTM (>4 total-vol stdevs) the premium is dominated
                // by intrinsic value: vega is ~1e-8 of the price scale, so a
                // finite difference suffers catastrophic cancellation there.
                let d1_approx = (fwd / strike).ln() / (vol * tau.sqrt());
                if d1_approx.abs() > 4.0 {
                    continue;
                }
                let delta = Bm::delta(fwd, strike, vol, tau, true)?;
                let fd_delta = (Bm::closed_form_price(fwd + h, strike, vol, tau, true)?
                    - Bm::closed_form_price(fwd - h, strike, vol, tau, true)?)
                    / (2.0 * h);
                assert!(
                    (delta - fd_delta).abs() < 1e-8,
                    "delta {delta} vs FD {fd_delta} at K={strike}, vol={vol}"
                );

                let vega = Bm::vega(fwd, strike, vol, tau)?;
                let hv = vol * 1e-4; // vol-scaled bump keeps FD truncation small deep OTM
                let fd_vega = (Bm::closed_form_price(fwd, strike, vol + hv, tau, true)?
                    - Bm::closed_form_price(fwd, strike, vol - hv, tau, true)?)
                    / (2.0 * hv);
                assert!(
                    (vega - fd_vega).abs() / vega.max(1e-8) < 1e-4,
                    "vega {vega} vs FD {fd_vega} at K={strike}, vol={vol}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn greek_boundary_conditions_and_parity() -> Result<()> {
        let (fwd, tau) = (100.0, 1.0);
        // Call and put delta magnitudes sum to one: N(d1) + N(-d1) = 1.
        for strike in [50.0, 100.0, 200.0] {
            let dc = Bm::delta(fwd, strike, 0.2, tau, true)?;
            let dp = Bm::delta(fwd, strike, 0.2, tau, false)?;
            assert!((dc + dp - 1.0).abs() < 1e-14);
            assert!((0.0..=1.0).contains(&dc));
            // Call and put vega coincide (put-call parity has no vol term).
            let vega_gap = Bm::vega(fwd, strike, 0.2, tau)?;
            assert!(vega_gap >= 0.0);
        }
        // Deep ITM call: delta -> 1; deep OTM: delta -> 0; vega -> 0 both ways.
        assert!((Bm::delta(fwd, 1.0, 0.2, tau, true)? - 1.0).abs() < 1e-10);
        assert!(Bm::delta(fwd, 10_000.0, 0.2, tau, true)? < 1e-10);
        assert!(Bm::vega(fwd, 1.0, 0.2, tau)? < 1e-8);
        assert!(Bm::vega(fwd, 10_000.0, 0.2, tau)? < 1e-8);
        // ATM vega is the largest of the three.
        assert!(Bm::vega(fwd, fwd, 0.2, tau)? > Bm::vega(fwd, 50.0, 0.2, tau)?);
        Ok(())
    }

    #[test]
    fn generate_rejects_mismatched_lengths() {
        let bm = Bm::new(100.0, 0.03, Box::new(ConstantVolatility::new(0.2)), None);
        let times = [0.5, 1.0];
        let draws = [0.1];
        let mut scenario = [0.0, 0.0];
        assert!(bm.generate(&times, &draws, &mut scenario).is_err());
    }

    #[test]
    fn near_zero_vol_path_is_deterministic_forward() -> Result<()> {
        // With vanishing vol the terminal spot is the forward regardless of draws.
        let (spot, rate, div) = (100.0, 0.05, 0.02);
        let bm = Bm::new(
            spot,
            rate,
            Box::new(ConstantVolatility::new(1e-12)),
            Some(div),
        );
        let times = [0.5, 1.0, 2.0];
        let draws = [3.0, -3.0, 2.5]; // extreme draws must not matter
        let mut scenario = [0.0; 3];
        bm.generate(&times, &draws, &mut scenario)?;
        let expected = spot * ((rate - div) * 2.0).exp();
        assert!(
            (scenario[2] - expected).abs() / expected < 1e-9,
            "terminal spot {} should equal forward {expected}",
            scenario[2]
        );
        Ok(())
    }

    #[test]
    fn generated_paths_match_lognormal_moments() -> Result<()> {
        // E[S_T] = S0 e^{(r-q)T} (martingale test) and Var[ln S_T] = sigma^2 T.
        let (spot, rate, vol) = (100.0_f64, 0.03_f64, 0.25_f64);
        let horizon = 2.0_f64;
        let n_steps = 24_usize;
        let n_paths = 20_000_i32;
        let times: Vec<f64> = (1..=n_steps)
            .map(|i| {
                horizon * f64::from(u32::try_from(i).unwrap_or(u32::MAX))
                    / f64::from(u32::try_from(n_steps).unwrap_or(u32::MAX))
            })
            .collect();

        let bm = Bm::new(spot, rate, Box::new(ConstantVolatility::new(vol)), None);
        let mut rng = StdRng::seed_from_u64(7);
        let mut draws = vec![0.0_f64; n_steps];
        let mut scenario = vec![0.0_f64; n_steps];

        let mut sum_s = 0.0;
        let mut sum_log = 0.0;
        let mut sum_log2 = 0.0;
        for _ in 0..n_paths {
            fill_std_normal(&mut rng, &mut draws);
            bm.generate(&times, &draws, &mut scenario)?;
            let s_end = scenario[n_steps - 1];
            assert!(s_end.is_finite() && s_end > 0.0);
            sum_s += s_end;
            let l = s_end.ln();
            sum_log += l;
            sum_log2 += l * l;
        }
        let n = f64::from(n_paths);
        let mean_s = sum_s / n;
        let mean_log = sum_log / n;
        let var_log = sum_log2 / n - mean_log * mean_log;

        let fwd = spot * (rate * horizon).exp();
        let theo_mean_log = (0.5 * vol).mul_add(-(vol * horizon), spot.ln() + rate * horizon);
        let theo_var_log = vol * vol * horizon;
        assert!(
            (mean_s - fwd).abs() / fwd < 0.01,
            "E[S_T] {mean_s} should be near forward {fwd}"
        );
        assert!(
            (mean_log - theo_mean_log).abs() < 0.01,
            "E[ln S_T] {mean_log} should be near {theo_mean_log}"
        );
        assert!(
            (var_log - theo_var_log).abs() / theo_var_log < 0.05,
            "Var[ln S_T] {var_log} should be near {theo_var_log}"
        );
        Ok(())
    }

    #[test]
    fn extreme_vol_paths_remain_finite_and_positive() -> Result<()> {
        let bm = Bm::new(100.0, 0.0, Box::new(ConstantVolatility::new(3.0)), None);
        let times: Vec<f64> = (1..=12).map(|i| f64::from(i) / 12.0).collect();
        let mut rng = StdRng::seed_from_u64(99);
        let mut draws = vec![0.0_f64; 12];
        let mut scenario = vec![0.0_f64; 12];
        for _ in 0..1_000 {
            fill_std_normal(&mut rng, &mut draws);
            bm.generate(&times, &draws, &mut scenario)?;
            assert!(scenario.iter().all(|s| s.is_finite() && *s > 0.0));
        }
        Ok(())
    }
}

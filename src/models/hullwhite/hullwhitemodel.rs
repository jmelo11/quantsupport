use crate::{
    ad::scalar::Scalar,
    math::probability::norm_cdf::norm_cdf,
    models::{
        hullwhite::{
            hullwhitecalibration::HullWhiteTimeDependentVolatility,
            hullwhitecalibrationquality::HullWhiteCalibrationQuality,
        },
        montecarloengine::{PathGenerator, TimeDependentVolatility},
    },
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    utils::errors::Result,
};

/// One-factor Hull-White analytic helpers and short-rate path generator.
///
/// `alpha` is mean reversion in inverse years. Method times are year fractions
/// from the input curve's reference date; `sigma` is absolute short-rate
/// volatility in decimal-rate units per square-root year (`0.01` means 100 bp).
/// Bond prices are per unit principal and caplet/floorlet prices per unit
/// notional. Explicit `sigma` arguments select constant-volatility analytic
/// formulas independently of the stored path-generation volatility schedule.
///
/// These low-level constructors do not validate numerical domains. Use finite
/// positive `alpha`: several formulas divide by it and do not implement its
/// zero limit. Bond-option formulas also require positive option time,
/// volatility, and bond strike; zero-variance intrinsic-value limits are not
/// handled here. The separate
/// [`LgmRateModel`](crate::models::lgm::lgmcomponents::LgmRateModel) supports
/// zero mean reversion.
///
/// [`PathGenerator::generate`] uses Euler steps for the Gaussian state, not
/// exact Ornstein-Uhlenbeck transitions. Its short-rate shift is the finite-
/// difference initial forward plus half the recursively accumulated state
/// variance. This path approximation must not be identified with the analytic
/// bond-option formulas or the callable-bond engines' transition schemes.
pub struct HullWhite<'a, T: Scalar> {
    /// Mean-reversion speed.
    alpha: T,
    curve: &'a dyn InterestRatesTermStructure<T>,
    calibration_quality: Option<HullWhiteCalibrationQuality>,
    vol_func: Option<HullWhiteTimeDependentVolatility<T>>,
}

impl<'a, T: Scalar> HullWhite<'a, T> {
    /// Borrows an initial discount curve and stores unvalidated mean reversion.
    ///
    /// No volatility schedule is installed. Calibrate the model or use
    /// [`Self::with_constant_volatility`] before generating paths.
    #[must_use]
    pub fn new(alpha: T, curve: &'a dyn InterestRatesTermStructure<T>) -> Self {
        Self {
            alpha,
            curve,
            calibration_quality: None,
            vol_func: None,
        }
    }

    /// Returns the mean-reversion speed.
    #[must_use]
    pub const fn alpha(&self) -> T {
        self.alpha
    }

    /// Returns a reference to the domestic discount curve.
    #[must_use]
    pub fn curve(&self) -> &dyn InterestRatesTermStructure<T> {
        self.curve
    }

    /// Returns the time-dependent volatility function.
    #[must_use]
    pub const fn vol_func(&self) -> Option<&HullWhiteTimeDependentVolatility<T>> {
        self.vol_func.as_ref()
    }

    /// Returns the calibration quality.
    #[must_use]
    pub fn calibration_quality(&self) -> Option<HullWhiteCalibrationQuality> {
        self.calibration_quality.clone()
    }

    /// Replaces the calibrated volatility function and its quality report together.
    pub fn set_calibration_result(
        &mut self,
        volatility: HullWhiteTimeDependentVolatility<T>,
        quality: HullWhiteCalibrationQuality,
    ) {
        self.vol_func = Some(volatility);
        self.calibration_quality = Some(quality);
    }
}

impl HullWhite<'_, f64> {
    /// Sets a flat absolute short-rate volatility for path generation.
    ///
    /// `sigma` is in decimal-rate units per square-root year, not a Black
    /// relative volatility. This setter does not validate its value.
    #[must_use]
    pub fn with_constant_volatility(mut self, sigma: f64) -> Self {
        self.vol_func = Some(HullWhiteTimeDependentVolatility::new(vec![(0.0, sigma)]));
        self
    }

    /// Computes `B(t,T) = (1 - exp(-alpha*(T-t))) / alpha`.
    #[allow(non_snake_case)]
    #[must_use]
    pub fn B(&self, t: f64, T: f64) -> f64 {
        (1.0 - (-self.alpha * (T - t)).exp()) / self.alpha
    }

    /// Computes `A(t,T)` for the affine ZCB price `P(t,T|r_t) = A(t,T) * exp(-B(t,T)*r_t)`.
    ///
    /// # Errors
    /// Returns an error if discount factor lookup fails.
    #[allow(non_snake_case)]
    pub fn A(
        &self,
        t: f64,
        T: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let b = self.B(t, T);
        let p_0_t = curve.discount_factor_from_time(t)?;
        let p_0_T = curve.discount_factor_from_time(T)?;

        let h = 1.0 / 365.0;
        let p_0_t_h = curve.discount_factor_from_time(t + h)?;
        let f_0_t = -(p_0_t_h / p_0_t).ln() / h;

        let ln_a = (sigma * sigma / (4.0 * self.alpha) * (1.0 - (-2.0 * self.alpha * t).exp()) * b)
            .mul_add(-b, b.mul_add(f_0_t, (p_0_T / p_0_t).ln()));
        Ok(ln_a.exp())
    }

    /// Returns the price of a zero-coupon bond at time `t` maturing at `T`
    /// given the short rate `r_t`, using the provided discount curve.
    ///
    /// # Errors
    /// Returns an error if discount factor lookup fails.
    #[allow(non_snake_case)]
    pub fn zcb_price(
        &self,
        r_t: f64,
        t: f64,
        T: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let a = self.A(t, T, sigma, curve)?;
        Ok(a * (-self.B(t, T) * r_t).exp())
    }

    /// ZCB price volatility used in the Jamshidian caplet / swaption formula.
    #[allow(non_snake_case)]
    #[must_use]
    pub fn zcb_price_volatility(&self, sigma: f64, t: f64, T: f64) -> f64 {
        let b = self.B(t, T);
        sigma * b * ((1.0 - (-2.0 * self.alpha * t).exp()) / (2.0 * self.alpha)).sqrt()
    }

    /// Approximates the drift function theta(t) from the initial curve.
    ///
    /// Uses a fixed `1/365`-year finite-difference increment. Curve coverage is
    /// required through `t + 1/365`, and through `t + 2/365` near the origin.
    ///
    /// # Errors
    /// Returns an error if discount factor lookup fails.
    #[allow(clippy::similar_names)]
    pub fn theta(
        &self,
        t: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let alpha = self.alpha;
        let h = 1.0 / 365.0;

        let df_t = curve.discount_factor_from_time(t)?;
        let df_plus = curve.discount_factor_from_time(t + h)?;
        let f_fwd = -(df_plus / df_t).ln() / h;

        let f_deriv = if t > h {
            let df_minus = curve.discount_factor_from_time(t - h)?;
            let f_bwd = -(df_t / df_minus).ln() / h;
            (f_fwd - f_bwd) / h
        } else {
            // Forward difference for small t.
            let df_plus2 = curve.discount_factor_from_time(2.0f64.mul_add(h, t))?;
            let f_fwd2 = -(df_plus2 / df_plus).ln() / h;
            (f_fwd2 - f_fwd) / h
        };

        Ok((sigma * sigma / (2.0 * alpha)).mul_add(
            1.0 - (-2.0 * alpha * t).exp(),
            alpha.mul_add(f_fwd, f_deriv),
        ))
    }

    /// Conditional variance of the short rate: `Var_t(r_T)` = σ²(1 − e^{−2α(T−t)}) / (2α).
    #[allow(non_snake_case)]
    #[must_use]
    pub fn short_rate_variance(&self, t: f64, T: f64, sigma: f64) -> f64 {
        sigma * sigma * (1.0 - (-2.0 * self.alpha * (T - t)).exp()) / (2.0 * self.alpha)
    }

    /// Price of a zero-coupon bond put at time 0:
    ///   Put(0; `T_opt`, `T_bond`, X) = `X·P(0,T_opt)·Φ(−d₂)` − `P(0,T_bond)·Φ(−d₁)`
    /// where `σ_P` = `σ·B(T_opt,T_bond)·√((1−e^{−2αT_opt})/(2α))`.
    ///
    /// # Errors
    /// Returns an error if discount factor lookup fails.
    #[allow(non_snake_case)]
    pub fn bond_put_price(
        &self,
        t_option: f64,
        t_bond: f64,
        strike_bond: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let p_0_t = curve.discount_factor_from_time(t_option)?;
        let p_0_s = curve.discount_factor_from_time(t_bond)?;
        let sigma_p = self.zcb_price_volatility(sigma, t_option, t_bond);
        let d1 = (0.5 * sigma_p).mul_add(sigma_p, (p_0_s / (strike_bond * p_0_t)).ln()) / sigma_p;
        let d2 = d1 - sigma_p;
        Ok((strike_bond * p_0_t).mul_add(norm_cdf(-d2), -(p_0_s * norm_cdf(-d1))))
    }

    /// Price of a zero-coupon bond call at time 0:
    ///   Call(0; `T_opt`, `T_bond`, X) = `P(0,T_bond)·Φ(d₁)` − `X·P(0,T_opt)·Φ(d₂)`
    ///
    /// # Errors
    /// Returns an error if discount factor lookup fails.
    #[allow(non_snake_case)]
    pub fn bond_call_price(
        &self,
        t_option: f64,
        t_bond: f64,
        strike_bond: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let p_0_t = curve.discount_factor_from_time(t_option)?;
        let p_0_s = curve.discount_factor_from_time(t_bond)?;
        let sigma_p = self.zcb_price_volatility(sigma, t_option, t_bond);
        let d1 = (0.5 * sigma_p).mul_add(sigma_p, (p_0_s / (strike_bond * p_0_t)).ln()) / sigma_p;
        let d2 = d1 - sigma_p;
        Ok(p_0_s.mul_add(norm_cdf(d1), -(strike_bond * p_0_t * norm_cdf(d2))))
    }

    /// Caplet price under the Hull-White model at time 0.
    ///
    /// Uses the bond-option representation:
    ///   Caplet(0) = (1 + δK) · BondPut(0; T, S, X)
    /// where T = reset date (option expiry), S = T + δ (payment date),
    /// δ = S − T (accrual period), K = strike rate, X = 1/(1+δK).
    ///
    /// # Errors
    /// Returns an error if the underlying bond put pricing fails.
    #[allow(non_snake_case)]
    pub fn caplet_price(
        &self,
        strike: f64,
        t: f64,
        S: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let tau = S - t;
        let x = 1.0 / tau.mul_add(strike, 1.0);
        let put = self.bond_put_price(t, S, x, sigma, curve)?;
        Ok(tau.mul_add(strike, 1.0) * put)
    }

    /// Floorlet price under the Hull-White model at time 0.
    ///
    /// Uses the bond-option representation:
    ///   Floorlet(0) = (1 + δK) · BondCall(0; T, S, X)
    /// where X = 1/(1+δK).
    ///
    /// # Errors
    /// Returns an error if the underlying bond call pricing fails.
    #[allow(non_snake_case)]
    pub fn floorlet_price(
        &self,
        strike: f64,
        t: f64,
        S: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let tau = S - t;
        let x = 1.0 / tau.mul_add(strike, 1.0);
        let call = self.bond_call_price(t, S, x, sigma, curve)?;
        Ok(tau.mul_add(strike, 1.0) * call)
    }

    /// Swaption price via Jamshidian decomposition.
    ///
    /// For a payer swaption on a swap with fixed rate K, payment dates
    /// `swap_schedule[0..n]`, and accrual fractions `tau_i`, the price
    /// is decomposed into a portfolio of zero-coupon bond options:
    ///   Swaption(0) = Σ `c_i` · BondPut(0; `T_opt`, `T_i`, `X_i`)
    /// where `X_i` = `P(T_opt`, `T_i` | r*) via the critical short rate r*
    /// that makes the swap value zero.
    ///
    /// # Errors
    /// Returns an error when a discount-factor lookup in the final bond-option
    /// valuation fails. The critical-rate search expands a bracket and takes
    /// 200 bisection steps without reporting a convergence status; failed curve
    /// lookups inside its objective are replaced by zero. Callers must ensure
    /// admissible cashflows and complete curve coverage. An empty schedule
    /// returns zero.
    #[allow(non_snake_case)]
    pub fn swaption_price(
        &self,
        strike: f64,
        t_option: f64,
        swap_schedule: &[(f64, f64)],
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        // swap_schedule: Vec of (payment_time, accrual_fraction)
        // Step 1: find r* such that sum_i c_i P(t_opt, T_i | r*) = 1
        //   where c_i = tau_i * K for i < n, c_n = 1 + tau_n * K
        let n = swap_schedule.len();
        if n == 0 {
            return Ok(0.0);
        }

        let mut cashflows = Vec::with_capacity(n);
        for (i, &(t_i, tau_i)) in swap_schedule.iter().enumerate() {
            let c = if i == n - 1 {
                tau_i.mul_add(strike, 1.0)
            } else {
                tau_i * strike
            };
            cashflows.push((t_i, c));
        }

        // Bisection to find r* such that sum c_i A(t,T_i) exp(-B(t,T_i) r*) = 1.
        // The objective is monotone decreasing in r, so first expand the
        // bracket until it contains the root (extreme trial sigmas during
        // calibration can push r* far outside a fixed interval).
        let eval = |r: f64| -> f64 {
            cashflows
                .iter()
                .map(|&(t_i, c_i)| {
                    let a = self.A(t_option, t_i, sigma, curve).unwrap_or(0.0);
                    c_i * a * (-self.B(t_option, t_i) * r).exp()
                })
                .sum()
        };
        let mut lo = -0.5_f64;
        let mut hi = 0.5_f64;
        for _ in 0..60 {
            if eval(lo) >= 1.0 {
                break;
            }
            lo *= 2.0;
        }
        for _ in 0..60 {
            if eval(hi) <= 1.0 {
                break;
            }
            hi *= 2.0;
        }
        for _ in 0..200 {
            let mid = f64::midpoint(lo, hi);
            if eval(mid) > 1.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let r_star = f64::midpoint(lo, hi);

        // Step 2: compute bond option strikes X_i = P(t_opt, T_i | r*)
        // and sum up the bond puts
        let mut total = 0.0;
        for &(t_i, c_i) in &cashflows {
            let x_i =
                self.A(t_option, t_i, sigma, curve)? * (-self.B(t_option, t_i) * r_star).exp();
            total = f64::mul_add(
                c_i,
                self.bond_put_price(t_option, t_i, x_i, sigma, curve)?,
                total,
            );
        }
        Ok(total)
    }

    /// Computes the instantaneous forward rate f(0,t) from the discount curve
    /// via finite differences.
    fn forward_rate_from_curve(&self, t: f64) -> Result<f64> {
        let h = 1.0 / 365.0;
        let df_t = self.curve.discount_factor_from_time(t)?;
        let df_plus = self.curve.discount_factor_from_time(t + h)?;
        Ok(-(df_plus / df_t).ln() / h)
    }
}

impl PathGenerator<f64> for HullWhite<'_, f64> {
    /// Generates Euler-discretized short rates at nondecreasing year fractions.
    ///
    /// `draws[i]` is a standard-normal innovation and `scenario[i]` receives
    /// the short rate at `times[i]`. Inputs must be finite, with nonnegative
    /// times and positive mean reversion.
    ///
    /// # Errors
    /// Returns an error if no volatility schedule is installed or if a
    /// volatility or forward-curve lookup fails.
    ///
    /// # Panics
    /// Panics if `draws` or `scenario` is shorter than `times`. Unlike some
    /// path generators, this implementation does not check buffer lengths.
    fn generate(&self, times: &[f64], draws: &[f64], scenario: &mut [f64]) -> Result<()> {
        let alpha = self.alpha;
        let vol_func = self.vol_func.as_ref().ok_or_else(|| {
            crate::utils::errors::QSError::InvalidValueErr(
                "HullWhite: vol_func not set (calibrate first)".into(),
            )
        })?;
        let mut x_t = 0.0_f64;
        let mut t_prev = 0.0;
        let mut var_x = 0.0_f64;

        for (i, &t) in times.iter().enumerate() {
            let dt = t - t_prev;
            let sigma_t = vol_func.vol(t)?;

            let decay = (-2.0 * alpha * dt).exp();
            var_x = var_x.mul_add(decay, sigma_t * sigma_t * (1.0 - decay) / (2.0 * alpha));

            let dw = draws[i] * sigma_t * dt.sqrt();
            x_t += (-alpha * x_t).mul_add(dt, dw);

            let f_0_t = self.forward_rate_from_curve(t)?;
            let phi_t = 0.5f64.mul_add(var_x, f_0_t);

            scenario[i] = x_t + phi_t;
            t_prev = t;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::{
        math::{interpolation::interpolator::Interpolator, random::fill_std_normal},
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
    };

    const RATE: f64 = 0.03;
    const ALPHA: f64 = 0.1;
    const SIGMA: f64 = 0.01;

    /// Flat continuously-compounded curve; log-linear interpolation makes the
    /// discount factors exact at every time.
    fn flat_curve() -> Result<DiscountTermStructure<f64>> {
        let reference_date = Date::new(2025, 1, 2);
        let dc = DayCounter::Actual365;
        let dates = vec![
            reference_date,
            reference_date + Period::new(1, TimeUnit::Years),
            reference_date + Period::new(40, TimeUnit::Years),
        ];
        let dfs: Vec<f64> = dates
            .iter()
            .map(|d| (-RATE * dc.year_fraction(reference_date, *d)).exp())
            .collect();
        DiscountTermStructure::<f64>::new(dates, dfs, dc, Interpolator::LogLinear, true)
    }

    #[test]
    fn zcb_price_reproduces_curve_at_time_zero() -> Result<()> {
        // P(0,T | r_0 = f(0,0)) must reproduce the input curve for any sigma.
        let curve = flat_curve()?;
        let hw = HullWhite::new(ALPHA, &curve);
        for t_bond in [0.25, 1.0, 5.0, 20.0] {
            for sigma in [1e-6, SIGMA, 0.05] {
                let model_df = hw.zcb_price(RATE, 0.0, t_bond, sigma, &curve)?;
                let curve_df = curve.discount_factor_from_time(t_bond)?;
                assert!(
                    (model_df - curve_df).abs() < 1e-10,
                    "P(0,{t_bond}) mismatch: model {model_df} vs curve {curve_df} (sigma {sigma})"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn bond_call_put_parity() -> Result<()> {
        // Call - Put = P(0,S) - X * P(0,T) for all strikes and maturities.
        let curve = flat_curve()?;
        let hw = HullWhite::new(ALPHA, &curve);
        for (t_option, t_bond) in [(0.5, 0.75), (1.0, 2.0), (5.0, 10.0)] {
            for strike_bond in [0.5, 0.9, 0.99, 1.1] {
                let call = hw.bond_call_price(t_option, t_bond, strike_bond, SIGMA, &curve)?;
                let put = hw.bond_put_price(t_option, t_bond, strike_bond, SIGMA, &curve)?;
                let p_s = curve.discount_factor_from_time(t_bond)?;
                let p_t = curve.discount_factor_from_time(t_option)?;
                assert!(
                    (call - put - strike_bond.mul_add(-p_t, p_s)).abs() < 1e-14,
                    "parity violated at T_opt={t_option}, T_bond={t_bond}, X={strike_bond}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn caplet_floorlet_parity_matches_fra_value() -> Result<()> {
        // Caplet - Floorlet = discounted FRA payoff df(S) * tau * (fwd - K).
        let curve = flat_curve()?;
        let hw = HullWhite::new(ALPHA, &curve);
        for (t, s) in [(0.5, 0.75), (2.0, 2.5), (10.0, 11.0)] {
            let tau = s - t;
            let df_t = curve.discount_factor_from_time(t)?;
            let df_s = curve.discount_factor_from_time(s)?;
            let fwd = (df_t / df_s - 1.0) / tau;
            for strike in [0.5 * fwd, fwd, 2.0 * fwd] {
                let caplet = hw.caplet_price(strike, t, s, SIGMA, &curve)?;
                let floorlet = hw.floorlet_price(strike, t, s, SIGMA, &curve)?;
                let fra = df_s * tau * (fwd - strike);
                assert!(
                    (caplet - floorlet - fra).abs() < 1e-13,
                    "parity violated at t={t}, S={s}, K={strike}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn caplet_boundary_conditions_and_monotonicity() -> Result<()> {
        let curve = flat_curve()?;
        let hw = HullWhite::new(ALPHA, &curve);
        let (t, s) = (1.0, 1.25);
        let tau = s - t;
        let df_t = curve.discount_factor_from_time(t)?;
        let df_s = curve.discount_factor_from_time(s)?;
        let fwd = (df_t / df_s - 1.0) / tau;

        // sigma -> 0: ITM caplet converges to intrinsic, OTM to zero.
        let itm_strike = 0.5 * fwd;
        let intrinsic = df_s * tau * (fwd - itm_strike);
        let itm = hw.caplet_price(itm_strike, t, s, 1e-8, &curve)?;
        assert!(
            (itm - intrinsic).abs() < 1e-10,
            "low-vol ITM caplet {itm} should equal intrinsic {intrinsic}"
        );
        assert!(hw.caplet_price(2.0 * fwd, t, s, 1e-8, &curve)? < 1e-14);

        // Deep OTM (extreme strike): negligible price at market vols.
        assert!(hw.caplet_price(0.5, t, s, SIGMA, &curve)? < 1e-12);

        // Price is increasing in sigma.
        let mut prev = 0.0;
        for sigma in [0.001, 0.005, 0.01, 0.02, 0.05] {
            let price = hw.caplet_price(fwd, t, s, sigma, &curve)?;
            assert!(price > prev, "caplet price must increase in sigma");
            prev = price;
        }
        Ok(())
    }

    #[test]
    fn single_period_swaption_degenerates_to_caplet() -> Result<()> {
        // Jamshidian with one payment must reproduce the bond-put caplet.
        let curve = flat_curve()?;
        let hw = HullWhite::new(ALPHA, &curve);
        let (t, s) = (1.0, 2.0);
        let tau = s - t;
        for strike in [0.01, 0.03, 0.06] {
            let swaption = hw.swaption_price(strike, t, &[(s, tau)], SIGMA, &curve)?;
            let caplet = hw.caplet_price(strike, t, s, SIGMA, &curve)?;
            assert!(
                (swaption - caplet).abs() < 1e-12,
                "single-period swaption {swaption} != caplet {caplet} at K={strike}"
            );
        }
        Ok(())
    }

    #[test]
    fn swaption_boundary_conditions() -> Result<()> {
        let curve = flat_curve()?;
        let hw = HullWhite::new(ALPHA, &curve);
        let t_option = 1.0;
        let schedule: Vec<(f64, f64)> = (1..=5).map(|i| (t_option + f64::from(i), 1.0)).collect();

        // Empty schedule prices to zero.
        assert!(hw.swaption_price(0.03, t_option, &[], SIGMA, &curve)?.abs() < 1e-15);

        // sigma -> 0: payer swaption converges to the forward swap intrinsic
        // max(P(0,t_opt) - sum c_i P(0,T_i), 0).
        for strike in [0.01_f64, 0.05] {
            let mut coupon_bond = 0.0;
            let n = schedule.len();
            for (i, &(t_i, tau_i)) in schedule.iter().enumerate() {
                let c = if i == n - 1 {
                    tau_i.mul_add(strike, 1.0)
                } else {
                    tau_i * strike
                };
                coupon_bond += c * curve.discount_factor_from_time(t_i)?;
            }
            let intrinsic = (curve.discount_factor_from_time(t_option)? - coupon_bond).max(0.0);
            let price = hw.swaption_price(strike, t_option, &schedule, 1e-8, &curve)?;
            assert!(
                (price - intrinsic).abs() < 1e-9,
                "low-vol swaption {price} should equal intrinsic {intrinsic} at K={strike}"
            );
        }

        // Extreme strike: deep OTM payer swaption is worthless.
        assert!(hw.swaption_price(0.20, t_option, &schedule, SIGMA, &curve)? < 1e-10);

        // Monotone increasing in sigma at the money-ish strike.
        let mut prev = 0.0;
        for sigma in [0.002, 0.005, 0.01, 0.02] {
            let price = hw.swaption_price(RATE, t_option, &schedule, sigma, &curve)?;
            assert!(price > prev, "swaption price must increase in sigma");
            prev = price;
        }
        Ok(())
    }

    #[test]
    fn analytic_factor_limits() {
        let curve = flat_curve().expect("curve");
        let hw = HullWhite::new(ALPHA, &curve);
        // B(t,t) = 0; B(t,inf) -> 1/alpha.
        assert!(hw.B(2.0, 2.0).abs() < 1e-15);
        assert!((hw.B(0.0, 1e6) - 1.0 / ALPHA).abs() < 1e-10);
        // Var(t,t) = 0; long-horizon variance -> sigma^2 / (2 alpha).
        assert!(hw.short_rate_variance(3.0, 3.0, SIGMA).abs() < 1e-18);
        let limit = SIGMA * SIGMA / (2.0 * ALPHA);
        assert!((hw.short_rate_variance(0.0, 1e6, SIGMA) - limit).abs() < 1e-15);
        // ZCB vol is zero at t=0 (no time for randomness) and for T=t.
        assert!(hw.zcb_price_volatility(SIGMA, 0.0, 5.0).abs() < 1e-15);
        assert!(hw.zcb_price_volatility(SIGMA, 2.0, 2.0).abs() < 1e-15);
    }

    #[test]
    fn generate_requires_volatility() {
        let curve = flat_curve().expect("curve");
        let hw = HullWhite::new(ALPHA, &curve);
        let times = [1.0];
        let draws = [0.0];
        let mut scenario = [0.0];
        assert!(hw.generate(&times, &draws, &mut scenario).is_err());
    }

    #[test]
    fn generated_paths_reproduce_curve_and_moments() -> Result<()> {
        // Strong correctness test of the simulation drift (phi) and variance:
        //   E[exp(-int r dt)] must reproduce P(0,T);
        //   E[r_T] ~ f(0,T) + V(T)/2;  Var[r_T] ~ short_rate_variance(0,T).
        let curve = flat_curve()?;
        let hw = HullWhite::new(ALPHA, &curve).with_constant_volatility(SIGMA);

        let horizon = 2.0_f64;
        let n_steps = 24_usize;
        let n_paths = 20_000_i32;
        let times: Vec<f64> = (1..=n_steps)
            .map(|i| horizon * f64::from(u32::try_from(i).unwrap_or(u32::MAX)) / 24.0)
            .collect();
        let dt = horizon / 24.0;

        let mut rng = StdRng::seed_from_u64(42);
        let mut draws = vec![0.0_f64; n_steps];
        let mut scenario = vec![0.0_f64; n_steps];

        let mut sum_df = 0.0;
        let mut sum_r = 0.0;
        let mut sum_r2 = 0.0;
        for _ in 0..n_paths {
            fill_std_normal(&mut rng, &mut draws);
            hw.generate(&times, &draws, &mut scenario)?;
            // Trapezoid integral of the short rate, starting from r(0) = f(0,0).
            let mut integral = 0.5 * (RATE + scenario[0]) * dt;
            for w in scenario.windows(2) {
                integral += 0.5 * (w[0] + w[1]) * dt;
            }
            sum_df += (-integral).exp();
            let r_end = scenario[scenario.len() - 1];
            sum_r += r_end;
            sum_r2 += r_end * r_end;
            assert!(scenario.iter().all(|r| r.is_finite()));
        }
        let n = f64::from(n_paths);
        let mc_df = sum_df / n;
        let mean_r = sum_r / n;
        let var_r = sum_r2 / n - mean_r * mean_r;

        let curve_df = curve.discount_factor_from_time(horizon)?;
        assert!(
            (mc_df - curve_df).abs() / curve_df < 0.01,
            "MC discount factor {mc_df} should reproduce curve {curve_df}"
        );

        let theo_var = hw.short_rate_variance(0.0, horizon, SIGMA);
        let theo_mean = RATE + 0.5 * theo_var;
        assert!(
            (mean_r - theo_mean).abs() < 5e-4,
            "mean short rate {mean_r} should be near {theo_mean}"
        );
        assert!(
            (var_r - theo_var).abs() / theo_var < 0.05,
            "short-rate variance {var_r} should be near {theo_var}"
        );
        Ok(())
    }
}

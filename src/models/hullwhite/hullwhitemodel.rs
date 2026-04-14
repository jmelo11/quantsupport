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

/// Parameters for the Hull-White (one-factor) short-rate model.
pub struct HullWhite<'a, T: Scalar> {
    /// Mean-reversion speed.
    alpha: T,
    curve: &'a dyn InterestRatesTermStructure<T>,
    pub(crate) calibration_quality: Option<HullWhiteCalibrationQuality>,
    pub(crate) vol_func: Option<HullWhiteTimeDependentVolatility<T>>,
}

impl<'a, T: Scalar> HullWhite<'a, T> {
    /// Creates new Hull-White parameters.
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
}

impl HullWhite<'_, f64> {
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

    /// Computes the drift function theta(t) from the initial curve.
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
    /// Returns an error if discount factor lookups or root-finding fails.
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

        // Bisection to find r* such that sum c_i A(t,T_i) exp(-B(t,T_i) r*) = 1
        let mut lo = -0.5_f64;
        let mut hi = 0.5_f64;
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            let val: f64 = cashflows
                .iter()
                .map(|&(t_i, c_i)| {
                    let a = self.A(t_option, t_i, sigma, curve).unwrap_or(0.0);
                    c_i * a * (-self.B(t_option, t_i) * mid).exp()
                })
                .sum();
            if val > 1.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let r_star = 0.5 * (lo + hi);

        // Step 2: compute bond option strikes X_i = P(t_opt, T_i | r*)
        // and sum up the bond puts
        let mut total = 0.0;
        for &(t_i, c_i) in &cashflows {
            let x_i =
                self.A(t_option, t_i, sigma, curve)? * (-self.B(t_option, t_i) * r_star).exp();
            total += c_i * self.bond_put_price(t_option, t_i, x_i, sigma, curve)?;
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

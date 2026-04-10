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
    pub fn alpha(&self) -> T {
        self.alpha
    }

    /// Returns a reference to the domestic discount curve.
    #[must_use]
    pub fn curve(&self) -> &dyn InterestRatesTermStructure<T> {
        self.curve
    }

    /// Returns the time-dependent volatility function.
    #[must_use]
    pub fn vol_func(&self) -> Option<&HullWhiteTimeDependentVolatility<T>> {
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

        let ln_a = (p_0_T / p_0_t).ln() + b * f_0_t
            - sigma * sigma / (4.0 * self.alpha) * (1.0 - (-2.0 * self.alpha * t).exp()) * b * b;
        Ok(ln_a.exp())
    }

    /// Returns the price of a zero-coupon bond at time `t` maturing at `T`
    /// given the short rate `r_t`, using the provided discount curve.
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
            let df_plus2 = curve.discount_factor_from_time(t + 2.0 * h)?;
            let f_fwd2 = -(df_plus2 / df_plus).ln() / h;
            (f_fwd2 - f_fwd) / h
        };

        Ok(f_deriv
            + alpha * f_fwd
            + sigma * sigma / (2.0 * alpha) * (1.0 - (-2.0 * alpha * t).exp()))
    }

    /// Conditional variance of the short rate: Var_t(r_T) = σ²(1 − e^{−2α(T−t)}) / (2α).
    #[allow(non_snake_case)]
    #[must_use]
    pub fn short_rate_variance(&self, t: f64, T: f64, sigma: f64) -> f64 {
        sigma * sigma * (1.0 - (-2.0 * self.alpha * (T - t)).exp()) / (2.0 * self.alpha)
    }

    /// Price of a zero-coupon bond put at time 0:
    ///   Put(0; T_opt, T_bond, X) = X·P(0,T_opt)·Φ(−d₂) − P(0,T_bond)·Φ(−d₁)
    /// where σ_P = σ·B(T_opt,T_bond)·√((1−e^{−2αT_opt})/(2α)).
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
        let d1 = ((p_0_s / (strike_bond * p_0_t)).ln() + 0.5 * sigma_p * sigma_p) / sigma_p;
        let d2 = d1 - sigma_p;
        Ok(strike_bond * p_0_t * norm_cdf(-d2) - p_0_s * norm_cdf(-d1))
    }

    /// Price of a zero-coupon bond call at time 0:
    ///   Call(0; T_opt, T_bond, X) = P(0,T_bond)·Φ(d₁) − X·P(0,T_opt)·Φ(d₂)
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
        let d1 = ((p_0_s / (strike_bond * p_0_t)).ln() + 0.5 * sigma_p * sigma_p) / sigma_p;
        let d2 = d1 - sigma_p;
        Ok(p_0_s * norm_cdf(d1) - strike_bond * p_0_t * norm_cdf(d2))
    }

    /// Caplet price under the Hull-White model at time 0.
    ///
    /// Uses the bond-option representation:
    ///   Caplet(0) = (1 + δK) · BondPut(0; T, S, X)
    /// where T = reset date (option expiry), S = T + δ (payment date),
    /// δ = S − T (accrual period), K = strike rate, X = 1/(1+δK).
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
        let x = 1.0 / (1.0 + tau * strike);
        let put = self.bond_put_price(t, S, x, sigma, curve)?;
        Ok((1.0 + tau * strike) * put)
    }

    /// Floorlet price under the Hull-White model at time 0.
    ///
    /// Uses the bond-option representation:
    ///   Floorlet(0) = (1 + δK) · BondCall(0; T, S, X)
    /// where X = 1/(1+δK).
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
        let x = 1.0 / (1.0 + tau * strike);
        let call = self.bond_call_price(t, S, x, sigma, curve)?;
        Ok((1.0 + tau * strike) * call)
    }

    /// Swaption price via Jamshidian decomposition.
    ///
    /// For a payer swaption on a swap with fixed rate K, payment dates
    /// `swap_schedule[0..n]`, and accrual fractions `tau_i`, the price
    /// is decomposed into a portfolio of zero-coupon bond options:
    ///   Swaption(0) = Σ c_i · BondPut(0; T_opt, T_i, X_i)
    /// where X_i = P(T_opt, T_i | r*) via the critical short rate r*
    /// that makes the swap value zero.
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
                1.0 + tau_i * strike
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
            var_x = var_x * decay + sigma_t * sigma_t * (1.0 - decay) / (2.0 * alpha);

            let dw = draws[i] * sigma_t * dt.sqrt();
            x_t += -alpha * x_t * dt + dw;

            let f_0_t = self.forward_rate_from_curve(t)?;
            let phi_t = f_0_t + 0.5 * var_x;

            scenario[i] = x_t + phi_t;
            t_prev = t;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        math::interpolation::interpolator::Interpolator,
        models::{
            hullwhite::hullwhitecalibration::HullWhiteTimeDependentVolatility, utils::black_call,
        },
        quotes::quote::Level,
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
    };
    use rand::rngs::StdRng;
    use rand::Rng;
    use rand::SeedableRng;

    /// Box-Muller standard normal sample.
    fn std_normal(rng: &mut impl Rng) -> f64 {
        let u1: f64 = rng.gen_range(f64::EPSILON..1.0);
        let u2: f64 = rng.gen_range(0.0..std::f64::consts::TAU);
        (-2.0 * u1.ln()).sqrt() * u2.cos()
    }

    /// Builds a flat discount curve DF(t) = exp(-r * t).
    fn flat_curve(ref_date: Date, rate: f64, dc: DayCounter) -> DiscountTermStructure<f64> {
        let mut dates = vec![ref_date];
        let mut dfs = vec![1.0];
        for y in 1..=30 {
            let d = ref_date.advance(y, TimeUnit::Years);
            let t = dc.year_fraction(ref_date, d);
            dates.push(d);
            dfs.push((-rate * t).exp());
        }
        DiscountTermStructure::<f64>::new(dates, dfs, dc, Interpolator::LogLinear, true)
            .expect("flat curve")
    }

    /// MC caplet: verify put-call parity and positivity.
    ///
    /// Uses a flat curve; theta(t) is computed on-the-fly by the path generator.
    #[test]
    fn hw_mc_caplet_put_call_parity() {
        let alpha = 0.1_f64;
        let sigma = 0.02;
        let r0 = 0.04;
        let strike = 0.05;

        let ref_date = Date::new(2025, 1, 1);
        let reset_date = Date::new(2026, 1, 1);
        let pay_date = Date::new(2026, 7, 1);
        let dc = DayCounter::Actual365;

        let t1 = dc.year_fraction(ref_date, reset_date);
        let t2 = dc.year_fraction(ref_date, pay_date);
        let tau = t2 - t1;

        let n_steps = 100;

        let curve = flat_curve(ref_date, r0, dc);
        let vol_func = HullWhiteTimeDependentVolatility::new(vec![(0.0, sigma)]);
        let mut hw = HullWhite::new(alpha, &curve);
        hw.vol_func = Some(vol_func);

        let mut times_f64 = Vec::with_capacity(n_steps);
        for i in 1..=n_steps {
            let frac = i as f64 / n_steps as f64;
            times_f64.push(t1 * frac);
        }

        let n_paths = 500_000;
        let mut rng = StdRng::seed_from_u64(42);
        let dt_step = t1 / n_steps as f64;

        let mut caplet_sum = 0.0;
        let mut floorlet_sum = 0.0;
        let mut fwd_sum = 0.0;
        let mut draws = vec![0.0_f64; n_steps];
        let mut scenario = vec![0.0_f64; n_steps];

        for _ in 0..n_paths {
            for d in &mut draws {
                *d = std_normal(&mut rng);
            }
            hw.generate(&times_f64, &draws, &mut scenario).unwrap();

            let r_t1 = scenario[n_steps - 1];

            // Discount 0 -> t1 via trapezoidal rule.
            // r(0) = phi(0) = f(0,0) = r0 for a flat curve.
            let mut integral = r0 * dt_step * 0.5;
            for j in 0..n_steps - 1 {
                integral += scenario[j] * dt_step;
            }
            integral += scenario[n_steps - 1] * dt_step * 0.5;
            let df_01 = (-integral).exp();

            let zcb = hw.zcb_price(r_t1, t1, t2, sigma, &curve).unwrap();
            let fwd_rate = (1.0 / zcb - 1.0) / tau;

            caplet_sum += (fwd_rate - strike).max(0.0) * tau * zcb * df_01;
            floorlet_sum += (strike - fwd_rate).max(0.0) * tau * zcb * df_01;
            fwd_sum += (fwd_rate - strike) * tau * zcb * df_01;
        }

        #[allow(clippy::cast_precision_loss)]
        let n = n_paths as f64;
        let mc_caplet = caplet_sum / n;
        let mc_floorlet = floorlet_sum / n;
        let mc_fwd_minus_k = fwd_sum / n;

        assert!(mc_caplet > 0.0, "caplet must be positive: {mc_caplet}");
        assert!(
            mc_floorlet > 0.0,
            "floorlet must be positive: {mc_floorlet}"
        );

        let parity_err = (mc_caplet - mc_floorlet - mc_fwd_minus_k).abs();
        assert!(
            parity_err < 1e-6,
            "Put-call parity violated: caplet {mc_caplet:.6} - floorlet {mc_floorlet:.6} \
             != fwd-K {mc_fwd_minus_k:.6}, err {parity_err:.2e}"
        );
    }

    /// Simulated short rates should mean-revert toward the flat forward rate.
    #[test]
    fn hw_mean_reversion() {
        let alpha = 0.5_f64;
        let sigma = 0.005;
        let flat_rate = 0.05;

        let ref_date = Date::new(2025, 1, 1);
        let dc = DayCounter::Actual365;

        let n_steps = 120;
        let mut times_f64 = Vec::with_capacity(n_steps);
        for i in 1..=n_steps {
            let d = ref_date.advance(i as i32, TimeUnit::Months);
            times_f64.push(dc.year_fraction(ref_date, d));
        }

        let curve = flat_curve(ref_date, flat_rate, dc);
        let vol_func = HullWhiteTimeDependentVolatility::new(vec![(0.0, sigma)]);
        let mut hw = HullWhite::new(alpha, &curve);
        hw.vol_func = Some(vol_func);

        let n_paths = 100_000;
        let mut rng = StdRng::seed_from_u64(99);
        let mut draws = vec![0.0_f64; n_steps];
        let mut scenario = vec![0.0_f64; n_steps];
        let mut terminal_sum = 0.0;

        for _ in 0..n_paths {
            for d in &mut draws {
                *d = std_normal(&mut rng);
            }
            hw.generate(&times_f64, &draws, &mut scenario).unwrap();
            terminal_sum += scenario[n_steps - 1];
        }

        #[allow(clippy::cast_precision_loss)]
        let mean_terminal = terminal_sum / n_paths as f64;

        let abs_err = (mean_terminal - flat_rate).abs();
        assert!(
            abs_err < 0.005,
            "Mean terminal rate {mean_terminal:.4} should be near \
             flat rate {flat_rate}, err {abs_err:.4}"
        );
    }

    /// Round-trip calibration: compute HW caplet prices with a known sigma,
    /// express as Black vols, create CapletFloorlet quotes in a QuoteStore,
    /// calibrate back via the quote-driven calibrate(), and verify recovery
    /// of the original sigma.
    /// Also verifies the IFT sensitivity matrix diagonal is non-zero.
    #[test]
    fn hw_calibrate_round_trip_via_quotes() {
        use crate::quotes::{
            quote::{Quote, QuoteDetails, QuoteLevels},
            quotestore::QuoteStore,
        };
        use std::str::FromStr as _;

        let alpha = 0.1_f64;
        let r0 = 0.03;
        let true_sigma = 0.015;
        let strike = 0.03; // ATM strike (equal to flat rate)

        let ref_date = Date::new(2025, 1, 1);
        let dc = DayCounter::Actual365;
        let curve = flat_curve(ref_date, r0, dc);
        let dummy_vol = HullWhiteTimeDependentVolatility::new(vec![(0.0, true_sigma)]);
        let mut hw_model = HullWhite::new(alpha, &curve);
        hw_model.vol_func = Some(dummy_vol);

        let expiry_labels = ["6M", "1Y", "18M", "2Y"];
        let idx_tenor = Period::from_str("6M").unwrap();

        // Build a QuoteStore with synthetic CapletFloorlet quotes.
        let mut store = QuoteStore::new(ref_date);
        let mut quote_ids = Vec::new();

        for exp_label in &expiry_labels {
            let expiry = Period::from_str(exp_label).unwrap();
            let exp_date = ref_date + expiry;
            let pay_date = exp_date + idx_tenor;
            let t = dc.year_fraction(ref_date, exp_date);
            let big_t = dc.year_fraction(ref_date, pay_date);
            let tau = big_t - t;

            let df_start = curve.discount_factor(exp_date).unwrap();
            let df_end = curve.discount_factor(pay_date).unwrap();
            let fwd = (df_start / df_end - 1.0) / tau;

            // HW caplet price -> invert Black for market vol
            let sigma_p = hw_model.zcb_price_volatility(true_sigma, t, big_t);
            let hw_price = df_end * tau * black_call(fwd, strike, sigma_p, t).unwrap();
            let scaled = hw_price / (df_end * tau);

            let mut lo = 1e-6_f64;
            let mut hi = 5.0_f64;
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if black_call(fwd, strike, mid, t).unwrap() > scaled {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let mkt_vol = 0.5 * (lo + hi);

            // Build quote identifier:
            // CapletFloorlet_USD_SOFR_6M_{expiry}_Absolute_{strike}_Straddle_Black
            let id = format!(
                "CapletFloorlet_USD_SOFR_6M_{exp_label}_Absolute_{strike:.3}_Straddle_Black"
            );
            let details = QuoteDetails::from_str(&id).unwrap();
            let levels = QuoteLevels::with_mid(mkt_vol);
            store.add_quote(Quote::new(details, levels));
            quote_ids.push(id);
        }

        // Calibrate
        hw_model
            .calibrate(&quote_ids, &store, &curve, Level::Mid)
            .expect("calibration should converge");

        let result = hw_model
            .vol_func()
            .expect("vol_func should be set after calibrate");
        assert_eq!(result.len(), expiry_labels.len());

        for (i, &(t, calibrated_sigma)) in result.iter().enumerate() {
            let err = (calibrated_sigma - true_sigma).abs();
            assert!(
                err < 1e-4,
                "caplet {i} (t={t:.2}): calibrated sigma {calibrated_sigma:.6} \
                 vs true {true_sigma}, err {err:.2e}"
            );
        }

        // Verify IFT matrix diagonal is non-zero.
        let ift = result.ift_sensitivities().expect("IFT should be present");
        for i in 0..ift.len() {
            assert!(
                ift[i][i].abs() > 0.01,
                "diagonal element [{i}][{i}] should be nonzero: {}",
                ift[i][i]
            );
        }
    }
}

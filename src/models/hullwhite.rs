use crate::{
    ad::{dual::DualFwd, expr::FloatExt, scalar::Scalar},
    math::{
        probability::norm_cdf::norm_cdf,
        solvers::{bisection::Bisection, solvertraits::ContFunc},
    },
    models::montecarloengine::{PathGenerator, TimeDependentVolatility},
    rates::{
        bootstrapping::calibrationinstrument::CalibrationInstrument,
        yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    },
    time::daycounter::DayCounter,
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::VolatilityType,
};

/// Parameters for the Hull-White (one-factor) short-rate model.
#[derive(Clone, Debug)]
pub struct HullWhite<T: Scalar> {
    /// Mean-reversion speed.
    alpha: T,
    /// Volatility convention used for calibration (Black or Normal).
    volatility_type: VolatilityType,
}

/// Piecewise-constant time-dependent volatility for the Hull-White model.
pub struct HullWhiteTimeDependentVolatility<T: Scalar> {
    schedule: Vec<(f64, T)>,
}

impl HullWhiteTimeDependentVolatility<f64> {
    /// Creates a new time-dependent volatility function from a schedule of
    /// `(year_fraction, sigma)` pairs. The schedule must be sorted by time.
    #[must_use]
    pub fn new(schedule: Vec<(f64, f64)>) -> Self {
        Self { schedule }
    }

    /// Returns the number of schedule entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.schedule.len()
    }

    /// Returns true if the schedule is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
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

impl<T> HullWhite<T>
where
    T: Scalar,
{
    /// Creates new Hull-White parameters.
    #[must_use]
    pub fn new(alpha: T, volatility_type: VolatilityType) -> Self {
        Self {
            alpha,
            volatility_type,
        }
    }

    /// Returns the mean-reversion speed.
    #[must_use]
    pub fn alpha(&self) -> T {
        self.alpha
    }

    /// Returns the volatility convention.
    #[must_use]
    pub fn volatility_type(&self) -> &VolatilityType {
        &self.volatility_type
    }
}

impl HullWhite<f64> {
    /// Computes `A(t,T)` for the affine ZCB price `P(t,T|r_t) = A(t,T) * exp(-B(t,T)*r_t)`.
    ///
    /// Uses the initial discount curve so that the model is consistent with
    /// the market term structure:
    ///
    /// `ln A(t,T) = ln(P(0,T)/P(0,t)) + B(t,T)*f(0,t)
    ///              - sigma^2/(4*alpha) * (1 - exp(-2*alpha*t)) * B(t,T)^2`
    #[allow(non_snake_case)]
    pub fn A(
        &self,
        t: f64,
        T: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let B = self.B(t, T);
        let p_0_t = curve.discount_factor_from_time(t)?;
        let p_0_T = curve.discount_factor_from_time(T)?;

        // Instantaneous forward f(0,t) via finite difference.
        let h = 1.0 / 365.0;
        let p_0_t_h = curve.discount_factor_from_time(t + h)?;
        let f_0_t = -(p_0_t_h / p_0_t).ln() / h;

        let ln_a = (p_0_T / p_0_t).ln() + B * f_0_t
            - sigma * sigma / (4.0 * self.alpha) * (1.0 - (-2.0 * self.alpha * t).exp()) * B * B;
        Ok(ln_a.exp())
    }

    /// Computes `B(t,T) = (1 - exp(-alpha*(T-t))) / alpha`.
    #[allow(non_snake_case)]
    pub fn B(&self, t: f64, T: f64) -> f64 {
        (1.0 - (-self.alpha * (T - t)).exp()) / self.alpha
    }

    /// Returns the price of a zero-coupon bond at time `t` maturing at `T`
    /// given the short rate `r_t`, using the initial discount curve.
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

    /// Computes the drift function theta(t) from the initial curve.
    #[allow(non_snake_case)]
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
            0.0
        };

        Ok(f_deriv
            + alpha * f_fwd
            + sigma * sigma / (2.0 * alpha) * (1.0 - (-2.0 * alpha * t).exp()))
    }

    /// Computes phi(t) = theta(t) - alpha * r(t).
    pub fn phi(
        &self,
        t: f64,
        r_t: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        Ok(self.theta(t, sigma, curve)? - self.alpha * r_t)
    }

    /// Variance of the short rate between `t` and `T`.
    #[allow(non_snake_case)]
    pub fn variance(&self, t: f64, T: f64, sigma: f64) -> f64 {
        let B = self.B(t, T);
        (sigma * sigma * B * B) / (2.0 * self.alpha)
    }

    /// ZCB price volatility used in the Jamshidian caplet / swaption formula.
    ///
    /// Computes sigma_p = sigma * B(t,T) * sqrt((1 - exp(-2*alpha*t)) / (2*alpha)),
    /// the lognormal volatility of the T-maturity ZCB price observed at the
    /// fixing time t, induced by the HW short-rate volatility sigma.
    #[allow(non_snake_case)]
    pub fn zcb_price_volatility(&self, sigma: f64, t: f64, T: f64) -> f64 {
        let B = self.B(t, T);
        sigma * B * ((1.0 - (-2.0 * self.alpha * t).exp()) / (2.0 * self.alpha)).sqrt()
    }

    /// Caplet price under the Hull-White model.
    #[allow(non_snake_case)]
    pub fn caplet_price(
        &self,
        strike: f64,
        t: f64,
        T: f64,
        r_t: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let forward_rate = (1.0 / self.zcb_price(r_t, t, T, sigma, curve)? - 1.0) / (T - t);
        let d1 = (forward_rate / strike).ln() + (sigma * sigma * (T - t) / 2.0);
        let d2 = d1 - sigma * (T - t).sqrt();
        Ok(forward_rate * norm_cdf(d1) - strike * norm_cdf(d2))
    }

    /// Floorlet price under the Hull-White model.
    #[allow(non_snake_case)]
    pub fn floorlet_price(
        &self,
        strike: f64,
        t: f64,
        T: f64,
        r_t: f64,
        sigma: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let forward_rate = (1.0 / self.zcb_price(r_t, t, T, sigma, curve)? - 1.0) / (T - t);
        let d1 = (forward_rate / strike).ln() + (sigma * sigma * (T - t) / 2.0);
        let d2 = d1 - sigma * (T - t).sqrt();
        Ok(strike * norm_cdf(-d2) - forward_rate * norm_cdf(-d1))
    }

    /// Swaption price under the Hull-White model.
    #[allow(non_snake_case)]
    pub fn swaption_price(
        &self,
        strike: f64,
        t: f64,
        T: f64,
        r_t: f64,
        sigma: f64,
        swap_annuity: f64,
        curve: &dyn InterestRatesTermStructure<f64>,
    ) -> Result<f64> {
        let forward_rate = (1.0 / self.zcb_price(r_t, t, T, sigma, curve)? - 1.0) / (T - t);
        let d1 = (forward_rate / strike).ln() + (sigma * sigma * (T - t) / 2.0);
        let d2 = d1 - sigma * (T - t).sqrt();
        Ok(swap_annuity * (forward_rate * norm_cdf(d1) - strike * norm_cdf(d2)))
    }

    /// Calibrates the short-rate volatility sigma(t) to the given
    /// calibration instruments (caplets, swaptions, etc.).
    ///
    /// Each instrument is expected to carry a market vol quote value.
    /// Bisection is used to find the HW sigma that matches each market price.
    pub fn calibrate(
        &self,
        instruments: &[CalibrationInstrument],
        curve: &dyn InterestRatesTermStructure<f64>,
        day_counter: DayCounter,
    ) -> Result<HullWhiteTimeDependentVolatility<f64>> {
        let ref_date = curve.reference_date();
        let mut schedule: Vec<(f64, f64)> = Vec::with_capacity(instruments.len());

        for instr in instruments {
            let details = instr.quote().details();
            let market_vol = instr.quote_value();

            let option_expiry = details.option_expiry().ok_or_else(|| {
                QSError::ValueNotSetErr("option_expiry on calibration quote".into())
            })?;
            let idx_tenor = details.index_tenor().ok_or_else(|| {
                QSError::ValueNotSetErr("index_tenor on calibration quote".into())
            })?;
            let strike = details
                .strike()
                .ok_or_else(|| QSError::ValueNotSetErr("strike on calibration quote".into()))?;

            let exp_date = ref_date + option_expiry;
            let pay_date = exp_date + idx_tenor;
            let t = day_counter.year_fraction(ref_date, exp_date);
            let big_t = day_counter.year_fraction(ref_date, pay_date);
            let tau = big_t - t;

            let df_start = curve.discount_factor(exp_date)?;
            let df_end = curve.discount_factor(pay_date)?;
            let fwd = (df_start / df_end - 1.0) / tau;

            // Compute the market-price target from the quoted vol.
            let target = df_end * tau * black_call(fwd, strike, market_vol, t);

            let obj = HwCalibrationObjective {
                hw: self,
                fwd,
                strike,
                t,
                big_t,
                target,
                is_swaption: false,
                swap_annuity: 0.0,
                df_end,
                tau,
            };

            let sol = Bisection::new(1e-8, 2.0, 200).solve(&obj)?;
            schedule.push((t, sol.x));
        }

        Ok(HullWhiteTimeDependentVolatility::new(schedule))
    }
}

/// Black call price: fwd * N(d1) - K * N(d2).
fn black_call(fwd: f64, strike: f64, vol: f64, t: f64) -> f64 {
    let sqrt_t = t.sqrt();
    let d1 = ((fwd / strike).ln() + 0.5 * vol * vol * t) / (vol * sqrt_t);
    let d2 = d1 - vol * sqrt_t;
    fwd * norm_cdf(d1) - strike * norm_cdf(d2)
}

/// Objective for HW calibration: f(sigma) = model_price(sigma) - target_price.
///
/// Uses the Jamshidian / ZCB-option approach: the HW short-rate volatility
/// maps to a ZCB price volatility sigma_p, which determines the caplet price
/// through df_end * tau * Black(fwd, K, sigma_p, t). The objective compares
/// the model-implied price against the market price.
struct HwCalibrationObjective<'a> {
    hw: &'a HullWhite<f64>,
    fwd: f64,
    strike: f64,
    t: f64,
    big_t: f64,
    target: f64,
    is_swaption: bool,
    swap_annuity: f64,
    df_end: f64,
    tau: f64,
}

impl ContFunc<f64> for HwCalibrationObjective<'_> {
    fn call(&self, sigma: &f64) -> Result<f64> {
        let price = if self.is_swaption {
            let sigma_p = self.hw.zcb_price_volatility(*sigma, self.t, self.big_t);
            self.swap_annuity * black_call(self.fwd, self.strike, sigma_p, self.t)
        } else {
            let sigma_p = self.hw.zcb_price_volatility(*sigma, self.t, self.big_t);
            self.df_end * self.tau * black_call(self.fwd, self.strike, sigma_p, self.t)
        };
        Ok(price - self.target)
    }
}

impl HullWhite<DualFwd> {
    /// Computes the `A(t,T)` function (AD-enabled).
    #[allow(non_snake_case)]
    pub fn A(&self, t: f64, T: f64, sigma: DualFwd) -> DualFwd {
        let B: DualFwd = self.B(t, T);
        let exp_term: DualFwd = (-B * self.alpha - (sigma * sigma * B * B) / 2.0)
            .exp()
            .into();
        exp_term
    }

    /// Computes the `B(t,T)` function (AD-enabled).
    #[allow(non_snake_case)]
    pub fn B(&self, t: f64, T: f64) -> DualFwd {
        let one: DualFwd = 1.0.into();
        ((one - (-self.alpha * (T - t)).exp()) / self.alpha).into()
    }

    /// Returns the ZCB price at time `t` maturing at `T` (AD-enabled).
    #[allow(non_snake_case)]
    pub fn zcb_price(&self, r_t: DualFwd, t: f64, T: f64, sigma: DualFwd) -> DualFwd {
        let a = self.A(t, T, sigma);
        let b = self.B(t, T);
        (a * (-b * r_t).exp()).into()
    }

    /// Computes the drift function theta(t) (AD-enabled).
    #[allow(non_snake_case)]
    pub fn theta(&self, t: f64, r_t: DualFwd, sigma: DualFwd) -> DualFwd {
        let B = self.B(0.0, t);
        let A = self.A(0.0, t, sigma);
        let dr_dt: DualFwd =
            (A * (-B * r_t).exp() * (-B * r_t).exp() * B * B * sigma * sigma / 2.0).into();
        (dr_dt + self.alpha * r_t).into()
    }

    /// Computes phi(t) (AD-enabled).
    pub fn phi(&self, t: f64, r_t: DualFwd, sigma: DualFwd) -> DualFwd {
        let th = self.theta(t, r_t, sigma);
        (th - self.alpha * r_t).into()
    }

    /// Variance of the short rate between `t` and `T` (AD-enabled).
    #[allow(non_snake_case)]
    pub fn variance(&self, t: f64, T: f64, sigma: DualFwd) -> DualFwd {
        let B = self.B(t, T);
        (sigma * sigma * B * B / (self.alpha * 2.0)).into()
    }

    /// Caplet price under the Hull-White model (AD-enabled).
    #[allow(non_snake_case)]
    pub fn caplet_price(
        &self,
        strike: f64,
        t: f64,
        T: f64,
        r_t: DualFwd,
        sigma: DualFwd,
    ) -> DualFwd {
        let zcb = self.zcb_price(r_t, t, T, sigma);
        let one = DualFwd::one();
        let tau = T - t;
        let forward_rate = (one / zcb - 1.0) / tau;
        let d1 = forward_rate.ln() - strike.ln() + sigma * sigma * tau / 2.0;
        let d2 = d1 - sigma * tau.sqrt();
        (forward_rate * norm_cdf::<DualFwd>(d1.into()) - norm_cdf::<DualFwd>(d2.into()) * strike)
            .into()
    }

    /// Floorlet price under the Hull-White model (AD-enabled).
    #[allow(non_snake_case)]
    pub fn floorlet_price(
        &self,
        strike: f64,
        t: f64,
        T: f64,
        r_t: DualFwd,
        sigma: DualFwd,
    ) -> DualFwd {
        let zcb = self.zcb_price(r_t, t, T, sigma);
        let tau = T - t;
        let forward_rate = (DualFwd::one() / zcb - 1.0) / tau;
        let d1 = (forward_rate / strike).ln() + sigma * sigma * tau / 2.0;
        let d2 = d1 - sigma * tau.sqrt();
        let neg_d1 = -d1;
        let neg_d2 = -d2;
        (norm_cdf::<DualFwd>(neg_d2.into()) * strike
            - forward_rate * norm_cdf::<DualFwd>(neg_d1.into()))
        .into()
    }

    /// Swaption price under the Hull-White model (AD-enabled).
    #[allow(non_snake_case)]
    pub fn swaption_price(
        &self,
        strike: f64,
        t: f64,
        T: f64,
        r_t: DualFwd,
        sigma: DualFwd,
        swap_annuity: DualFwd,
    ) -> DualFwd {
        let zcb = self.zcb_price(r_t, t, T, sigma);
        let one = DualFwd::one();
        let tau = T - t;
        let forward_rate = (one / zcb - 1.0) / tau;
        let d1 = forward_rate.ln() - strike.ln() + sigma * sigma * tau / 2.0;
        let d2 = d1 - sigma * tau.sqrt();
        (swap_annuity
            * (forward_rate * norm_cdf::<DualFwd>(d1.into())
                - norm_cdf::<DualFwd>(d2.into()) * strike))
            .into()
    }
}

/// Hull-White path generator for Monte Carlo simulation.
///
/// The drift theta(t) is computed on-the-fly from the initial discount
/// curve using finite differences. The volatility sigma(t) comes from
/// a calibrated `TimeDependentVolatility`.
pub struct HullWhitePathGenerator<T: Scalar> {
    alpha: T,
    r0: T,
    vol_func: Box<dyn TimeDependentVolatility<T>>,
    curve: Box<dyn InterestRatesTermStructure<T>>,
}

impl<T: Scalar> HullWhitePathGenerator<T> {
    /// Creates a new path generator.
    ///
    /// * `alpha` - mean-reversion speed
    /// * `r0` - initial short rate
    /// * `vol_func` - calibrated time-dependent volatility
    /// * `curve` - initial discount curve (used to compute theta on-the-fly)
    #[must_use]
    pub fn new(
        alpha: T,
        r0: T,
        vol_func: Box<dyn TimeDependentVolatility<T>>,
        curve: Box<dyn InterestRatesTermStructure<T>>,
    ) -> Self {
        Self {
            alpha,
            r0,
            vol_func,
            curve,
        }
    }
}

impl HullWhitePathGenerator<f64> {
    /// Computes the HW drift theta(t) from the initial discount curve.
    ///
    /// Uses finite differences on the curve discount factors to estimate
    /// the instantaneous forward rate and its derivative.
    fn theta_from_curve(&self, t: f64, sigma: f64) -> Result<f64> {
        let alpha = self.alpha;
        let h = 1.0 / 365.0; // one-day bump in year fractions

        let df_t = self.curve.discount_factor_from_time(t)?;
        let df_plus = self.curve.discount_factor_from_time(t + h)?;
        let f_fwd = -(df_plus / df_t).ln() / h;

        let f_deriv = if t > h {
            let df_minus = self.curve.discount_factor_from_time(t - h)?;
            let f_bwd = -(df_t / df_minus).ln() / h;
            (f_fwd - f_bwd) / (2.0 * h)
        } else {
            0.0
        };

        let theta_t = f_deriv
            + alpha * f_fwd
            + sigma * sigma / (2.0 * alpha) * (1.0 - (-2.0 * alpha * t).exp());

        Ok(theta_t)
    }
}

impl PathGenerator<f64> for HullWhitePathGenerator<f64> {
    fn generate(&self, times: &[f64], draws: &[f64], scenario: &mut [f64]) -> Result<()> {
        let mut r_t = self.r0;
        let mut t_prev = 0.0;

        for (i, &t) in times.iter().enumerate() {
            let dt = t - t_prev;
            let sigma_t = self.vol_func.vol(t)?;
            let theta_t = self.theta_from_curve(t, sigma_t)?;
            let dw = draws[i] * sigma_t * dt.sqrt();
            r_t += (theta_t - self.alpha * r_t) * dt + dw;
            scenario[i] = r_t;
            t_prev = t;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        instruments::rates::capfloor::{CapFloor, CapFloorType},
        math::interpolation::interpolator::Interpolator,
        quotes::quote::{
            CalibrationInstrumentType, Level, Quote, QuoteDetails, QuoteInstrument, QuoteLevels,
        },
        rates::{
            bootstrapping::calibrationinstrument::CalibrationInstrument,
            yieldtermstructure::discounttermstructure::DiscountTermStructure,
        },
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

        let curve_gen = flat_curve(ref_date, r0, dc);
        let curve_zcb = flat_curve(ref_date, r0, dc);
        let hw = HullWhite::<f64>::new(alpha, VolatilityType::Normal);
        let vol_func = HullWhiteTimeDependentVolatility::new(vec![(0.0, sigma)]);

        let gen = HullWhitePathGenerator::new(alpha, r0, Box::new(vol_func), Box::new(curve_gen));

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
            gen.generate(&times_f64, &draws, &mut scenario).unwrap();

            let r_t1 = scenario[n_steps - 1];

            // Discount 0 -> t1 via trapezoidal rule.
            let mut integral = r0 * dt_step * 0.5;
            for j in 0..n_steps - 1 {
                integral += scenario[j] * dt_step;
            }
            integral += scenario[n_steps - 1] * dt_step * 0.5;
            let df_01 = (-integral).exp();

            let zcb = hw.zcb_price(r_t1, t1, t2, sigma, &curve_zcb).unwrap();
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
        let r0 = 0.05;
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

        let gen = HullWhitePathGenerator::new(alpha, r0, Box::new(vol_func), Box::new(curve));

        let n_paths = 100_000;
        let mut rng = StdRng::seed_from_u64(99);
        let mut draws = vec![0.0_f64; n_steps];
        let mut scenario = vec![0.0_f64; n_steps];
        let mut terminal_sum = 0.0;

        for _ in 0..n_paths {
            for d in &mut draws {
                *d = std_normal(&mut rng);
            }
            gen.generate(&times_f64, &draws, &mut scenario).unwrap();
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
    /// express as Black vols, build CalibrationInstruments, calibrate back,
    /// and verify recovery of the original sigma.
    #[test]
    fn hw_calibrate_round_trip_via_quotes() {
        let alpha = 0.1_f64;
        let r0 = 0.03;
        let true_sigma = 0.015;
        let strike = 0.03; // ATM strike (equal to flat rate)

        let ref_date = Date::new(2025, 1, 1);
        let dc = DayCounter::Actual365;
        let curve = flat_curve(ref_date, r0, dc);

        let hw_model = HullWhite::<f64>::new(alpha, VolatilityType::Normal);

        // Define caplet expiries and underlying index tenor.
        let expiries = ["6M", "1Y", "18M", "2Y"];
        let idx_tenor_str = "6M";
        let idx_tenor = Period::from_str(idx_tenor_str).unwrap();

        let mut instruments: Vec<CalibrationInstrument> = Vec::new();

        for exp_str in &expiries {
            let exp = Period::from_str(exp_str).unwrap();
            let exp_date = ref_date + exp;
            let pay_date = exp_date + idx_tenor;
            let t = dc.year_fraction(ref_date, exp_date);
            let big_t = dc.year_fraction(ref_date, pay_date);
            let tau = big_t - t;

            // Curve-based forward and discount factors.
            let df_start = curve.discount_factor(exp_date).unwrap();
            let df_end = curve.discount_factor(pay_date).unwrap();
            let fwd = (df_start / df_end - 1.0) / tau;

            // HW caplet price via Jamshidian: df_end * tau * Black(fwd, K, sigma_p, t).
            let sigma_p = hw_model.zcb_price_volatility(true_sigma, t, big_t);
            let hw_price = df_end * tau * black_call(fwd, strike, sigma_p, t);

            // Invert Black formula for the market vol that gives the same price.
            let scaled = hw_price / (df_end * tau);
            let mut lo = 1e-6_f64;
            let mut hi = 5.0_f64;
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if black_call(fwd, strike, mid, t) > scaled {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let mkt_vol = 0.5 * (lo + hi);

            // Build the quote.
            let qid = format!(
                "CapletFloorlet_USD_SOFR_{idx_tenor_str}_{exp_str}_Absolute_{strike}_Black"
            );
            let details = QuoteDetails::new(qid.clone(), QuoteInstrument::CapletFloorlet)
                .with_option_expiry(exp)
                .with_index_tenor(idx_tenor)
                .with_strike(strike)
                .with_vol_type(VolatilityType::Black);
            let levels = QuoteLevels::with_mid(mkt_vol);
            let quote = Quote::new(details, levels);

            // Build a dummy CapFloor wrapper (calibrate reads from quote details).
            let built = CalibrationInstrumentType::CapFloor(CapFloor::new(
                qid,
                vec![],
                MarketIndex::SOFR,
                Currency::USD,
                strike,
                CapFloorType::Cap,
            ));

            instruments.push(CalibrationInstrument::new(
                quote,
                Level::Mid,
                built,
                mkt_vol,
                pay_date,
            ));
        }

        let schedule = hw_model
            .calibrate(&instruments, &curve, dc)
            .expect("calibration should converge");

        assert_eq!(schedule.len(), expiries.len());

        for (i, &(t, calibrated_sigma)) in schedule.iter().enumerate() {
            let err = (calibrated_sigma - true_sigma).abs();
            assert!(
                err < 1e-4,
                "caplet {i} (t={t:.2}): calibrated sigma {calibrated_sigma:.6} \
                 vs true {true_sigma}, err {err:.2e}"
            );
        }
    }
}

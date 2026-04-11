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

/// Brownian Motion (GBM / Black-Scholes) model.
pub struct BrownianMotion<T: Scalar> {
    spot: T,
    rate: T,
    vol_func: Box<dyn TimeDependentVolatility<T>>,
    dividend_rate: Option<T>,
}

impl<T: Scalar> BrownianMotion<T> {
    /// Creates a new [`BrownianMotion`].
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

    /// Returns the risk-free rate.
    #[must_use]
    pub const fn rate(&self) -> T {
        self.rate
    }

    /// Returns the continuous dividend rate.
    #[must_use]
    pub const fn dividend_rate(&self) -> Option<&T> {
        self.dividend_rate.as_ref()
    }
}

impl BrownianMotion<f64> {
    ///  Black call/put price from a forward (AD-enabled).
    ///
    /// # Errors
    /// Returns an error if strike, volatility, or time to expiry are non-positive.
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

    /// Black-Scholes delta: ∂V/∂F (forward delta).
    ///
    /// # Errors
    /// Returns an error if d₁/d₂ computation fails.
    pub fn delta(fwd: f64, strike: f64, vol: f64, tau: f64, is_call: bool) -> Result<f64> {
        let (d1, _) = d1_d2(fwd, strike, vol, tau)?;
        if is_call {
            Ok(d1.norm_cdf())
        } else {
            Ok((-d1).norm_cdf())
        }
    }

    /// Black-Scholes vega: ∂V/∂σ = F · φ(d₁) · √τ.
    ///
    /// # Errors
    /// Returns an error if d₁/d₂ computation fails.
    pub fn vega(fwd: f64, strike: f64, vol: f64, tau: f64) -> Result<f64> {
        let (d1, _) = d1_d2(fwd, strike, vol, tau)?;
        let pdf_d1 = (-0.5 * d1 * d1).exp() * FRAC_1_SQRT_2PI;
        Ok(fwd * pdf_d1 * tau.sqrt())
    }

    /// Black-Scholes rho: ∂V/∂r = K · τ · N(±d₂).
    ///
    /// # Errors
    /// Returns an error if d₁/d₂ computation fails.
    pub fn rho(fwd: f64, strike: f64, vol: f64, tau: f64, is_call: bool) -> Result<f64> {
        let (_, d2) = d1_d2(fwd, strike, vol, tau)?;
        if is_call {
            Ok(strike * tau * d2.norm_cdf())
        } else {
            Ok(-(strike * tau * (-d2).norm_cdf()))
        }
    }

    /// Black-Scholes theta: ∂V/∂τ.
    ///
    /// # Errors
    /// Returns an error if d₁/d₂ computation fails.
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
    /// # Errors
    /// Returns an error if strike or time to expiry are non-positive.
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

    /// Black-Scholes delta: ∂V/∂F (AD-enabled).
    ///
    /// # Errors
    /// Returns an error if d₁/d₂ computation fails.
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

    /// Black-Scholes vega: ∂V/∂σ = F · φ(d₁) · √τ (AD-enabled).
    ///
    /// # Errors
    /// Returns an error if d₁/d₂ computation fails.
    pub fn vega(fwd: DualFwd, strike: f64, vol: DualFwd, tau: f64) -> Result<DualFwd> {
        let (d1, _) = d1_d2_ad(fwd, strike, vol, tau)?;
        let pdf_d1 = (-d1 * d1 * 0.5).exp() * FRAC_1_SQRT_2PI;
        Ok((fwd * pdf_d1 * tau.sqrt()).into())
    }

    /// Black-Scholes rho: ∂V/∂r = K · τ · N(±d₂) (AD-enabled).
    ///
    /// # Errors
    /// Returns an error if d₁/d₂ computation fails.
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

    /// Black-Scholes theta: ∂V/∂τ (AD-enabled).
    ///
    /// # Errors
    /// Returns an error if d₁/d₂ computation fails.
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
    fn generate(&self, times: &[f64], draws: &[f64], scenario: &mut [f64]) -> Result<()> {
        if times.len() != draws.len() || times.len() != scenario.len() {
            return Err(QSError::InvalidValueErr(
                "times, draws, and scenario must have the same length".to_string(),
            ));
        }

        let mut prev_spot = self.spot;
        for i in 0..times.len() {
            let t = times[i];
            let z = draws[i];
            let vol = self.vol_func.vol(t)?;
            let drift = (self.rate - self.dividend_rate.unwrap_or(0.0)).mul_add(t, -(0.5 * vol * vol * t));
            let diffusion = vol * z * t.sqrt();
            let log_return = drift + diffusion;
            let spot = prev_spot * log_return.exp();
            scenario[i] = spot;
            prev_spot = spot;
        }
        Ok(())
    }
}

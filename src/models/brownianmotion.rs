use std::marker::PhantomData;

use crate::{
    ad::{dual::DualFwd, expr::FloatExt, scalar::Scalar},
    math::probability::norm_cdf::{norm_cdf, NormCDF},
    models::montecarloengine::{PathGenerator, TimeDependentVolatility},
    utils::errors::{QSError, Result},
};

/// 1 / sqrt(2 pi)
const FRAC_1_SQRT_2PI: f64 =
    std::f64::consts::FRAC_2_SQRT_PI * 0.5 * std::f64::consts::FRAC_1_SQRT_2;

/// Brownian Motion (GBM / Black-Scholes) model.
#[derive(Clone, Debug)]
pub struct BrownianMotion<T: Scalar + NormCDF> {
    _marker: PhantomData<T>,
}

impl<T: Scalar + NormCDF> Default for BrownianMotion<T> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl BrownianMotion<f64> {
    /// Computes d₁ and d₂ for the Black-Scholes formula.
    fn d1_d2(fwd: f64, strike: f64, vol: f64, tau: f64) -> Result<(f64, f64)> {
        if strike <= 0.0 {
            return Err(QSError::InvalidValueErr("strike must be positive".into()));
        }
        if tau <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "time to expiry must be positive".into(),
            ));
        }
        if vol <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "volatility must be positive".into(),
            ));
        }
        let sqrt_tau = tau.sqrt();
        let d1 = (fwd / strike).ln() / (vol * sqrt_tau) + 0.5 * vol * sqrt_tau;
        let d2 = d1 - vol * sqrt_tau;
        Ok((d1, d2))
    }

    /// Undiscounted Black call/put price from a forward.
    pub fn undiscounted_closed_form_price(
        fwd: f64,
        strike: f64,
        vol: f64,
        tau: f64,
        is_call: bool,
    ) -> Result<f64> {
        let (d1, d2) = Self::d1_d2(fwd, strike, vol, tau)?;
        if is_call {
            Ok(fwd * d1.norm_cdf() - strike * d2.norm_cdf())
        } else {
            Ok(strike * (-d2).norm_cdf() - fwd * (-d1).norm_cdf())
        }
    }

    /// Black-Scholes delta: ∂V/∂F (forward delta).
    pub fn delta(fwd: f64, strike: f64, vol: f64, tau: f64, is_call: bool) -> Result<f64> {
        let (d1, _) = Self::d1_d2(fwd, strike, vol, tau)?;
        if is_call {
            Ok(d1.norm_cdf())
        } else {
            Ok((-d1).norm_cdf())
        }
    }

    /// Black-Scholes vega: ∂V/∂σ = F · φ(d₁) · √τ.
    pub fn vega(fwd: f64, strike: f64, vol: f64, tau: f64) -> Result<f64> {
        let (d1, _) = Self::d1_d2(fwd, strike, vol, tau)?;
        let pdf_d1 = (-0.5 * d1 * d1).exp() * FRAC_1_SQRT_2PI;
        Ok(fwd * pdf_d1 * tau.sqrt())
    }

    /// Black-Scholes rho: ∂V/∂r = K · τ · N(±d₂).
    pub fn rho(fwd: f64, strike: f64, vol: f64, tau: f64, is_call: bool) -> Result<f64> {
        let (_, d2) = Self::d1_d2(fwd, strike, vol, tau)?;
        if is_call {
            Ok(strike * tau * d2.norm_cdf())
        } else {
            Ok(-(strike * tau * (-d2).norm_cdf()))
        }
    }

    /// Black-Scholes theta: ∂V/∂τ.
    pub fn theta(fwd: f64, strike: f64, vol: f64, tau: f64, is_call: bool) -> Result<f64> {
        let (d1, d2) = Self::d1_d2(fwd, strike, vol, tau)?;
        let pdf_d1 = (-0.5 * d1 * d1).exp() * FRAC_1_SQRT_2PI;
        let term1 = -fwd * pdf_d1 * vol / (2.0 * tau.sqrt());
        if is_call {
            Ok(term1 + strike * d2.norm_cdf())
        } else {
            Ok(term1 - strike * (-d2).norm_cdf())
        }
    }
}

impl BrownianMotion<DualFwd> {
    /// Computes d₁ and d₂ for the Black-Scholes formula (AD-enabled).
    fn d1_d2(fwd: DualFwd, strike: f64, vol: DualFwd, tau: f64) -> Result<(DualFwd, DualFwd)> {
        if strike <= 0.0 {
            return Err(QSError::InvalidValueErr("strike must be positive".into()));
        }
        if tau <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "time to expiry must be positive".into(),
            ));
        }
        let sqrt_tau = tau.sqrt();
        let d1: DualFwd = ((fwd / strike).ln() / (vol * sqrt_tau) + vol * sqrt_tau * 0.5).into();
        let d2: DualFwd = (d1 - vol * sqrt_tau).into();
        Ok((d1, d2))
    }

    /// Undiscounted Black call/put price from a forward (AD-enabled).
    pub fn closed_form_price(
        fwd: DualFwd,
        strike: f64,
        vol: DualFwd,
        tau: f64,
        is_call: bool,
    ) -> Result<DualFwd> {
        let (d1, d2) = Self::d1_d2(fwd, strike, vol, tau)?;
        if is_call {
            Ok((fwd * norm_cdf(d1) - norm_cdf(d2) * strike).into())
        } else {
            let neg_d2: DualFwd = (-d2).into();
            let neg_d1: DualFwd = (-d1).into();
            Ok((norm_cdf(neg_d2) * strike - fwd * norm_cdf(neg_d1)).into())
        }
    }

    /// Black-Scholes delta: ∂V/∂F (AD-enabled).
    pub fn delta(
        fwd: DualFwd,
        strike: f64,
        vol: DualFwd,
        tau: f64,
        is_call: bool,
    ) -> Result<DualFwd> {
        let (d1, _) = Self::d1_d2(fwd, strike, vol, tau)?;
        if is_call {
            Ok(norm_cdf(d1))
        } else {
            let neg_d1: DualFwd = (-d1).into();
            Ok(norm_cdf(neg_d1))
        }
    }

    /// Black-Scholes vega: ∂V/∂σ = F · φ(d₁) · √τ (AD-enabled).
    pub fn vega(fwd: DualFwd, strike: f64, vol: DualFwd, tau: f64) -> Result<DualFwd> {
        let (d1, _) = Self::d1_d2(fwd, strike, vol, tau)?;
        let pdf_d1 = (-d1 * d1 * 0.5).exp() * FRAC_1_SQRT_2PI;
        Ok((fwd * pdf_d1 * tau.sqrt()).into())
    }

    /// Black-Scholes rho: ∂V/∂r = K · τ · N(±d₂) (AD-enabled).
    pub fn rho(
        fwd: DualFwd,
        strike: f64,
        vol: DualFwd,
        tau: f64,
        is_call: bool,
    ) -> Result<DualFwd> {
        let (_, d2) = Self::d1_d2(fwd, strike, vol, tau)?;
        let st = strike * tau;
        if is_call {
            Ok((norm_cdf(d2) * st).into())
        } else {
            let neg_d2: DualFwd = (-d2).into();
            Ok((-(norm_cdf(neg_d2) * st)).into())
        }
    }

    /// Black-Scholes theta: ∂V/∂τ (AD-enabled).
    pub fn theta(
        fwd: DualFwd,
        strike: f64,
        vol: DualFwd,
        tau: f64,
        is_call: bool,
    ) -> Result<DualFwd> {
        let (d1, d2) = Self::d1_d2(fwd, strike, vol, tau)?;
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

/// Monte Carlo path generator for Brownian Motion dynamics.
pub struct BronianMotionPathGenerator<T: Scalar> {
    spot: T,
    rate: T,
    vol_func: Box<dyn TimeDependentVolatility<T>>,
    dividend_rate: Option<T>,
}

impl<T: Scalar> BronianMotionPathGenerator<T> {
    /// Creates a new [`BronianMotionPathGenerator`].
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
    pub fn spot(&self) -> T {
        self.spot
    }

    /// Returns the risk-free rate.
    #[must_use]
    pub fn rate(&self) -> T {
        self.rate
    }

    /// Returns the continuous dividend rate.
    #[must_use]
    pub fn dividend_rate(&self) -> Option<&T> {
        self.dividend_rate.as_ref()
    }
}

impl PathGenerator<f64> for BronianMotionPathGenerator<f64> {
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
            let drift = (self.rate - self.dividend_rate.unwrap_or(0.0)) * t - 0.5 * vol * vol * t;
            let diffusion = vol * z * t.sqrt();
            let log_return = drift + diffusion;
            let spot = prev_spot * log_return.exp();
            scenario[i] = spot;
            prev_spot = spot;
        }

        Ok(())
    }
}

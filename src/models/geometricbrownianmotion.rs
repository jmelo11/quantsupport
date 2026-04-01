use std::marker::PhantomData;

use crate::ad::scalar::Scalar;
use crate::math::probability::norm_cdf::{norm_cdf, NormCDF};

/// 1 / sqrt(2π)
const FRAC_1_SQRT_2PI: f64 =
    std::f64::consts::FRAC_2_SQRT_PI * 0.5 * std::f64::consts::FRAC_1_SQRT_2;

/// Geometric Brownian Motion (GBM / Black-Scholes) model, generic over the
/// scalar type `T`.  Use `GeometricBrownianMotion<f64>` for plain pricing and
/// `GeometricBrownianMotion<DualFwd>` for AD-enabled pricing.
#[derive(Clone, Debug)]
pub struct GeometricBrownianMotion<T: Scalar + NormCDF> {
    _marker: PhantomData<T>,
}

impl<T: Scalar + NormCDF> Default for GeometricBrownianMotion<T> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T: Scalar + NormCDF> GeometricBrownianMotion<T> {
    /// Computes d₁ and d₂ for the Black-Scholes formula.
    #[must_use]
    pub fn d1_d2(fwd: T, strike: f64, vol: T, tau: f64) -> (T, T) {
        let vol_sqrt_tau = vol.mul_val(T::scalar(tau.sqrt()));
        let d1 = fwd
            .div_val(T::scalar(strike))
            .ln()
            .add_val(vol.mul_val(vol).mul_val(T::scalar(0.5 * tau)))
            .div_val(vol_sqrt_tau);
        let d2 = d1.sub_val(vol_sqrt_tau);
        (d1, d2)
    }

    /// Undiscounted Black call/put price from a forward.
    #[must_use]
    pub fn closed_form_price(fwd: T, strike: f64, vol: T, tau: f64, is_call: bool) -> T {
        let (d1, d2) = Self::d1_d2(fwd, strike, vol, tau);
        let s = T::scalar(strike);
        if is_call {
            fwd.mul_val(norm_cdf(d1)).sub_val(s.mul_val(norm_cdf(d2)))
        } else {
            s.mul_val(norm_cdf(d2.neg_val()))
                .sub_val(fwd.mul_val(norm_cdf(d1.neg_val())))
        }
    }

    /// Black-Scholes delta: ∂V/∂S.
    ///
    /// # Panics
    /// Panics if `strike <= 0`, `tau <= 0`, or `vol <= 0`.
    #[must_use]
    pub fn delta(fwd: T, strike: f64, vol: T, tau: f64, is_call: bool) -> T {
        assert!(strike > 0.0, "strike must be positive");
        assert!(tau > 0.0, "time to expiry must be positive");
        assert!(vol > 0.0, "volatility must be positive");

        let (d1, _) = Self::d1_d2(fwd, strike, vol, tau);
        if is_call {
            norm_cdf(d1)
        } else {
            norm_cdf(d1.neg_val())
        }
    }

    /// Black-Scholes vega: ∂V/∂σ = φ(d₁) · √τ.
    ///
    /// # Panics
    /// Panics if `strike <= 0`, `tau <= 0`, or `vol <= 0`.
    #[must_use]
    pub fn vega(fwd: T, strike: f64, vol: T, tau: f64) -> T {
        assert!(strike > 0.0, "strike must be positive");
        assert!(tau > 0.0, "time to expiry must be positive");
        assert!(vol > 0.0, "volatility must be positive");

        let (d1, _) = Self::d1_d2(fwd, strike, vol, tau);
        let pdf_d1 = d1
            .mul_val(d1)
            .neg_val()
            .mul_val(T::scalar(0.5))
            .exp()
            .mul_val(T::scalar(FRAC_1_SQRT_2PI));
        pdf_d1.mul_val(T::scalar(tau.sqrt()))
    }

    /// Black-Scholes rho: ∂V/∂r.
    ///
    /// # Panics
    /// Panics if `strike <= 0`, `tau <= 0`, or `vol <= 0`.
    #[must_use]
    pub fn rho(fwd: T, strike: f64, vol: T, tau: f64, is_call: bool) -> T {
        assert!(strike > 0.0, "strike must be positive");
        assert!(tau > 0.0, "time to expiry must be positive");
        assert!(vol > 0.0, "volatility must be positive");

        let (_, d2) = Self::d1_d2(fwd, strike, vol, tau);
        let st = T::scalar(strike * tau);
        if is_call {
            norm_cdf(d2).mul_val(st)
        } else {
            norm_cdf(d2.neg_val()).mul_val(st).neg_val()
        }
    }

    /// Black-Scholes theta: ∂V/∂τ.
    ///
    /// # Panics
    /// Panics if `strike <= 0`, `tau <= 0`, or `vol <= 0`.
    #[must_use]
    pub fn theta(fwd: T, strike: f64, vol: T, tau: f64, is_call: bool) -> T {
        assert!(strike > 0.0, "strike must be positive");
        assert!(tau > 0.0, "time to expiry must be positive");
        assert!(vol > 0.0, "volatility must be positive");

        let (d1, d2) = Self::d1_d2(fwd, strike, vol, tau);
        let pdf_d1 = d1
            .mul_val(d1)
            .neg_val()
            .mul_val(T::scalar(0.5))
            .exp()
            .mul_val(T::scalar(FRAC_1_SQRT_2PI));
        let term1 = pdf_d1
            .neg_val()
            .mul_val(vol)
            .div_val(T::scalar(2.0 * tau.sqrt()));
        let s = T::scalar(strike);
        let term2 = if is_call {
            norm_cdf(d2).mul_val(s)
        } else {
            norm_cdf(d2.neg_val()).mul_val(s).neg_val()
        };
        term1.add_val(term2)
    }
}

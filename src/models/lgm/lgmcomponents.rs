use crate::{
    ad::scalar::Scalar,
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    utils::errors::Result,
};

/// Single-factor LGM rate model parametrised by mean-reversion (`lambda`)
/// and volatility (`sigma`), calibrated to an initial discount curve.
pub struct LgmRateModel<'a, T: Scalar> {
    lambda: T,
    sigma: T,
    discount_curve: &'a dyn InterestRatesTermStructure<T>,
}

impl<'a, T: Scalar> LgmRateModel<'a, T> {
    /// Creates a new LGM rate model.
    pub fn new(lambda: T, sigma: T, discount_curve: &'a dyn InterestRatesTermStructure<T>) -> Self {
        Self {
            lambda,
            sigma,
            discount_curve,
        }
    }

    /// Returns the drift of the Gaussian factor under its own measure (always zero).
    #[must_use]
    pub fn self_drift(&self, _t: f64) -> T {
        T::zero()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Generic T: Scalar instantiation
// ═══════════════════════════════════════════════════════════════════════════

impl<T: Scalar> LgmRateModel<'_, T> {
    /// Mean-reversion function `H(t) = (1 - e^{-λt}) / λ`.
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

    /// Derivative `H'(t) = e^{-λt}`.
    #[allow(non_snake_case)]
    #[must_use]
    pub fn H_dot(&self, t: f64) -> T {
        if self.lambda.value().abs() < 1e-14 {
            T::one()
        } else {
            self.lambda.neg_val().mul_val(T::scalar(t)).exp()
        }
    }

    /// Instantaneous volatility of the Gaussian factor.
    #[must_use]
    pub fn alpha(&self, t: f64) -> T {
        if self.lambda.value().abs() < 1e-14 {
            self.sigma
        } else {
            self.sigma.mul_val(self.lambda.mul_val(T::scalar(t)).exp())
        }
    }

    /// Integrated variance `ζ(t) = ∫₀ᵗ α²(s) ds`.
    #[must_use]
    pub fn zeta(&self, t: f64) -> T {
        if self.lambda.value().abs() < 1e-14 {
            self.sigma.mul_val(self.sigma).mul_val(T::scalar(t))
        } else {
            // σ² (exp(2λt) - 1) / (2λ)
            let two_lambda = self.lambda.mul_val(T::scalar(2.0));
            let sigma_sq = self.sigma.mul_val(self.sigma);
            let exp_part = two_lambda.mul_val(T::scalar(t)).exp().sub_val(T::one());
            sigma_sq.mul_val(exp_part).div_val(two_lambda)
        }
    }

    /// Computes the simulated discount factor `P(t,T|z_t)`.
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

    /// Computes the instantaneous forward rate `f(t,T|z_t)`.
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

    /// Computes the short rate `r(t|z_t)`.
    ///
    /// # Errors
    /// Returns an error if forward rate computation fails.
    pub fn short_rate(&self, t: f64, z_t: T) -> Result<T> {
        self.instantaneous_forward_rate(t, t, z_t)
    }

    /// Drift adjustment (gamma) for a foreign factor under the domestic measure.
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

    /// Euler step for the Gaussian factor with an arbitrary drift.
    #[must_use]
    pub fn evolve_factor_euler(&self, t: f64, z_t: T, dt: f64, drift: T, dw_z: f64) -> T {
        // z + drift * dt + alpha(t) * dW
        z_t.add_val(drift.mul_val(T::scalar(dt)))
            .add_val(self.alpha(t).mul_val(T::scalar(dw_z)))
    }

    /// Euler step for the domestic Gaussian factor (zero drift).
    #[must_use]
    pub fn evolve_domestic_factor_euler(&self, t: f64, z_t: T, dt: f64, dw_z: f64) -> T {
        self.evolve_factor_euler(t, z_t, dt, T::zero(), dw_z)
    }

    /// Euler step for a foreign factor under the domestic risk-neutral measure.
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

// ═══════════════════════════════════════════════════════════════════════════
//  LgmFxModel
// ═══════════════════════════════════════════════════════════════════════════

/// LGM FX model coupling domestic and foreign rate models with an FX volatility.
pub struct LgmFxModel<'a, T: Scalar> {
    domestic: &'a LgmRateModel<'a, T>,
    foreign: &'a LgmRateModel<'a, T>,
    fx_vol: T,
    spot_0: T,
    rho_zx_dom_fx: T, // rho_{0i}^{zx}
}

impl<'a, T: Scalar> LgmFxModel<'a, T> {
    /// Creates a new LGM FX model.
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

    /// Returns the FX volatility.
    #[must_use]
    pub const fn fx_vol(&self) -> T {
        self.fx_vol
    }

    /// Returns the initial FX spot rate.
    #[must_use]
    pub const fn initial_spot(&self) -> T {
        self.spot_0
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  LgmFxModel — generic T: Scalar instantiation
// ═══════════════════════════════════════════════════════════════════════════

impl<T: Scalar> LgmFxModel<'_, T> {
    /// Computes the FX drift under the domestic measure.
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

    /// Computes the log FX drift under the domestic measure.
    ///
    /// # Errors
    /// Returns an error if FX drift computation fails.
    pub fn log_fx_drift(&self, t: f64, z_dom: T, z_for: T) -> Result<T> {
        let drift = self.fx_drift(t, z_dom, z_for)?;
        // drift - 0.5 * σ_fx²
        Ok(drift.sub_val(T::scalar(0.5).mul_val(self.fx_vol).mul_val(self.fx_vol)))
    }

    /// Evolves the FX spot using log-Euler discretization.
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

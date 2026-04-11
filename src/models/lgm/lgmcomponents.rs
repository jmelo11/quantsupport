use crate::{
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    utils::errors::Result,
};

pub struct LgmRateModel<'a> {
    lambda: f64,
    sigma: f64,
    discount_curve: &'a dyn InterestRatesTermStructure<f64>,
}

impl<'a> LgmRateModel<'a> {
    pub fn new(
        lambda: f64,
        sigma: f64,
        discount_curve: &'a dyn InterestRatesTermStructure<f64>,
    ) -> Self {
        Self {
            lambda,
            sigma,
            discount_curve,
        }
    }

    #[allow(non_snake_case)]
    #[must_use]
    pub fn H(&self, t: f64) -> f64 {
        if self.lambda.abs() < 1e-14 {
            t
        } else {
            (1.0 - (-self.lambda * t).exp()) / self.lambda
        }
    }

    #[allow(non_snake_case)]
    #[must_use]
    pub fn H_dot(&self, t: f64) -> f64 {
        if self.lambda.abs() < 1e-14 {
            1.0
        } else {
            (-self.lambda * t).exp()
        }
    }

    #[must_use]
    pub fn alpha(&self, t: f64) -> f64 {
        if self.lambda.abs() < 1e-14 {
            self.sigma
        } else {
            self.sigma * (self.lambda * t).exp()
        }
    }

    #[must_use]
    pub fn zeta(&self, t: f64) -> f64 {
        if self.lambda.abs() < 1e-14 {
            self.sigma * self.sigma * t
        } else {
            self.sigma * self.sigma * (2.0 * self.lambda * t).exp_m1() / (2.0 * self.lambda)
        }
    }

    /// Computes the simulated discount factor `P(t,T|z_t)`.
    ///
    /// # Errors
    /// Returns an error if discount factor lookup fails.
    #[allow(non_snake_case)]
    pub fn P_discount(&self, t: f64, T: f64, z_t: f64) -> Result<f64> {
        let p0_t = self.discount_curve.discount_factor_from_time(t)?;
        let p0_T = self.discount_curve.discount_factor_from_time(T)?;
        let h_t = self.H(t);
        let h_T = self.H(T);
        let zeta_t = self.zeta(t);

        Ok((p0_T / p0_t)
            * (-(h_T - h_t))
                .mul_add(z_t, -(0.5 * h_T.mul_add(h_T, -(h_t * h_t)) * zeta_t))
                .exp())
    }

    /// Computes the instantaneous forward rate `f(t,T|z_t)`.
    ///
    /// # Errors
    /// Returns an error if forward rate lookup fails.
    #[allow(non_snake_case)]
    pub fn instantaneous_forward_rate(&self, t: f64, T: f64, z_t: f64) -> Result<f64> {
        let f0_T = self.discount_curve.forward_rate_from_time(0.0, T)?;
        let h_T = self.H(T);
        let h_T_dot = self.H_dot(T);
        let zeta_t = self.zeta(t);

        Ok((h_T_dot * h_T).mul_add(zeta_t, h_T_dot.mul_add(z_t, f0_T)))
    }

    /// Computes the short rate `r(t|z_t)`.
    ///
    /// # Errors
    /// Returns an error if forward rate computation fails.
    pub fn short_rate(&self, t: f64, z_t: f64) -> Result<f64> {
        self.instantaneous_forward_rate(t, t, z_t)
    }

    #[must_use]
    pub const fn self_drift(&self, _t: f64) -> f64 {
        0.0
    }

    #[must_use]
    pub fn gamma_under_domestic_measure(
        &self,
        t: f64,
        domestic_rate_model: &Self,
        fx_vol: f64,
        rho_zx_self_fx: f64,
        rho_zz_self_dom: f64,
    ) -> f64 {
        let alpha_i = self.alpha(t);
        let alpha_0 = domestic_rate_model.alpha(t);
        let h_i = self.H(t);
        let h_0 = domestic_rate_model.H(t);

        (rho_zz_self_dom * alpha_i * alpha_0).mul_add(
            h_0,
            (-alpha_i * alpha_i).mul_add(h_i, -(rho_zx_self_fx * fx_vol * alpha_i)),
        )
    }

    #[must_use]
    pub fn evolve_factor_euler(&self, t: f64, z_t: f64, dt: f64, drift: f64, dw_z: f64) -> f64 {
        self.alpha(t).mul_add(dw_z, drift.mul_add(dt, z_t))
    }

    #[must_use]
    pub fn evolve_domestic_factor_euler(&self, t: f64, z_t: f64, dt: f64, dw_z: f64) -> f64 {
        self.evolve_factor_euler(t, z_t, dt, 0.0, dw_z)
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn evolve_foreign_factor_under_domestic_measure_euler(
        &self,
        t: f64,
        z_t: f64,
        dt: f64,
        dw_z: f64,
        domestic_rate_model: &Self,
        fx_vol: f64,
        rho_zx_self_fx: f64,
        rho_zz_self_dom: f64,
    ) -> f64 {
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

pub struct LgmFxModel<'a> {
    domestic: &'a LgmRateModel<'a>,
    foreign: &'a LgmRateModel<'a>,
    fx_vol: f64,
    spot_0: f64,
    rho_zx_dom_fx: f64, // rho_{0i}^{zx}
}

impl<'a> LgmFxModel<'a> {
    #[must_use]
    pub const fn new(
        domestic: &'a LgmRateModel<'a>,
        foreign: &'a LgmRateModel<'a>,
        fx_vol: f64,
        spot_0: f64,
        rho_zx_dom_fx: f64,
    ) -> Self {
        Self {
            domestic,
            foreign,
            fx_vol,
            spot_0,
            rho_zx_dom_fx,
        }
    }

    /// Computes the FX drift under the domestic measure.
    ///
    /// # Errors
    /// Returns an error if short rate computation fails.
    pub fn fx_drift(&self, t: f64, z_dom: f64, z_for: f64) -> Result<f64> {
        let r_0 = self.domestic.short_rate(t, z_dom)?;
        let r_i = self.foreign.short_rate(t, z_for)?;
        let alpha_0 = self.domestic.alpha(t);
        let h_0 = self.domestic.H(t);

        Ok((self.rho_zx_dom_fx * alpha_0 * h_0).mul_add(self.fx_vol, r_0 - r_i))
    }

    /// Computes the log FX drift under the domestic measure.
    ///
    /// # Errors
    /// Returns an error if FX drift computation fails.
    pub fn log_fx_drift(&self, t: f64, z_dom: f64, z_for: f64) -> Result<f64> {
        Ok((0.5 * self.fx_vol).mul_add(-self.fx_vol, self.fx_drift(t, z_dom, z_for)?))
    }

    /// Evolves the FX spot using log-Euler discretization.
    ///
    /// # Errors
    /// Returns an error if log FX drift computation fails.
    pub fn evolve_fx_spot_log_euler(
        &self,
        t: f64,
        x_t: f64,
        z_dom: f64,
        z_for: f64,
        dt: f64,
        dw_x: f64,
    ) -> Result<f64> {
        let mu_log = self.log_fx_drift(t, z_dom, z_for)?;
        Ok(x_t * mu_log.mul_add(dt, self.fx_vol * dw_x).exp())
    }

    #[must_use]
    pub const fn fx_vol(&self) -> f64 {
        self.fx_vol
    }

    #[must_use]
    pub const fn initial_spot(&self) -> f64 {
        self.spot_0
    }
}

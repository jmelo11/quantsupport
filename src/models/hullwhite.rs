use crate::{
    ad::{
        adreal::{DualFwd, FloatExt},
        scalar::Scalar,
    },
    math::probability::norm_cdf::norm_cdf,
    volatility::volatilityindexing::VolatilityType,
};

/// Parameters for the Hull-White (one-factor) short-rate model.
#[derive(Clone, Debug)]
pub struct HullWhite<T: Scalar> {
    /// Mean-reversion speed.
    alpha: T,
    /// Volatility convention used for calibration (Black or Normal).
    volatility_type: VolatilityType,
    /// Quote IDs that define the calibration basket.
    calibration_quote_ids: Vec<String>,
}

impl<T> HullWhite<T>
where
    T: Scalar,
{
    /// Creates new shifted Hull-White parameters.
    #[must_use]
    pub fn new(alpha: T, volatility_type: VolatilityType) -> Self {
        Self {
            alpha,
            volatility_type,
            calibration_quote_ids: Vec::new(),
        }
    }

    /// Adds swaption quote IDs that form the calibration basket.
    #[must_use]
    pub fn with_calibration_quotes(mut self, quote_ids: Vec<String>) -> Self {
        self.calibration_quote_ids = quote_ids;
        self
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

    /// Returns the calibration swaption quote IDs.
    #[must_use]
    pub fn calibration_quote_ids(&self) -> &[String] {
        &self.calibration_quote_ids
    }
}

impl HullWhite<f64> {
    /// Computes the `A(t,T)` function used in the affine ZCB formula.
    #[allow(non_snake_case)]
    pub fn A(&self, t: f64, T: f64, sigma: f64) -> f64 {
        let B = self.B(t, T);
        let exp_term = (-B * self.alpha - (sigma * sigma * B * B) / 2.0).exp();
        exp_term
    }

    /// Computes the `B(t,T)` function used in the affine ZCB formula.
    #[allow(non_snake_case)]
    pub fn B(&self, t: f64, T: f64) -> f64 {
        (1.0 - (-self.alpha * (T - t)).exp()) / self.alpha
    }

    /// Returns the price of a zero-coupon bond at time `t` maturing at `T` given the short rate `r_t`.
    #[allow(non_snake_case)]
    pub fn zcb_price(&self, r_t: f64, t: f64, T: f64, sigma: f64) -> f64 {
        self.A(t, T, sigma) * (-self.B(t, T) * r_t).exp()
    }

    /// Computes θ(t) = d/dt ln P(0,t) + α r(t).
    #[allow(non_snake_case)]
    pub fn theta(&self, t: f64, r_t: f64, sigma: f64) -> f64 {
        let B = self.B(0.0, t);
        let A = self.A(0.0, t, sigma);
        let dr_dt = (A * (-B * r_t).exp() * (-B * r_t).exp() * B * B * sigma * sigma) / 2.0;
        dr_dt + self.alpha * r_t
    }

    /// Computes φ(t) = θ(t) − α r(t).
    pub fn phi(&self, t: f64, r_t: f64, sigma: f64) -> f64 {
        self.theta(t, r_t, sigma) - self.alpha * r_t
    }

    /// Variance of the short rate between `t` and `T`.
    #[allow(non_snake_case)]
    pub fn variance(&self, t: f64, T: f64, sigma: f64) -> f64 {
        let B = self.B(t, T);
        (sigma * sigma * B * B) / (2.0 * self.alpha)
    }

    /// Caplet price under the Hull-White model.
    #[allow(non_snake_case)]
    pub fn caplet_price(&self, strike: f64, t: f64, T: f64, r_t: f64, sigma: f64) -> f64 {
        let forward_rate = (1.0 / self.zcb_price(r_t, t, T, sigma) - 1.0) / (T - t);
        let d1 = (forward_rate / strike).ln() + (sigma * sigma * (T - t) / 2.0);
        let d2 = d1 - sigma * (T - t).sqrt();
        forward_rate * norm_cdf(d1) - strike * norm_cdf(d2)
    }

    /// Floorlet price under the Hull-White model.
    #[allow(non_snake_case)]
    pub fn floorlet_price(&self, strike: f64, t: f64, T: f64, r_t: f64, sigma: f64) -> f64 {
        let forward_rate = (1.0 / self.zcb_price(r_t, t, T, sigma) - 1.0) / (T - t);
        let d1 = (forward_rate / strike).ln() + (sigma * sigma * (T - t) / 2.0);
        let d2 = d1 - sigma * (T - t).sqrt();
        strike * norm_cdf(-d2) - forward_rate * norm_cdf(-d1)
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
    ) -> f64 {
        let forward_rate = (1.0 / self.zcb_price(r_t, t, T, sigma) - 1.0) / (T - t);
        let d1 = (forward_rate / strike).ln() + (sigma * sigma * (T - t) / 2.0);
        let d2 = d1 - sigma * (T - t).sqrt();
        swap_annuity * (forward_rate * norm_cdf(d1) - strike * norm_cdf(d2))
    }
}

// ═════════════════════════════════════════════════════════════════════════
//  AD-enabled version using DualFwd (expression templates + .into())
// ═════════════════════════════════════════════════════════════════════════

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

    /// Computes θ(t) (AD-enabled).
    #[allow(non_snake_case)]
    pub fn theta(&self, t: f64, r_t: DualFwd, sigma: DualFwd) -> DualFwd {
        let B = self.B(0.0, t);
        let A = self.A(0.0, t, sigma);
        let dr_dt: DualFwd =
            (A * (-B * r_t).exp() * (-B * r_t).exp() * B * B * sigma * sigma / 2.0).into();
        (dr_dt + self.alpha * r_t).into()
    }

    /// Computes φ(t) (AD-enabled).
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
        let one: DualFwd = 1.0.into();
        let tau = T - t;
        let forward_rate: DualFwd = ((one / zcb - 1.0) / tau).into();
        let d1: DualFwd = ((forward_rate / strike).ln() + sigma * sigma * tau / 2.0).into();
        let d2: DualFwd = (d1 - sigma * tau.sqrt()).into();
        (forward_rate * norm_cdf(d1) - norm_cdf(d2) * strike).into()
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
        let one: DualFwd = 1.0.into();
        let tau = T - t;
        let forward_rate: DualFwd = ((one / zcb - 1.0) / tau).into();
        let d1: DualFwd = ((forward_rate / strike).ln() + sigma * sigma * tau / 2.0).into();
        let d2: DualFwd = (d1 - sigma * tau.sqrt()).into();
        let neg_d1: DualFwd = (-d1).into();
        let neg_d2: DualFwd = (-d2).into();
        (norm_cdf(neg_d2) * strike - forward_rate * norm_cdf(neg_d1)).into()
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
        let one: DualFwd = 1.0.into();
        let tau = T - t;
        let forward_rate: DualFwd = ((one / zcb - 1.0) / tau).into();
        let d1: DualFwd = ((forward_rate / strike).ln() + sigma * sigma * tau / 2.0).into();
        let d2: DualFwd = (d1 - sigma * tau.sqrt()).into();
        (swap_annuity * (forward_rate * norm_cdf(d1) - norm_cdf(d2) * strike)).into()
    }
}

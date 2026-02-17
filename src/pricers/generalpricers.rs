use crate::{
    ad::adreal::{ADReal, FloatExt, IsReal},
    math::probability::norm_cdf::norm_cdf,
};

/// Closed-form pricer trait.
pub trait CloseFormPricer {}
/// Monte Carlo pricer trait.
pub trait MonteCarloPricer {}

/// PDE pricer trait.
pub trait PDEPricer {}
/// Backward evolution pricer trait.
pub trait BackwardEvolutionPricer {}

/// Black-Scholes closed-form pricer.
#[derive(Clone, Copy, Debug, Default)]
pub struct BlackClosedFormPricer;
impl CloseFormPricer for BlackClosedFormPricer {}

impl BlackClosedFormPricer {
    /// Computes d1 and d2 for Black-style formulas.
    #[must_use]
    pub fn d1_d2(fwd: ADReal, strike: ADReal, vol: ADReal, tau: f64) -> (ADReal, ADReal) {
        let vol_sqrt_tau = vol * tau.sqrt();
        let d1: ADReal =
            (((fwd / strike).ln() + vol * vol * 0.5 * tau) / vol_sqrt_tau.clone()).into();
        let d2: ADReal = (d1 - vol_sqrt_tau).into();
        (d1, d2)
    }

    /// Returns undiscounted Black call/put price from a forward.
    #[must_use]
    pub fn black_forward_price(
        fwd: ADReal,
        strike: ADReal,
        vol: ADReal,
        tau: f64,
        is_call: bool,
    ) -> ADReal {
        let (d1, d2) = Self::d1_d2(fwd, strike, vol, tau);
        let nd1 = norm_cdf(d1.value());
        let nd2 = norm_cdf(d2.value());
        let nmd1 = norm_cdf(-d1.value());
        let nmd2 = norm_cdf(-d2.value());

        if is_call {
            (fwd * nd1 - strike * nd2).into()
        } else {
            (strike * nmd2 - fwd * nmd1).into()
        }
    }
}

/// Black-Scholes Monte Carlo pricer.
#[derive(Clone, Copy, Debug, Default)]
pub struct BlackMonteCarloPricer;
impl MonteCarloPricer for BlackMonteCarloPricer {}

/// Normal (Bachelier) closed-form pricer.
pub struct NormalClosedFormPricer;
impl CloseFormPricer for NormalClosedFormPricer {}

/// Hull-White closed-form pricer.
pub struct HullWhiteClosedFormPricer;
impl CloseFormPricer for HullWhiteClosedFormPricer {}

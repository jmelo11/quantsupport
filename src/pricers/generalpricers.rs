use crate::ad::adreal::{ADReal, FloatExt, IsReal};

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
    fn norm_cdf_ad(x: ADReal) -> ADReal {
        let one: ADReal = 1.0.into();
        let l = x.abs();
        let k: ADReal = (one / (one + l.clone() * 0.231_641_9)).into();
        let poly: ADReal =
            (((((k * 1.330_274_429 - 1.821_255_978) * k + 1.781_477_937) * k - 0.356_563_782) * k
                + 0.319_381_530)
                * k)
                .into();
        let pdf: ADReal = ((-(l.clone() * l) * 0.5).exp() * 0.398_942_280_401_432_7).into();
        let w: ADReal = (one - pdf * poly).into();

        if x.value() < 0.0 {
            (one - w).into()
        } else {
            w
        }
    }

    /// Computes d1 and d2 for Black-style formulas.
    #[must_use]
    pub fn d1_d2(fwd: ADReal, strike: f64, vol: ADReal, tau: f64) -> (ADReal, ADReal) {
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
        strike: f64,
        vol: ADReal,
        tau: f64,
        is_call: bool,
    ) -> ADReal {
        let (d1, d2) = Self::d1_d2(fwd, strike, vol, tau);
        let nd1 = Self::norm_cdf_ad(d1);
        let nd2 = Self::norm_cdf_ad(d2);
        let nmd1 = Self::norm_cdf_ad((-d1).into());
        let nmd2 = Self::norm_cdf_ad((-d2).into());

        if is_call {
            (fwd * nd1 - nd2 * strike).into()
        } else {
            (nmd2 * strike - fwd * nmd1).into()
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

#[cfg(test)]
mod tests {
    use crate::ad::{adreal::{ADReal, IsReal}, tape::Tape};

    use super::BlackClosedFormPricer;

    #[test]
    fn black_option_ad_sensitivities_match_bump_and_reprice() {
        let fwd = 102.0;
        let strike = 100.0;
        let vol = 0.24;
        let tau = 0.75;

        Tape::start_recording();

        let mut fwd_ad = ADReal::new(fwd);
        let mut vol_ad = ADReal::new(vol);
        fwd_ad.put_on_tape();
        vol_ad.put_on_tape();

        let value = BlackClosedFormPricer::black_forward_price(fwd_ad, strike, vol_ad, tau, true);
        let bw = value.backward();
        assert!(bw.is_ok());

        let ad_delta = fwd_ad.adjoint();
        let ad_vega = vol_ad.adjoint();
        assert!(ad_delta.is_ok());
        assert!(ad_vega.is_ok());

        let bump = 1e-5;
        let pv_up_f = BlackClosedFormPricer::black_forward_price(
            ADReal::new(fwd + bump),
            strike,
            ADReal::new(vol),
            tau,
            true,
        )
        .value();
        let pv_dn_f = BlackClosedFormPricer::black_forward_price(
            ADReal::new(fwd - bump),
            strike,
            ADReal::new(vol),
            tau,
            true,
        )
        .value();

        let pv_up_v = BlackClosedFormPricer::black_forward_price(
            ADReal::new(fwd),
            strike,
            ADReal::new(vol + bump),
            tau,
            true,
        )
        .value();
        let pv_dn_v = BlackClosedFormPricer::black_forward_price(
            ADReal::new(fwd),
            strike,
            ADReal::new(vol - bump),
            tau,
            true,
        )
        .value();

        let fd_delta = (pv_up_f - pv_dn_f) / (2.0 * bump);
        let fd_vega = (pv_up_v - pv_dn_v) / (2.0 * bump);

        let ad_delta = ad_delta.unwrap_or_default();
        let ad_vega = ad_vega.unwrap_or_default();

        assert!((ad_delta - fd_delta).abs() < 1e-4);
        assert!((ad_vega - fd_vega).abs() < 1e-4);
    }
}

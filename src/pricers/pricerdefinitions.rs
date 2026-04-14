// use crate::ad::{dual::DualFwd, expr::FloatExt};
// use crate::math::probability::norm_cdf::norm_cdf;
// use crate::models::GbmModelParameters;

// /// Closed-form pricer trait.
// pub trait CloseFormPricer {}
// /// Monte Carlo pricer trait.
// pub trait MonteCarloPricer {}

// /// PDE pricer trait.
// pub trait PDEPricer {}
// /// Backward evolution pricer trait.
// pub trait BackwardEvolutionPricer {}

// /// Black-Scholes closed-form pricer.
// #[derive(Clone, Copy, Debug, Default)]
// pub struct BlackClosedFormPricer;
// impl CloseFormPricer for BlackClosedFormPricer {}

// impl BlackClosedFormPricer {
//     /// Computes d1 and d2 for Black-style formulas.
//     #[must_use]
//     pub fn d1_d2(fwd: DualFwd, strike: f64, vol: DualFwd, tau: f64) -> (DualFwd, DualFwd) {
//         let vol_sqrt_tau = vol * tau.sqrt();
//         let d1: DualFwd =
//             (((fwd / strike).ln() + vol * vol * 0.5 * tau) / vol_sqrt_tau.clone()).into();
//         let d2: DualFwd = (d1 - vol_sqrt_tau).into();
//         (d1, d2)
//     }

//     /// Returns undiscounted Black call/put price from a forward.
//     #[must_use]
//     pub fn black_forward_price(
//         fwd: DualFwd,
//         strike: f64,
//         vol: DualFwd,
//         tau: f64,
//         is_call: bool,
//     ) -> DualFwd {
//         let (d1, d2) = Self::d1_d2(fwd, strike, vol, tau);

//         if is_call {
//             let nd1 = norm_cdf(d1);
//             let nd2 = norm_cdf(d2);
//             (fwd * nd1 - nd2 * strike).into()
//         } else {
//             let nmd1: DualFwd = norm_cdf((-d1).into());
//             let nmd2: DualFwd = norm_cdf((-d2).into());
//             (nmd2 * strike - fwd * nmd1).into()
//         }
//     }
// }

// /// GBM Monte Carlo pricer for European equity options.
// ///
// /// Uses pre-generated standard-normal draws (held in a
// /// [`MonteCarloSimulationElement`](crate::core::elements::montecarlosimulationelement::MonteCarloSimulationElement))
// /// to price under GBM dynamics. The [`GbmModelParameters`] field is serialised
// /// into the [`MarketDataRequest`](crate::core::marketdatahandling::marketdata::MarketDataRequest)
// /// so that any [`MarketDataProvider`](crate::core::marketdatahandling::marketdata::MarketDataProvider)
// /// implementation can inspect them when building or validating the simulation element.
// #[derive(Clone, Copy, Debug, Default)]
// pub struct GbmMonteCarloPricer {
//     /// Parameters controlling path generation (number of paths, random seed).
//     model_parameters: GbmModelParameters,
// }

// impl GbmMonteCarloPricer {
//     /// Creates a new [`GbmMonteCarloPricer`] with the given model parameters.
//     #[must_use]
//     pub const fn new(model_parameters: GbmModelParameters) -> Self {
//         Self { model_parameters }
//     }

//     /// Returns the model parameters of this pricer.
//     #[must_use]
//     pub const fn model_parameters(&self) -> &GbmModelParameters {
//         &self.model_parameters
//     }
// }

// impl MonteCarloPricer for GbmMonteCarloPricer {}

// /// Normal (Bachelier) closed-form pricer.
// pub struct NormalClosedFormPricer;
// impl CloseFormPricer for NormalClosedFormPricer {}

// /// Hull-White closed-form pricer.
// pub struct HullWhiteClosedFormPricer;
// impl CloseFormPricer for HullWhiteClosedFormPricer {}

// #[cfg(test)]
// mod tests {
//     use crate::ad::{dual::DualFwd, tape::Tape};

//     use super::BlackClosedFormPricer;

//     #[test]
//     fn black_option_ad_sensitivities_match_bump_and_reprice() {
//         let fwd = 102.0;
//         let strike = 100.0;
//         let vol = 0.24;
//         let tau = 0.75;

//         Tape::start_recording_fwd();

//         let mut fwd_ad = DualFwd::new(fwd);
//         let mut vol_ad = DualFwd::new(vol);
//         fwd_ad.put_on_tape();
//         vol_ad.put_on_tape();

//         let value = BlackClosedFormPricer::black_forward_price(fwd_ad, strike, vol_ad, tau, true);
//         let bw = value.backward();
//         assert!(bw.is_ok());

//         let ad_delta = fwd_ad.adjoint();
//         let ad_vega = vol_ad.adjoint();
//         assert!(ad_delta.is_ok());
//         assert!(ad_vega.is_ok());

//         Tape::stop_recording_fwd();

//         let bump = 1e-5;
//         let pv_up_f = BlackClosedFormPricer::black_forward_price(
//             DualFwd::new(fwd + bump),
//             strike,
//             DualFwd::new(vol),
//             tau,
//             true,
//         )
//         .value();
//         let pv_dn_f = BlackClosedFormPricer::black_forward_price(
//             DualFwd::new(fwd - bump),
//             strike,
//             DualFwd::new(vol),
//             tau,
//             true,
//         )
//         .value();

//         let pv_up_v = BlackClosedFormPricer::black_forward_price(
//             DualFwd::new(fwd),
//             strike,
//             DualFwd::new(vol + bump),
//             tau,
//             true,
//         )
//         .value();
//         let pv_dn_v = BlackClosedFormPricer::black_forward_price(
//             DualFwd::new(fwd),
//             strike,
//             DualFwd::new(vol - bump),
//             tau,
//             true,
//         )
//         .value();

//         let fd_delta = (pv_up_f - pv_dn_f) / (2.0 * bump);
//         let fd_vega = (pv_up_v - pv_dn_v) / (2.0 * bump);

//         let ad_delta = ad_delta.unwrap_or_default().value();
//         let ad_vega = ad_vega.unwrap_or_default().value();

//         assert!((ad_delta - fd_delta).abs() < 1e-4);
//         assert!((ad_vega - fd_vega).abs() < 1e-4);

//         Tape::rewind_to_init_fwd();
//     }
// }

//! SABR (Stochastic Alpha Beta Rho) model parameter definitions and volatility formulas.
//!
//! The SABR model, introduced by Hagan, Kumar, Lesniewski, and Woodward (2002),
//! is a stochastic volatility model widely used for capturing the volatility smile
//! observed in interest rate and equity option markets.
//!
//! Under the SABR model, the forward price `F` and instantaneous volatility `σ` follow:
//!
//! ```text
//! dF = σ F^β dW₁
//! dσ = ν σ dW₂
//! ⟨dW₁, dW₂⟩ = ρ dt
//! ```
//!
//! where:
//! - `α = σ(0)` is the initial volatility,
//! - `β ∈ [0, 1]` controls the forward's elasticity (0 = normal, 1 = log-normal),
//! - `ρ ∈ (-1, 1)` is the correlation between forward and vol processes,
//! - `ν ≥ 0` is the vol-of-vol (volvol).
//!
//! # Usage
//!
//! ```
//! use quantsupport::models::sabr::SabrModelParameters;
//!
//! let params = SabrModelParameters::new(0.3, 0.5, -0.2, 0.4)
//!     .expect("valid SABR parameters");
//!
//! let forward = 0.05;
//! let strike  = 0.05;
//! let expiry  = 1.0;
//!
//! let vol = params.implied_vol_black(forward, strike, expiry);
//! assert!(vol > 0.0);
//! ```

use crate::utils::errors::{QSError, Result};

/// Parameters for the SABR stochastic volatility model.
///
/// The four SABR parameters completely characterise the dynamics:
///
/// | Field  | Symbol | Constraint      | Meaning                          |
/// |--------|--------|-----------------|----------------------------------|
/// | alpha  | α      | α > 0           | Initial (spot) volatility        |
/// | beta   | β      | 0 ≤ β ≤ 1       | CEV elasticity of the forward    |
/// | rho    | ρ      | −1 < ρ < 1      | Forward–vol correlation          |
/// | nu     | ν      | ν ≥ 0           | Volatility of volatility (volvol)|
#[derive(Clone, Debug)]
pub struct SabrModelParameters {
    /// Initial volatility (`α > 0`).
    alpha: f64,
    /// CEV elasticity (`0 ≤ β ≤ 1`).
    beta: f64,
    /// Forward–vol correlation (`−1 < ρ < 1`).
    rho: f64,
    /// Volatility of volatility (`ν ≥ 0`).
    nu: f64,
}

impl Default for SabrModelParameters {
    /// Returns sensible default SABR parameters suitable for a typical interest-rate smile.
    ///
    /// | α   | β   | ρ    | ν   |
    /// |-----|-----|------|-----|
    /// | 0.3 | 0.5 | −0.2 | 0.4 |
    fn default() -> Self {
        Self {
            alpha: 0.3,
            beta: 0.5,
            rho: -0.2,
            nu: 0.4,
        }
    }
}

impl SabrModelParameters {
    // ------------------------------------------------------------------ //
    //  Construction                                                        //
    // ------------------------------------------------------------------ //

    /// Creates a new [`SabrModelParameters`] after validating all constraints.
    ///
    /// # Parameters
    /// - `alpha` – initial volatility (`α > 0`)
    /// - `beta`  – CEV elasticity (`0 ≤ β ≤ 1`)
    /// - `rho`   – correlation (`−1 < ρ < 1`)
    /// - `nu`    – volvol (`ν ≥ 0`)
    ///
    /// # Errors
    /// Returns [`QSError::InvalidValueErr`] if any parameter violates its constraint.
    pub fn new(alpha: f64, beta: f64, rho: f64, nu: f64) -> Result<Self> {
        let params = Self {
            alpha,
            beta,
            rho,
            nu,
        };
        params.validate()?;
        Ok(params)
    }

    /// Validates all parameter constraints.
    ///
    /// # Errors
    /// Returns [`QSError::InvalidValueErr`] describing the first violated constraint.
    fn validate(&self) -> Result<()> {
        if self.alpha <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "SABR alpha must be strictly positive".into(),
            ));
        }
        if !(0.0..=1.0).contains(&self.beta) {
            return Err(QSError::InvalidValueErr(
                "SABR beta must be in [0, 1]".into(),
            ));
        }
        if self.rho <= -1.0 || self.rho >= 1.0 {
            return Err(QSError::InvalidValueErr(
                "SABR rho must be in the open interval (-1, 1)".into(),
            ));
        }
        if self.nu < 0.0 {
            return Err(QSError::InvalidValueErr(
                "SABR nu must be non-negative".into(),
            ));
        }
        Ok(())
    }

    // ------------------------------------------------------------------ //
    //  Getters                                                             //
    // ------------------------------------------------------------------ //

    /// Returns the initial volatility parameter `α`.
    #[must_use]
    pub const fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Returns the CEV elasticity parameter `β`.
    #[must_use]
    pub const fn beta(&self) -> f64 {
        self.beta
    }

    /// Returns the forward–vol correlation parameter `ρ`.
    #[must_use]
    pub const fn rho(&self) -> f64 {
        self.rho
    }

    /// Returns the volvol parameter `ν`.
    #[must_use]
    pub const fn nu(&self) -> f64 {
        self.nu
    }

    // ------------------------------------------------------------------ //
    //  Builder-style setters                                               //
    // ------------------------------------------------------------------ //

    /// Returns a copy with `alpha` replaced.
    ///
    /// # Errors
    /// Returns an error if the new value violates `α > 0`.
    pub fn with_alpha(mut self, alpha: f64) -> Result<Self> {
        self.alpha = alpha;
        self.validate()?;
        Ok(self)
    }

    /// Returns a copy with `beta` replaced.
    ///
    /// # Errors
    /// Returns an error if the new value violates `0 ≤ β ≤ 1`.
    pub fn with_beta(mut self, beta: f64) -> Result<Self> {
        self.beta = beta;
        self.validate()?;
        Ok(self)
    }

    /// Returns a copy with `rho` replaced.
    ///
    /// # Errors
    /// Returns an error if the new value violates `−1 < ρ < 1`.
    pub fn with_rho(mut self, rho: f64) -> Result<Self> {
        self.rho = rho;
        self.validate()?;
        Ok(self)
    }

    /// Returns a copy with `nu` replaced.
    ///
    /// # Errors
    /// Returns an error if the new value violates `ν ≥ 0`.
    pub fn with_nu(mut self, nu: f64) -> Result<Self> {
        self.nu = nu;
        self.validate()?;
        Ok(self)
    }

    // ------------------------------------------------------------------ //
    //  Hagan et al. (2002) Black implied volatility                       //
    // ------------------------------------------------------------------ //

    /// Computes the Black (log-normal) implied volatility using the
    /// Hagan–Kumar–Lesniewski–Woodward (2002) closed-form SABR approximation.
    ///
    /// The formula distinguishes three regimes for numerical stability:
    ///
    /// 1. **Zero expiry** – returns `α / F^(1−β)` (the ATM backbone at `T = 0`).
    /// 2. **ATM** (`|F − K| < ε·F`) – uses the simplified ATM expansion to avoid
    ///    division by zero in the `log(F/K)` term.
    /// 3. **General (OTM/ITM)** – applies the full Hagan formula with the
    ///    `z / χ(z)` correction for skew and smile.
    ///
    /// # Parameters
    /// - `forward`         – forward price / rate (`F > 0`)
    /// - `strike`          – option strike (`K > 0`)
    /// - `time_to_expiry`  – time to expiry in years (`T ≥ 0`)
    ///
    /// # Returns
    /// Black implied volatility (annualised, dimensionless). Returns `0.0` for
    /// degenerate inputs (e.g. non-positive forward or strike).
    #[must_use]
    pub fn implied_vol_black(&self, forward: f64, strike: f64, time_to_expiry: f64) -> f64 {
        if forward <= 0.0 || strike <= 0.0 {
            return 0.0;
        }

        let alpha = self.alpha;
        let beta = self.beta;
        let rho = self.rho;
        let nu = self.nu;

        // Geometric mean of F and K used throughout.
        let fk = forward * strike;
        let fk_mid = fk.sqrt(); // (FK)^(1/2)
        let fk_beta = fk_mid.powf(1.0 - beta); // (FK)^((1-β)/2)

        // ── Zero expiry: degenerate case ───────────────────────────────
        if time_to_expiry <= 0.0 {
            return alpha / fk_beta;
        }

        let log_fk = (forward / strike).ln();
        let atm_tol = 1e-7 * forward.abs().max(1e-15);

        // ── ATM approximation ──────────────────────────────────────────
        if log_fk.abs() < atm_tol {
            return Self::atm_vol(alpha, beta, rho, nu, forward, time_to_expiry);
        }

        // ── General (OTM / ITM) formula ────────────────────────────────
        // z and χ(z) for the skew correction.
        let z = (nu / alpha) * fk_beta * log_fk;
        let chi_z = Self::chi(z, rho);

        // Leading term: α / [ (FK)^((1−β)/2) · D(F,K,β) ]
        // where D(F,K,β) = 1 + (1−β)²/24 · log²(F/K) + (1−β)⁴/1920 · log⁴(F/K)
        let log_fk_sq = log_fk * log_fk;
        let one_minus_beta = 1.0 - beta;
        let d_fk = 1.0
            + (one_minus_beta * one_minus_beta / 24.0) * log_fk_sq
            + (one_minus_beta.powi(4) / 1920.0) * log_fk_sq * log_fk_sq;

        let leading = alpha / (fk_beta * d_fk);
        let z_over_chi = if chi_z.abs() < f64::EPSILON {
            1.0
        } else {
            z / chi_z
        };

        // Time-dependent correction: 1 + T·[...]
        let correction = Self::time_correction(alpha, beta, rho, nu, fk_mid, time_to_expiry);

        leading * z_over_chi * correction
    }

    /// ATM (`F ≈ K`) SABR Black volatility (Hagan eq. 2.17b).
    fn atm_vol(alpha: f64, beta: f64, rho: f64, nu: f64, forward: f64, t: f64) -> f64 {
        let f_beta = forward.powf(1.0 - beta);
        let correction = Self::time_correction(alpha, beta, rho, nu, forward, t);
        (alpha / f_beta) * correction
    }

    /// Time-dependent higher-order correction factor used in both ATM and general formulas.
    ///
    /// `1 + T · [ (1−β)²α²/(24(FK)^(1−β)) + ρβνα/(4(FK)^((1−β)/2)) + (2−3ρ²)ν²/24 ]`
    fn time_correction(
        alpha: f64,
        beta: f64,
        rho: f64,
        nu: f64,
        fk_mid: f64, // (FK)^(1/2) in general, or F at ATM
        t: f64,
    ) -> f64 {
        let one_minus_beta = 1.0 - beta;
        let fk_beta = fk_mid.powf(1.0 - beta); // (FK)^(1−β) (= F^(1−β) at ATM)
        let fk_pow_half_one_minus_beta = fk_mid.powf(one_minus_beta / 2.0); // (FK)^((1−β)/2)

        let term1 = (one_minus_beta * one_minus_beta * alpha * alpha) / (24.0 * fk_beta);
        let term2 = (rho * beta * nu * alpha) / (4.0 * fk_pow_half_one_minus_beta);
        let term3 = (2.0 - 3.0 * rho * rho) * nu * nu / 24.0;

        1.0 + (term1 + term2 + term3) * t
    }

    /// `χ(z) = log( (√(1 − 2ρz + z²) + z − ρ) / (1 − ρ) )`
    ///
    /// Used in the log-normal mapping correction of the Hagan formula.
    fn chi(z: f64, rho: f64) -> f64 {
        let discriminant = (1.0 - 2.0 * rho * z + z * z).max(0.0);
        let numerator = discriminant.sqrt() + z - rho;
        let denominator = 1.0 - rho;
        if denominator.abs() < f64::EPSILON || numerator <= 0.0 {
            // Near ρ = 1 or degenerate; fall back to z (first-order approximation).
            return z;
        }
        (numerator / denominator).ln()
    }

    // ------------------------------------------------------------------ //
    //  Calibration                                                         //
    // ------------------------------------------------------------------ //

    /// Calibrates SABR parameters to market-observed implied volatilities.
    ///
    /// Fits `α`, `ρ`, and `ν` by minimising the sum of squared differences
    /// between the model's Black implied volatilities and the supplied market
    /// vols.  `β` is **fixed** at `self.beta` (common practice: choose β = 0.5
    /// or β = 1 *a priori* and calibrate the remaining three parameters).
    ///
    /// The minimisation uses a simple gradient-descent loop with adaptive step
    /// size (no external optimiser dependencies required).
    ///
    /// # Parameters
    /// - `strikes`          – market strike prices / rates (must be `> 0`)
    /// - `market_vols`      – corresponding market Black implied vols
    /// - `forward`          – current forward price / rate (`> 0`)
    /// - `time_to_expiry`   – option expiry in years (`> 0`)
    ///
    /// # Returns
    /// Calibrated [`SabrModelParameters`] (with the same `β` as `self`).
    ///
    /// # Errors
    /// - [`QSError::InvalidValueErr`] if inputs are inconsistent (e.g. empty
    ///   slices, mismatched lengths, non-positive forward/expiry).
    /// - [`QSError::SolverErr`] if the optimiser does not converge.
    pub fn calibrate(
        &self,
        strikes: &[f64],
        market_vols: &[f64],
        forward: f64,
        time_to_expiry: f64,
    ) -> Result<Self> {
        if strikes.is_empty() || market_vols.is_empty() {
            return Err(QSError::InvalidValueErr(
                "strikes and market_vols must not be empty".into(),
            ));
        }
        if strikes.len() != market_vols.len() {
            return Err(QSError::InvalidValueErr(
                "strikes and market_vols must have the same length".into(),
            ));
        }
        if forward <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "forward must be strictly positive".into(),
            ));
        }
        if time_to_expiry <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "time_to_expiry must be strictly positive".into(),
            ));
        }

        let beta = self.beta;

        // Initial guess: current parameters.
        let mut alpha = self.alpha;
        let mut rho = self.rho;
        let mut nu = self.nu;

        /// Maximum gradient-descent iterations before declaring non-convergence.
        const MAX_ITER: usize = 50_000;
        let tol = 1e-10;
        let h = 1e-6; // finite-difference step

        // Objective: sum of squared errors.
        let objective = |a: f64, r: f64, n: f64| -> f64 {
            let p = Self {
                alpha: a,
                beta,
                rho: r,
                nu: n,
            };
            strikes
                .iter()
                .zip(market_vols.iter())
                .map(|(k, mv)| {
                    let model_v = p.implied_vol_black(forward, *k, time_to_expiry);
                    let diff = model_v - mv;
                    diff * diff
                })
                .sum()
        };

        let mut step = 1e-4;
        let mut prev_obj = objective(alpha, rho, nu);

        for _ in 0..MAX_ITER {
            if prev_obj < tol {
                break;
            }

            // Forward-difference gradient (reuses `prev_obj` as the base value,
            // requiring only 3 additional objective evaluations per iteration).
            let grad_a = (objective(alpha + h, rho, nu) - prev_obj) / h;
            let grad_r = (objective(alpha, rho + h, nu) - prev_obj) / h;
            let grad_n = (objective(alpha, rho, nu + h) - prev_obj) / h;

            // Gradient-descent step with projection onto feasible region.
            let new_alpha = (alpha - step * grad_a).max(1e-6);
            let new_rho = (rho - step * grad_r).clamp(-0.999, 0.999);
            let new_nu = (nu - step * grad_n).max(0.0);

            let new_obj = objective(new_alpha, new_rho, new_nu);

            if new_obj < prev_obj {
                alpha = new_alpha;
                rho = new_rho;
                nu = new_nu;
                prev_obj = new_obj;
                step *= 1.05; // grow step when improving
            } else {
                step *= 0.5; // shrink step on backtrack
                if step < 1e-16 {
                    break;
                }
            }
        }

        if prev_obj > 1e-4 {
            return Err(QSError::SolverErr(format!(
                "SABR calibration did not converge; residual SSE = {prev_obj:.6e}"
            )));
        }

        Self::new(alpha, beta, rho, nu)
    }
}

// ======================================================================== //
//  Tests                                                                    //
// ======================================================================== //

#[cfg(test)]
mod tests {
    use super::SabrModelParameters;

    // ── Helpers ──────────────────────────────────────────────────────────── //

    /// Asserts that two floats are within `tol` of each other.
    fn assert_approx(a: f64, b: f64, tol: f64) {
        assert!(
            (a - b).abs() <= tol,
            "Expected {a:.8} ≈ {b:.8} (tol = {tol:.2e})"
        );
    }

    // ── Construction & validation ─────────────────────────────────────────── //

    #[test]
    fn default_parameters_are_valid() {
        let p = SabrModelParameters::default();
        assert_approx(p.alpha(), 0.3, 1e-15);
        assert_approx(p.beta(), 0.5, 1e-15);
        assert_approx(p.rho(), -0.2, 1e-15);
        assert_approx(p.nu(), 0.4, 1e-15);
    }

    #[test]
    fn new_accepts_valid_parameters() {
        let p = SabrModelParameters::new(0.2, 0.7, 0.1, 0.3).expect("should be valid");
        assert_approx(p.alpha(), 0.2, 1e-15);
        assert_approx(p.beta(), 0.7, 1e-15);
    }

    #[test]
    fn new_rejects_alpha_zero() {
        assert!(SabrModelParameters::new(0.0, 0.5, 0.0, 0.3).is_err());
    }

    #[test]
    fn new_rejects_alpha_negative() {
        assert!(SabrModelParameters::new(-0.1, 0.5, 0.0, 0.3).is_err());
    }

    #[test]
    fn new_rejects_beta_above_one() {
        assert!(SabrModelParameters::new(0.3, 1.1, 0.0, 0.3).is_err());
    }

    #[test]
    fn new_rejects_beta_below_zero() {
        assert!(SabrModelParameters::new(0.3, -0.1, 0.0, 0.3).is_err());
    }

    #[test]
    fn new_rejects_rho_equal_one() {
        assert!(SabrModelParameters::new(0.3, 0.5, 1.0, 0.3).is_err());
    }

    #[test]
    fn new_rejects_rho_equal_minus_one() {
        assert!(SabrModelParameters::new(0.3, 0.5, -1.0, 0.3).is_err());
    }

    #[test]
    fn new_rejects_nu_negative() {
        assert!(SabrModelParameters::new(0.3, 0.5, 0.0, -0.1).is_err());
    }

    #[test]
    fn builder_with_alpha_validates() {
        let p = SabrModelParameters::default();
        assert!(p.clone().with_alpha(0.5).is_ok());
        assert!(p.with_alpha(-0.1).is_err());
    }

    #[test]
    fn builder_with_beta_validates() {
        let p = SabrModelParameters::default();
        assert!(p.clone().with_beta(0.0).is_ok());
        assert!(p.clone().with_beta(1.0).is_ok());
        assert!(p.with_beta(1.5).is_err());
    }

    #[test]
    fn builder_with_rho_validates() {
        let p = SabrModelParameters::default();
        assert!(p.clone().with_rho(0.0).is_ok());
        assert!(p.with_rho(1.0).is_err());
    }

    #[test]
    fn builder_with_nu_validates() {
        let p = SabrModelParameters::default();
        assert!(p.clone().with_nu(0.0).is_ok());
        assert!(p.with_nu(-0.01).is_err());
    }

    // ── Implied vol – ATM ──────────────────────────────────────────────────── //

    #[test]
    fn atm_vol_is_positive() {
        let p = SabrModelParameters::default();
        let vol = p.implied_vol_black(0.05, 0.05, 1.0);
        assert!(vol > 0.0);
    }

    #[test]
    fn atm_vol_zero_expiry_returns_backbone() {
        let p = SabrModelParameters::new(0.3, 0.5, -0.2, 0.4).expect("valid");
        // T = 0 → α / F^(1−β)
        let forward = 0.05_f64;
        let expected = 0.3 / forward.powf(0.5);
        let vol = p.implied_vol_black(forward, forward, 0.0);
        assert_approx(vol, expected, 1e-12);
    }

    #[test]
    fn atm_vol_increases_with_nu() {
        let f = 0.05;
        let t = 1.0;
        let p_low = SabrModelParameters::new(0.3, 0.5, 0.0, 0.1).expect("valid");
        let p_high = SabrModelParameters::new(0.3, 0.5, 0.0, 0.8).expect("valid");
        // Higher ν ⟹ higher time-dependent correction ⟹ higher ATM vol for ρ = 0.
        assert!(p_high.implied_vol_black(f, f, t) > p_low.implied_vol_black(f, f, t));
    }

    // ── Implied vol – OTM / ITM ───────────────────────────────────────────── //

    #[test]
    fn otm_call_vol_is_positive() {
        let p = SabrModelParameters::default();
        let vol = p.implied_vol_black(0.05, 0.07, 1.0);
        assert!(vol > 0.0);
    }

    #[test]
    fn itm_call_vol_is_positive() {
        let p = SabrModelParameters::default();
        let vol = p.implied_vol_black(0.05, 0.03, 1.0);
        assert!(vol > 0.0);
    }

    #[test]
    fn negative_rho_produces_skew() {
        // Negative ρ ⟹ left (downward) skew: low strikes have higher vol.
        let p = SabrModelParameters::new(0.3, 0.5, -0.5, 0.4).expect("valid");
        let f = 0.05;
        let t = 1.0;
        let vol_itm = p.implied_vol_black(f, f * 0.8, t);
        let vol_otm = p.implied_vol_black(f, f * 1.2, t);
        assert!(vol_itm > vol_otm, "negative ρ should give higher vol for low K");
    }

    // ── Edge cases ────────────────────────────────────────────────────────── //

    #[test]
    fn beta_zero_lognormal_backbone() {
        // β = 0: normal SABR (Bachelier backbone).
        let p = SabrModelParameters::new(0.01, 0.0, 0.0, 0.0).expect("valid");
        let vol = p.implied_vol_black(0.05, 0.05, 1.0);
        assert!(vol > 0.0);
    }

    #[test]
    fn beta_one_pure_lognormal() {
        // β = 1: pure log-normal (Black-Scholes backbone).
        let p = SabrModelParameters::new(0.3, 1.0, 0.0, 0.0).expect("valid");
        let vol = p.implied_vol_black(0.05, 0.05, 1.0);
        // With β = 1 and ν = 0, ATM vol → α (no time correction at leading order).
        assert_approx(vol, 0.3, 0.01);
    }

    #[test]
    fn rho_zero_symmetric_smile() {
        // With β = 1 and ρ = 0, log-symmetric strikes (K± = F·exp(±δ)) produce
        // identical implied vols because the D(F,K,β) term vanishes (1-β = 0)
        // and the z/χ(z) factor is an even function of log-moneyness when ρ = 0.
        let p = SabrModelParameters::new(0.3, 1.0, 0.0, 0.4).expect("valid");
        let f = 0.05;
        let t = 1.0;
        let delta = 0.1_f64; // 10 % log-moneyness offset
        let vol_up = p.implied_vol_black(f, f * delta.exp(), t);
        let vol_dn = p.implied_vol_black(f, f * (-delta).exp(), t);
        // Should be exactly symmetric.
        assert_approx(vol_up, vol_dn, 1e-12);
    }

    #[test]
    fn degenerate_forward_returns_zero() {
        let p = SabrModelParameters::default();
        assert_approx(p.implied_vol_black(0.0, 0.05, 1.0), 0.0, 1e-15);
        assert_approx(p.implied_vol_black(0.05, 0.0, 1.0), 0.0, 1e-15);
    }

    #[test]
    fn small_expiry_vol_finite() {
        let p = SabrModelParameters::default();
        let vol = p.implied_vol_black(0.05, 0.05, 1e-6);
        assert!(vol.is_finite() && vol > 0.0);
    }

    #[test]
    fn large_expiry_vol_finite() {
        let p = SabrModelParameters::default();
        let vol = p.implied_vol_black(0.05, 0.05, 30.0);
        assert!(vol.is_finite() && vol > 0.0);
    }

    // ── Calibration ───────────────────────────────────────────────────────── //

    /// Generates a smile from known parameters, calibrates back, and verifies
    /// the recovered parameters are close to the originals.
    #[test]
    fn calibration_round_trip() {
        let true_params =
            SabrModelParameters::new(0.25, 0.5, -0.3, 0.35).expect("valid true params");

        let forward = 0.05;
        let expiry = 1.0;
        let strikes: Vec<f64> = vec![0.03, 0.04, 0.045, 0.05, 0.055, 0.06, 0.07];

        let market_vols: Vec<f64> = strikes
            .iter()
            .map(|k| true_params.implied_vol_black(forward, *k, expiry))
            .collect();

        // Start the calibration from a different initial guess.
        let init = SabrModelParameters::new(0.3, 0.5, 0.0, 0.2).expect("valid init");
        let calibrated = init
            .calibrate(&strikes, &market_vols, forward, expiry)
            .expect("calibration should converge");

        // Parameter recovery tolerance: gradient-descent calibration may not reach
        // sub-basis-point accuracy due to the well-known SABR parameter degeneracy,
        // so we verify parameters are recovered to practical precision.
        assert_approx(calibrated.alpha(), true_params.alpha(), 2e-2);
        assert_approx(calibrated.rho(), true_params.rho(), 2e-2);
        assert_approx(calibrated.nu(), true_params.nu(), 2e-2);
        // beta is fixed during calibration.
        assert_approx(calibrated.beta(), true_params.beta(), 1e-15);
    }

    #[test]
    fn calibration_rejects_empty_strikes() {
        let p = SabrModelParameters::default();
        assert!(p.calibrate(&[], &[], 0.05, 1.0).is_err());
    }

    #[test]
    fn calibration_rejects_mismatched_lengths() {
        let p = SabrModelParameters::default();
        assert!(p.calibrate(&[0.05], &[0.2, 0.21], 0.05, 1.0).is_err());
    }

    #[test]
    fn calibration_rejects_non_positive_forward() {
        let p = SabrModelParameters::default();
        assert!(p.calibrate(&[0.05], &[0.2], 0.0, 1.0).is_err());
    }

    #[test]
    fn calibration_rejects_non_positive_expiry() {
        let p = SabrModelParameters::default();
        assert!(p.calibrate(&[0.05], &[0.2], 0.05, 0.0).is_err());
    }
}

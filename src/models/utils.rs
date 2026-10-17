//! Common Black-Scholes / Black-76 / Bachelier pricing functions shared
//! across models.
//!
//! Option values are undiscounted, in the same units as the forward and strike.
//! Times are in years. Black volatility is an annualized decimal lognormal
//! volatility; Bachelier volatility has forward units per square-root year.
//! Apply payment discounting and contract scaling separately.

use crate::{
    ad::{dual::DualFwd, expr::FloatExt},
    math::probability::{
        norm_cdf::{norm_cdf, NormCDF},
        norm_pdf::norm_pdf,
    },
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
    utils::errors::{QSError, Result},
};

// ---------------------------------------------------------------------------
// f64 variants
// ---------------------------------------------------------------------------

/// Black forward-price `d1` and `d2`.
///
/// Uses `d1 = ln(fwd/strike)/(vol*sqrt(tau)) + vol*sqrt(tau)/2` and
/// `d2 = d1 - vol*sqrt(tau)`. Inputs should be finite and strictly positive.
/// There is no explicit zero-volatility or zero-expiry limiting branch.
///
/// # Errors
/// Returns an error if `strike`, `tau`, or `vol` are non-positive.
/// Forward positivity and input finiteness are not checked; invalid inputs
/// can produce nonfinite outputs rather than errors.
pub fn d1_d2(fwd: f64, strike: f64, vol: f64, tau: f64) -> Result<(f64, f64)> {
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
    let d1 = (0.5 * vol).mul_add(sqrt_tau, (fwd / strike).ln() / (vol * sqrt_tau));
    let d2 = vol.mul_add(-sqrt_tau, d1);
    Ok((d1, d2))
}

/// Undiscounted Black call price: F·N(d₁) − K·N(d₂).
///
/// # Errors
/// Returns an error if d₁/d₂ computation fails.
pub fn black_call(fwd: f64, strike: f64, vol: f64, tau: f64) -> Result<f64> {
    let (d1, d2) = d1_d2(fwd, strike, vol, tau)?;
    Ok(fwd.mul_add(d1.norm_cdf(), -(strike * d2.norm_cdf())))
}

/// Undiscounted Black put price: K·N(−d₂) − F·N(−d₁).
///
/// # Errors
/// Returns an error if d₁/d₂ computation fails.
pub fn black_put(fwd: f64, strike: f64, vol: f64, tau: f64) -> Result<f64> {
    let (d1, d2) = d1_d2(fwd, strike, vol, tau)?;
    Ok(strike.mul_add((-d2).norm_cdf(), -(fwd * (-d1).norm_cdf())))
}

/// Undiscounted Bachelier (normal-model) call price:
/// `(F − K)·Φ(d) + σ·√τ·φ(d)` with `d = (F − K)/(σ·√τ)`.
///
/// Used to price instruments quoted with a Normal volatility. Unlike Black's
/// formula, forwards and strikes may be zero or negative.
/// `vol` is in the same units as `fwd` per square-root year, not a lognormal
/// percentage. No zero-volatility or zero-expiry limiting branch is provided.
///
/// # Errors
/// Returns an error if `tau` or `vol` are non-positive.
/// Input finiteness is not validated.
pub fn bachelier_call(fwd: f64, strike: f64, vol: f64, tau: f64) -> Result<f64> {
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
    let sigma_sqrt_tau = vol * tau.sqrt();
    let d = (fwd - strike) / sigma_sqrt_tau;
    Ok((fwd - strike).mul_add(norm_cdf(d), sigma_sqrt_tau * norm_pdf(d)))
}

// ---------------------------------------------------------------------------
// DualFwd (AD-enabled) variants
// ---------------------------------------------------------------------------

/// Black forward-price `d1` and `d2` (AD-enabled).
///
/// `fwd` and `vol` carry derivative information; `strike` and `tau` are
/// constants. Uses the same formula and positive finite input domain as
/// [`d1_d2`], but unlike that helper does not reject non-positive volatility.
/// There is no explicit zero-volatility or zero-expiry limiting branch.
///
/// # Errors
/// Returns an error if `strike` or `tau` are non-positive.
/// Forward, volatility, and input finiteness are not checked; invalid inputs
/// can produce nonfinite outputs rather than errors.
pub fn d1_d2_ad(fwd: DualFwd, strike: f64, vol: DualFwd, tau: f64) -> Result<(DualFwd, DualFwd)> {
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

/// Undiscounted Black call price (AD-enabled).
///
/// ## Arguments
/// * `fwd` - Forward price (AD-enabled).
/// * `strike` - Strike price.
/// * `vol` - Annualized decimal lognormal volatility (AD-enabled).
/// * `tau` - Time to expiry in years.
///
/// # Errors
/// Returns an error if d₁/d₂ computation fails.
pub fn black_call_ad(fwd: DualFwd, strike: f64, vol: DualFwd, tau: f64) -> Result<DualFwd> {
    let (d1, d2) = d1_d2_ad(fwd, strike, vol, tau)?;
    Ok((fwd * norm_cdf(d1) - norm_cdf(d2) * strike).into())
}

/// Undiscounted Black put price (AD-enabled).
///
/// ## Arguments
/// * `fwd` - Forward price (AD-enabled).
/// * `strike` - Strike price.
/// * `vol` - Annualized decimal lognormal volatility (AD-enabled).
/// * `tau` - Time to expiry in years.
///
/// # Errors
/// Returns an error if d₁/d₂ computation fails.
pub fn black_put_ad(fwd: DualFwd, strike: f64, vol: DualFwd, tau: f64) -> Result<DualFwd> {
    let (d1, d2) = d1_d2_ad(fwd, strike, vol, tau)?;
    let neg_d2: DualFwd = (-d2).into();
    let neg_d1: DualFwd = (-d1).into();
    Ok((norm_cdf(neg_d2) * strike - fwd * norm_cdf(neg_d1)).into())
}

/// Computes the swap annuity (sum of discount factors times year fractions)
/// for annual payment dates from `start` to `end`.
///
/// # Errors
/// Returns an error if discount factor lookup fails.
pub fn swap_annuity_from_curve(
    curve: &dyn InterestRatesTermStructure<f64>,
    reference_date: Date,
    start: Date,
    end: Date,
    day_counter: DayCounter,
) -> Result<f64> {
    let mut annuity = 0.0;
    let mut date = start;
    let one_year = Period::new(1, TimeUnit::Years);
    while date < end {
        let next = std::cmp::min(date + one_year, end);
        let t = day_counter.year_fraction(reference_date, next);
        let tau = day_counter.year_fraction(date, next);
        annuity = tau.mul_add(curve.discount_factor_from_time(t)?, annuity);
        date = next;
    }
    Ok(annuity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bachelier_call_atm_matches_closed_form() -> Result<()> {
        // ATM Bachelier price = sigma * sqrt(tau) / sqrt(2*pi).
        let (fwd, vol, tau) = (0.03, 0.01, 2.0);
        let price = bachelier_call(fwd, fwd, vol, tau)?;
        let expected = vol * tau.sqrt() / (2.0 * std::f64::consts::PI).sqrt();
        assert!((price - expected).abs() < 1e-15);
        Ok(())
    }

    #[test]
    fn bachelier_call_bounds_and_negative_rates() -> Result<()> {
        // Price is bounded below by intrinsic and works for negative fwd/strike.
        let (fwd, strike, vol, tau) = (-0.005, -0.01, 0.008, 1.5);
        let price = bachelier_call(fwd, strike, vol, tau)?;
        assert!(price > (fwd - strike).max(0.0));
        // Monotone in vol.
        let higher = bachelier_call(fwd, strike, 2.0 * vol, tau)?;
        assert!(higher > price);
        // Invalid inputs error.
        assert!(bachelier_call(fwd, strike, 0.0, tau).is_err());
        assert!(bachelier_call(fwd, strike, vol, 0.0).is_err());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Stress tests: boundary conditions, extreme values, parities
    // -----------------------------------------------------------------------

    #[test]
    fn black_put_call_parity_across_ladder() -> Result<()> {
        // C - P = F - K must hold exactly for every strike/vol/tau combination.
        let fwd = 0.04;
        for strike_mult in [0.25, 0.5, 1.0, 1.5, 4.0] {
            for vol in [0.01, 0.2, 1.0, 3.0] {
                for tau in [0.01, 1.0, 30.0] {
                    let strike = fwd * strike_mult;
                    let call = black_call(fwd, strike, vol, tau)?;
                    let put = black_put(fwd, strike, vol, tau)?;
                    assert!(
                        (call - put - (fwd - strike)).abs() < 1e-14,
                        "parity violated at K={strike}, vol={vol}, tau={tau}"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn black_call_respects_no_arbitrage_bounds_and_extreme_vol_limits() -> Result<()> {
        let fwd = 100.0;
        for strike in [1.0, 50.0, 100.0, 150.0, 10_000.0] {
            for vol in [1e-6, 0.2, 5.0] {
                for tau in [1e-6, 1.0, 50.0] {
                    let call = black_call(fwd, strike, vol, tau)?;
                    let intrinsic = (fwd - strike).max(0.0);
                    assert!(
                        call >= intrinsic - 1e-10 && call <= fwd + 1e-10,
                        "bounds violated: C={call} at K={strike}, vol={vol}, tau={tau}"
                    );
                }
            }
        }
        // vol -> 0: call converges to intrinsic.
        assert!((black_call(fwd, 80.0, 1e-9, 1.0)? - 20.0).abs() < 1e-9);
        assert!(black_call(fwd, 120.0, 1e-9, 1.0)? < 1e-12);
        // total variance -> inf: call converges to F, put to K.
        assert!((black_call(fwd, 80.0, 40.0, 10.0)? - fwd).abs() < 1e-9);
        assert!((black_put(fwd, 80.0, 40.0, 10.0)? - 80.0).abs() < 1e-9);
        Ok(())
    }

    #[test]
    fn black_call_is_monotone_in_strike_vol_and_time() -> Result<()> {
        let fwd = 0.05;
        // Decreasing in strike.
        let mut prev = f64::INFINITY;
        for strike_mult in [0.5, 0.75, 1.0, 1.25, 1.5] {
            let c = black_call(fwd, fwd * strike_mult, 0.3, 2.0)?;
            assert!(c < prev, "call must decrease in strike");
            prev = c;
        }
        // Increasing in vol.
        prev = 0.0;
        for vol in [0.05, 0.1, 0.2, 0.5, 1.0] {
            let c = black_call(fwd, fwd, vol, 2.0)?;
            assert!(c > prev, "call must increase in vol");
            prev = c;
        }
        // Increasing in tau (total variance).
        prev = 0.0;
        for tau in [0.1, 0.5, 1.0, 5.0, 20.0] {
            let c = black_call(fwd, fwd, 0.2, tau)?;
            assert!(c > prev, "ATM call must increase in tau");
            prev = c;
        }
        Ok(())
    }

    #[test]
    fn black_errors_on_degenerate_inputs() {
        assert!(black_call(0.04, 0.0, 0.2, 1.0).is_err());
        assert!(black_call(0.04, -0.01, 0.2, 1.0).is_err());
        assert!(black_call(0.04, 0.04, 0.0, 1.0).is_err());
        assert!(black_call(0.04, 0.04, -0.2, 1.0).is_err());
        assert!(black_call(0.04, 0.04, 0.2, 0.0).is_err());
        assert!(black_put(0.04, 0.04, 0.2, -1.0).is_err());
    }

    #[test]
    fn bachelier_put_call_symmetry_gives_parity() -> Result<()> {
        // In the normal model the put is P(F,K) = C(K,F), so the parity
        // C(F,K) - C(K,F) = F - K must hold, including for negative rates.
        for (fwd, strike) in [(0.04, 0.03), (0.04, 0.06), (-0.01, 0.005), (-0.02, -0.03)] {
            for vol in [0.001, 0.01, 0.10] {
                for tau in [0.1, 2.0, 20.0] {
                    let call = bachelier_call(fwd, strike, vol, tau)?;
                    let reversed = bachelier_call(strike, fwd, vol, tau)?;
                    assert!(
                        (call - reversed - (fwd - strike)).abs() < 1e-14,
                        "normal parity violated at F={fwd}, K={strike}, vol={vol}, tau={tau}"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn bachelier_call_converges_to_intrinsic_at_low_vol() -> Result<()> {
        // ITM: price -> F - K; OTM: price -> 0.
        assert!((bachelier_call(0.05, 0.03, 1e-10, 1.0)? - 0.02).abs() < 1e-12);
        assert!(bachelier_call(0.03, 0.05, 1e-10, 1.0)? < 1e-15);
        Ok(())
    }

    #[test]
    fn black_and_bachelier_agree_atm_for_small_total_vol() -> Result<()> {
        // At the money, sigma_N ~= sigma_B * F for small total variance:
        // both reduce to sigma * sqrt(tau) / sqrt(2 pi) (times F for Black).
        let (fwd, tau) = (0.04, 0.5);
        let sigma_b = 0.05;
        let sigma_n = sigma_b * fwd;
        let black = black_call(fwd, fwd, sigma_b, tau)?;
        let normal = bachelier_call(fwd, fwd, sigma_n, tau)?;
        assert!(
            (black - normal).abs() / black < 1e-3,
            "ATM Black {black} and Bachelier {normal} should agree for small vol"
        );
        Ok(())
    }
}

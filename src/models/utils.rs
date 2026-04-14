//! Common Black-Scholes / Black-76 pricing functions shared across models.

use crate::{
    ad::{dual::DualFwd, expr::FloatExt},
    math::probability::norm_cdf::{norm_cdf, NormCDF},
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
    utils::errors::{QSError, Result},
};

// ---------------------------------------------------------------------------
// f64 variants
// ---------------------------------------------------------------------------

/// Black-Scholes d₁ and d₂.
///
/// # Errors
/// Returns an error if `strike`, `tau`, or `vol` are non-positive.
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

// ---------------------------------------------------------------------------
// DualFwd (AD-enabled) variants
// ---------------------------------------------------------------------------

/// Black-Scholes d₁ and d₂ (AD-enabled).
///
/// `fwd` and `vol` carry derivative information; `strike` and `tau` are
/// constants.
///
/// # Errors
/// Returns an error if `strike` or `tau` are non-positive.
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
/// * `vol` - Volatility (AD-enabled).
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
/// * `vol` - Volatility (AD-enabled).
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
        annuity += tau * curve.discount_factor_from_time(t)?;
        date = next;
    }
    Ok(annuity)
}

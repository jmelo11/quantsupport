use crate::time::period::Period;

/// Per-expiry calibration diagnostics.
#[derive(Clone, Debug)]
pub struct HullWhiteCalibrationRecord {
    /// Quote identifier used for calibration.
    pub identifier: String,
    /// Caplet / swaption expiry period.
    pub expiry: Period,
    /// Year-fraction to expiry.
    pub t: f64,
    /// Year-fraction to payment (caplet) or last swap date (swaption).
    pub big_t: f64,
    /// Market Black or normal volatility, sampled from the configured surface/
    /// cube when configuration-based calibration is used, otherwise the raw quote.
    pub market_vol: f64,
    /// Target price derived from market vol.
    pub market_price: f64,
    /// Instrument price at its independently calibrated constant sigma,
    /// not a repricing under the assembled piecewise schedule.
    pub model_price: f64,
    /// Calibrated HW short-rate volatility.
    pub calibrated_sigma: f64,
    /// Forward rate at the expiry.
    pub forward_rate: f64,
    /// Effective strike used in calibration (after ATM/Relative resolution).
    pub effective_strike: f64,
}

/// Aggregated diagnostics produced by
/// [`HullWhite::calibrate_with_configuration`](crate::models::hullwhite::hullwhitemodel::HullWhite::calibrate_with_configuration).
#[derive(Clone, Debug)]
pub struct HullWhiteCalibrationQuality {
    /// Per-expiry calibration records.
    pub records: Vec<HullWhiteCalibrationRecord>,
}

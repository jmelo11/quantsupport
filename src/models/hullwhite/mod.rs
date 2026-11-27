//! Hull-White one-factor short-rate model.

/// Fits Hull–White parameters to the selected market instruments.
pub mod hullwhitecalibration;
/// Reports the fit errors and diagnostics from Hull–White calibration.
pub mod hullwhitecalibrationquality;
/// Calculates Hull–White bond and option values under the configured parameters.
pub mod hullwhitemodel;

/// Stores the model parameters and their calibrated quote connections.
pub mod parameters;

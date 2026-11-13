//! Calibration framework: traits and processes for model calibration.

/// Trait for pricing calibration instruments.
pub mod calibrationpricer;
/// Trait for calibration processes (residual computation).
pub mod calibrationprocess;

/// Model-implied native calibration quotes.
pub mod impliedquote;

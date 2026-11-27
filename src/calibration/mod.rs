//! Calibration framework: traits and processes for model calibration.

/// Defines how a calibration instrument supplies its model-implied quote.
pub mod calibrationpricer;
/// Defines the residual equations used to fit a model or curve.
pub mod calibrationprocess;

/// Calculates native model-implied quotes for calibration instruments.
pub mod impliedquote;

use crate::{quotes::calibrationinstrument::CalibrationInstrument, utils::errors::Result};

/// A pricer that can compute model-implied values and sensitivities
/// for a [`CalibrationInstrument`].
pub trait CalibrationInstrumentPricer {
    /// Returns the model-implied calibration value for the instrument.
    /// Implementations used with the default [`CalibrationProcess`](crate::calibration::calibrationprocess::CalibrationProcess)
    /// return a zero-target residual and interpret any product-specific
    /// calibration strategy here.
    ///
    /// # Errors
    /// Returns an error if pricing fails.
    fn price(&self, instrument: &CalibrationInstrument) -> Result<f64>;
    /// Returns the sensitivity of the model price w.r.t. the calibration variable.
    ///
    /// # Errors
    /// Returns an error if the sensitivity computation fails.
    fn sensitivity(&self, instrument: &CalibrationInstrument) -> Result<f64>;
}

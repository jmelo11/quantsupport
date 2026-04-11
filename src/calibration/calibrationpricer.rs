use crate::{quotes::calibrationinstrument::CalibrationInstrument, utils::errors::Result};

/// A pricer that can compute model-implied values and sensitivities
/// for a [`CalibrationInstrument`].
pub trait CalibrationInstrumentPricer {
    /// Returns the model-implied price (or rate) for the given instrument.
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

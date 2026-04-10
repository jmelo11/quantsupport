use crate::{quotes::calibrationinstrument::CalibrationInstrument, utils::errors::Result};

/// A pricer that can compute model-implied values and sensitivities
/// for a [`CalibrationInstrument`].
pub trait CalibrationInstrumentPricer {
    /// Returns the model-implied price (or rate) for the given instrument.
    fn price(&self, instrument: &CalibrationInstrument) -> Result<f64>;
    /// Returns the sensitivity of the model price w.r.t. the calibration variable.
    fn sensitivity(&self, instrument: &CalibrationInstrument) -> Result<f64>;
}

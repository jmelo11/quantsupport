use crate::calibration::calibrationpricer::CalibrationInstrumentPricer;
use crate::quotes::calibrationinstrument::CalibrationInstrument;
use crate::utils::errors::Result;

/// A calibration process computes residuals (model − market) for a set of
/// calibration instruments. The default implementation delegates all
/// instrument-specific interpretation to the pricer.
pub trait CalibrationProcess: CalibrationInstrumentPricer {
    /// Computes the residual (model − market) for each calibration instrument.
    ///
    /// # Errors
    /// Returns an error if pricing any instrument fails.
    fn residual(&self, instruments: &[CalibrationInstrument]) -> Result<Vec<f64>> {
        instruments
            .iter()
            .map(|instrument| self.price(instrument))
            .collect()
    }
}

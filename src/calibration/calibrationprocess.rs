use crate::calibration::calibrationpricer::CalibrationInstrumentPricer;
use crate::quotes::calibrationinstrument::CalibrationInstrument;
use crate::utils::errors::Result;

/// Computes zero-target residuals for calibration instruments.
///
/// The default implementation collects each pricer's `price()` output without
/// subtracting a market value. The pricer must already return the desired
/// residual, or the process must override this method.
pub trait CalibrationProcess: CalibrationInstrumentPricer {
    /// Collects pricer outputs as residuals, without further model/market subtraction.
    ///
    /// # Errors
    /// Returns an error if pricing any instrument fails.
    fn residual(&self, instruments: &[CalibrationInstrument]) -> Result<Vec<f64>> {
        instruments
            .iter()
            .map(|instrument| {
                self.price(instrument).map_err(|error| {
                    error.context(format!(
                        "pricing calibration instrument '{}'",
                        instrument.pillar_label()
                    ))
                })
            })
            .collect()
    }
}

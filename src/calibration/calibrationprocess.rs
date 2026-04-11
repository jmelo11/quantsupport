use crate::calibration::calibrationpricer::CalibrationInstrumentPricer;
use crate::quotes::calibrationinstrument::CalibrationInstrument;
use crate::quotes::quote::CalibrationInstrumentType;
use crate::utils::errors::{QSError, Result};

/// A calibration process computes residuals (model − market) for a set of
/// calibration instruments.  The default implementation dispatches by
/// instrument type.
pub trait CalibrationProcess: CalibrationInstrumentPricer {
    /// Computes the residual (model − market) for each calibration instrument.
    ///
    /// # Errors
    /// Returns an error if pricing any instrument fails.
    fn residual(&self, instruments: &[CalibrationInstrument]) -> Result<Vec<f64>> {
        let mut residuals = Vec::new();
        for inst in instruments {
            let res = match inst.built() {
                CalibrationInstrumentType::Swap(_)
                | CalibrationInstrumentType::BasisSwap(_)
                | CalibrationInstrumentType::FixFloatCrossCurrencySwap(_)
                | CalibrationInstrumentType::FloatFloatCrossCurrencySwap(_) => self.price(inst)?,
                CalibrationInstrumentType::FixedRateDeposit(_) => {
                    let implied = self.price(inst)?;
                    implied - inst.quote_value()
                }
                CalibrationInstrumentType::RateFutures(rf) => {
                    let implied = self.price(inst)?;
                    implied - rf.implied_rate()
                }
                // an fx forward should have two legs, one per currency.
                CalibrationInstrumentType::FxForward(fxf) => {
                    // Handle both outright forward prices and forward points.
                    let market_fwd = if let Some(price) = fxf.forward_price() {
                        price
                    } else if let Some(points) = fxf.forward_points() {
                        points
                    } else {
                        return Err(QSError::ValueNotSetErr(
                            "FX forward: neither price nor points set".into(),
                        ));
                    };
                    let implied = self.price(inst)?;
                    implied - market_fwd
                }
                // Vol products: quote_value() is a market vol — the default
                // impl cannot convert it to a price.  Implementors must
                // override residual() for vol-quoted instruments.
                CalibrationInstrumentType::CapletFloorlet(_)
                | CalibrationInstrumentType::CapFloor(_)
                | CalibrationInstrumentType::EuropeanSwaption(_) => {
                    return Err(QSError::InvalidValueErr(
                        "Vol-quoted instruments require an overridden residual() implementation"
                            .into(),
                    ))
                }
                _ => {
                    return Err(QSError::InvalidValueErr(
                        "Unsupported instrument type for calibration residual".into(),
                    ))
                }
            };
            residuals.push(res);
        }
        Ok(residuals)
    }
}

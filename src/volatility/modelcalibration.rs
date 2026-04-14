use serde::{Deserialize, Serialize};

use crate::indices::marketindex::MarketIndex;

/// Specifies which market data object to calibrate against.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CalibrationSource {
    /// Calibrate to caplet vols from a 2-D volatility surface.
    Surface {
        /// Market index identifying the surface to read from.
        market_index: MarketIndex,
    },
    /// Calibrate to swaption vols from a 3-D volatility cube.
    Cube {
        /// Market index identifying the cube to read from.
        market_index: MarketIndex,
    },
}

/// Configuration for model calibration (e.g. Hull-White to caplet/swaption vols).
///
/// Quote identifiers follow the same convention as
/// [`CurveConfiguration`](crate::rates::bootstrapping::curveconfiguration::CurveConfiguration):
/// each string is resolved against a [`QuoteSelector`](crate::quotes::quoteselector::QuoteSelector)
/// to obtain the market quote and instrument details.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelCalibrationConfiguration {
    /// Which vol surface or cube to calibrate against.
    source: CalibrationSource,
    /// Quote identifiers for the calibration instruments (caplets, swaptions, or both).
    quote_ids: Vec<String>,
    /// Mean-reversion speed.
    alpha: f64,
}

impl ModelCalibrationConfiguration {
    /// Creates a new calibration configuration.
    #[must_use]
    pub const fn new(source: CalibrationSource, quote_ids: Vec<String>, alpha: f64) -> Self {
        Self {
            source,
            quote_ids,
            alpha,
        }
    }

    /// Returns the calibration source.
    #[must_use]
    pub const fn source(&self) -> &CalibrationSource {
        &self.source
    }

    /// Returns the calibration quote identifiers.
    #[must_use]
    pub fn quote_ids(&self) -> &[String] {
        &self.quote_ids
    }

    /// Returns the mean-reversion speed.
    #[must_use]
    pub const fn alpha(&self) -> f64 {
        self.alpha
    }
}

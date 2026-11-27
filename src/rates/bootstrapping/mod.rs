/// Provides the instruments used to calibrate bootstrapped curves.
pub mod bootstrapcalibrationinstrument;
/// Provides the discounting policy used while bootstrapping.
pub mod bootstrapdiscountpolicy;
/// Provides the bootstrapped yield-curve representation.
pub mod bootstrappedcurve;
/// Describes one instrument and solved node in a curve bootstrap.
pub mod bootstrapstep;
/// Provides the shared curve-bootstrap utilities.
pub mod bootstraputils;
/// Calibrates survival probabilities to credit quotes and connects their sensitivities to those quotes.
pub mod creditcurvebootstrapper;
/// Describes the credit instruments, recovery and discount curve used in a credit bootstrap.
pub mod creditcurveconfiguration;
/// Provides the serializable curve-bootstrap configuration.
pub mod curveconfiguration;
/// Orders curve dependencies and calibrates the linked discount and forwarding curves.
pub mod multicurvebootstrapper;

/// Associates a constructed curve with its input quotes and calibration map.
mod quotedcurve;

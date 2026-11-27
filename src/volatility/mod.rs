//! Volatility surface and cube definitions.
//!
//! Interpolated volatility surfaces, volatility cubes, and
//! quote-indexing types for equity and rates vol.

/// Provides the interpolated three-dimensional volatility cube.
pub mod interpolatedvolatilitycube;
/// Provides the interpolated two-dimensional volatility surface.
pub mod interpolatedvolatilitysurface;
/// Provides the model calibration configuration.
pub mod modelcalibration;
/// Reorients FX volatility requests between the canonical and requested currency pairs.
pub mod orientedfxvolsurface;
/// Defines the query interface for three-dimensional volatility data.
pub mod volatilitycube;
/// Constructs volatility cubes from market quotes and configuration.
pub mod volatilitycubebuilder;
/// Provides the serializable volatility-cube configuration.
pub mod volatilitycubeconfiguration;
/// Provides the quote coordinates used to address volatility data.
pub mod volatilityindexing;
/// Supplies the volatility data required by a valuation.
pub mod volatilitysource;
/// Defines the query interface for two-dimensional volatility data.
pub mod volatilitysurface;
/// Constructs volatility surfaces from market quotes and configuration.
pub mod volatilitysurfacebuilder;
/// Provides the serializable volatility-surface configuration.
pub mod volatilitysurfaceconfiguration;

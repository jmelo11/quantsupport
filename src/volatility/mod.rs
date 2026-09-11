//! Volatility surface and cube definitions.
//!
//! Interpolated volatility surfaces, volatility cubes, and
//! quote-indexing types for equity and rates vol.

/// Interpolated two-dimensional volatility surface.
pub mod interpolatedvolatilitysurface;
/// Interpolated three-dimensional volatility cube.
pub mod interpolatedvolatilitycube;
/// Volatility-cube interface.
pub mod volatilitycube;
/// Quote coordinates used to address volatility data.
pub mod volatilityindexing;
/// Volatility-surface interface.
pub mod volatilitysurface;
pub mod orientedfxvolsurface;
/// Serializable volatility-surface configuration.
pub mod volatilitysurfaceconfiguration;
/// Serializable volatility-cube configuration.
pub mod volatilitycubeconfiguration;
/// Volatility-surface construction from market data.
pub mod volatilitysurfacebuilder;
/// Volatility-cube construction from market data.
pub mod volatilitycubebuilder;
/// Model calibration configuration.
pub mod modelcalibration;
pub mod volatilitysource;

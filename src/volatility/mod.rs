//! Volatility surface and cube definitions.
//!
//! Interpolated volatility surfaces, volatility cubes, and
//! quote-indexing types for equity and rates vol.

/// Interpolated three-dimensional volatility cube.
pub mod interpolatedvolatilitycube;
/// Interpolated two-dimensional volatility surface.
pub mod interpolatedvolatilitysurface;
/// Model calibration configuration.
pub mod modelcalibration;
pub mod orientedfxvolsurface;
/// Volatility-cube interface.
pub mod volatilitycube;
/// Volatility-cube construction from market data.
pub mod volatilitycubebuilder;
/// Serializable volatility-cube configuration.
pub mod volatilitycubeconfiguration;
/// Quote coordinates used to address volatility data.
pub mod volatilityindexing;
pub mod volatilitysource;
/// Volatility-surface interface.
pub mod volatilitysurface;
/// Volatility-surface construction from market data.
pub mod volatilitysurfacebuilder;
/// Serializable volatility-surface configuration.
pub mod volatilitysurfaceconfiguration;

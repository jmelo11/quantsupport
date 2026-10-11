//! Interpolation module containing various interpolation methods and traits.

/// Policies for queries outside multidimensional interpolation grids.
pub mod extrapolation;
/// Enumeration types for interpolation.
pub mod interpolator;
/// Linear interpolation implementation.
pub mod linear;
/// Log-linear interpolation implementation.
pub mod loglinear;
mod multilinear;

/// Bilinear interpolation implementation.
pub mod bilinear;

/// Trilinear interpolation implementation.
pub mod trilinear;

/// Cubic spline interpolation implementation.
pub mod cubicspline;

//! Interpolation module containing various interpolation methods and traits.

/// Provides the policies for queries outside multidimensional interpolation grids.
pub mod extrapolation;
/// Provides the enumeration types for interpolation.
pub mod interpolator;
/// Provides the linear interpolation implementation.
pub mod linear;
/// Provides the log-linear interpolation implementation.
pub mod loglinear;
/// Interpolates values on a rectangular grid with a caller-selected number of dimensions.
mod multilinear;

/// Provides the bilinear interpolation implementation.
pub mod bilinear;

/// Provides the trilinear interpolation implementation.
pub mod trilinear;

/// Provides the cubic spline interpolation implementation.
pub mod cubicspline;

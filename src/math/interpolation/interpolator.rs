use serde::{Deserialize, Serialize};

use super::{linear::LinearInterpolator, loglinear::LogLinearInterpolator};
use crate::{ad::adreal::IsReal, utils::errors::Result};

/// # `StaticInterpolate` trait
///
/// A trait that defines the interpolation of a function. It does not require a reference to self.
pub trait StaticInterpolate<T>
where
    T: IsReal,
{
    /// Interpolates a value at the given point.
    ///
    /// ## Arguments
    /// * `x` - The point at which to interpolate
    /// * `x_` - The x-coordinates of the data points
    /// * `y_` - The y-coordinates of the data points
    /// * `enable_extrapolation` - Whether to allow extrapolation beyond the data range
    ///
    /// ## Returns
    /// * `Result<f64, AtlasError>` - The interpolated value or an error
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if interpolation fails.
    fn interpolate(x: T, x_: &[T], y_: &[T], enable_extrapolation: bool) -> Result<T>;
}

/// # `Interpolator`
/// Enum that represents the type of interpolation.
///
/// ### Example
/// ```
/// use quantsupport::math::interpolation::interpolator::Interpolator;
/// use quantsupport::math::interpolation::interpolator::Interpolate;
/// 
/// let x = 1.0;
/// let x_: Vec<f64> = vec![0.0, 1.0, 2.0];
/// let y_: Vec<f64> = vec![0.0, 1.0, 4.0];
/// let interpolator = Interpolator::Linear;
/// let y = interpolator.interpolate(x, &x_, &y_, true).unwrap();
/// assert_eq!(y, 1.0);
/// ```
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Interpolator {
    /// Linear interpolation method.
    Linear,
    /// Log-linear interpolation method.
    LogLinear,
}

/// # `Interpolator`
pub trait Interpolate<T>
where
    T: IsReal,
{
    /// Performs interpolation for a given x value using the specified interpolation method.
    ///
    /// ## Arguments
    /// * `x` - The point at which to interpolate
    /// * `x_` - The x-coordinates of the data points
    /// * `y_` - The y-coordinates of the data points
    /// * `enable_extrapolation` - Whether to allow extrapolation beyond the data range
    ///
    /// ## Returns
    /// * `Result<f64, AtlasError>` - The interpolated value or an error
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if interpolation fails.
    fn interpolate(&self, x: T, x_: &[T], y_: &[T], enable_extrapolation: bool) -> Result<T>;
}

impl<T> Interpolate<T> for Interpolator
where
    T: IsReal,
    LinearInterpolator: StaticInterpolate<T>,
    LogLinearInterpolator: StaticInterpolate<T>,
{
    fn interpolate(&self, x: T, x_: &[T], y_: &[T], enable_extrapolation: bool) -> Result<T> {
        match self {
            Self::Linear => LinearInterpolator::interpolate(x, x_, y_, enable_extrapolation),
            Self::LogLinear => LogLinearInterpolator::interpolate(x, x_, y_, enable_extrapolation),
        }
    }
}

use serde::{Deserialize, Serialize};

use super::{linear::LinearInterpolator, loglinear::LogLinearInterpolator, traits::Interpolate};
use crate::utils::errors::Result;

/// # `Interpolator`
/// Enum that represents the type of interpolation.
///
/// ### Example
/// ```
/// use rustatlas::prelude::*;
/// let x = 1.0;
/// let x_ = vec![0.0, 1.0, 2.0];
/// let y_ = vec![0.0, 1.0, 4.0];
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

impl Interpolator {
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
    pub fn interpolate(
        &self,
        x: f64,
        x_: &[f64],
        y_: &[f64],
        enable_extrapolation: bool,
    ) -> Result<f64> {
        match self {
            Self::Linear => LinearInterpolator::interpolate(x, x_, y_, enable_extrapolation),
            Self::LogLinear => LogLinearInterpolator::interpolate(x, x_, y_, enable_extrapolation),
        }
    }
}

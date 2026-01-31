use crate::prelude::AtlasError;

/// # `Interpolation` trait
/// A trait that defines the interpolation of a function.
pub trait Interpolate {
    /// Interpolates a value at the given point.
    ///
    /// # Arguments
    /// * `x` - The point at which to interpolate
    /// * `x_` - The x-coordinates of the data points
    /// * `y_` - The y-coordinates of the data points
    /// * `enable_extrapolation` - Whether to allow extrapolation beyond the data range
    ///
    /// # Returns
    /// Ok if the interpolation of `x` was succesfull.
    fn interpolate(
        x: f64,
        x_: &[f64],
        y_: &[f64],
        enable_extrapolation: bool,
    ) -> Result<f64, AtlasError>;
}

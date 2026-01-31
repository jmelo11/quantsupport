use std::cmp::Ordering;

use super::traits::Interpolate;
use crate::{prelude::AtlasError, utils::errors::Result};

/// # `Linear Interpolator`
/// Basic linear interpolator.
#[derive(Clone)]
pub struct LinearInterpolator {}

impl Interpolate for LinearInterpolator {
    fn interpolate(x: f64, x_: &[f64], y_: &[f64], enable_extrapolation: bool) -> Result<f64> {
        let index =
            match x_.binary_search_by(|&probe| probe.partial_cmp(&x).unwrap_or(Ordering::Equal)) {
                Ok(index) | Err(index) => index,
            };

        let (Some(first_x), Some(last_x)) = (x_.first(), x_.last()) else {
            return Err(AtlasError::InterpolationErr(
                "Interpolation data must contain at least one x value.".into(),
            ));
        };

        if !enable_extrapolation && (x < *first_x || x > *last_x) {
            return Err(AtlasError::InterpolationErr(
                "Extrapolation is not enabled, and the provided value is outside the range.".into(),
            ));
        }

        let y = match index {
            0 => y_[0] + (x - x_[0]) * (y_[1] - y_[0]) / (x_[1] - x_[0]),
            index if index == x_.len() => {
                y_[index - 1]
                    + (x - x_[index - 1]) * (y_[index - 1] - y_[index - 2])
                        / (x_[index - 1] - x_[index - 2])
            }
            _ => {
                y_[index - 1]
                    + (x - x_[index - 1]) * (y_[index] - y_[index - 1])
                        / (x_[index] - x_[index - 1])
            }
        };
        Ok(y)
    }
}
#[cfg(test)]
mod tests {
    use super::Interpolate;
    use super::LinearInterpolator;

    #[test]
    fn test_linear_interpolation() {
        let x = 0.5;
        let x_ = vec![0.0, 1.0];
        let y_ = vec![0.0, 1.0];
        let y = LinearInterpolator::interpolate(x, &x_, &y_, true).unwrap();
        assert!((y - 0.5).abs() < 1e-12);
    }
}

use std::cmp::Ordering;

use crate::{
    ad::adreal::{ADReal, FloatExt},
    math::interpolation::interpolator::StaticInterpolate,
    utils::errors::{AtlasError, Result},
};

/// # `LogLinearInterpolator`
/// Log-linear interpolator.
#[derive(Clone)]
pub struct LogLinearInterpolator {}

impl StaticInterpolate<f64> for LogLinearInterpolator {
    fn interpolate(x: f64, x_: &[f64], y_: &[f64], enable_extrapolation: bool) -> Result<f64> {
        let index =
            match x_.binary_search_by(|&probe| probe.partial_cmp(&x).unwrap_or(Ordering::Less)) {
                Ok(index) | Err(index) => index,
            };

        let (Some(first_x), Some(last_x)) = (x_.first(), x_.last()) else {
            return Err(AtlasError::InterpolationErr(
                "Interpolation data must contain at least one x value.".into(),
            ));
        };

        if !(enable_extrapolation || (x >= *first_x && x <= *last_x)) {
            return Err(AtlasError::InterpolationErr(
                "Extrapolation is not enabled, and the provided value is outside the range.".into(),
            ));
        }

        let y = match index {
            0 => y_[0] * (y_[1] / y_[0]).powf((x - x_[0]) / (x_[1] - x_[0])),
            idx if idx == x_.len() => {
                y_[idx - 1]
                    * (y_[idx - 1] / y_[idx - 2])
                        .powf((x - x_[idx - 1]) / (x_[idx - 1] - x_[idx - 2]))
            }
            _ => {
                y_[index - 1]
                    * (y_[index] / y_[index - 1])
                        .powf((x - x_[index - 1]) / (x_[index] - x_[index - 1]))
            }
        };
        Ok(y)
    }
}

impl StaticInterpolate<ADReal> for LogLinearInterpolator {
    fn interpolate(
        x: ADReal,
        x_: &[ADReal],
        y_: &[ADReal],
        enable_extrapolation: bool,
    ) -> Result<ADReal> {
        let index =
            match x_.binary_search_by(|&probe| probe.partial_cmp(&x).unwrap_or(Ordering::Less)) {
                Ok(index) | Err(index) => index,
            };

        let (Some(first_x), Some(last_x)) = (x_.first(), x_.last()) else {
            return Err(AtlasError::InterpolationErr(
                "Interpolation data must contain at least one x value.".into(),
            ));
        };

        if !(enable_extrapolation || ((&x >= first_x) && (&x <= last_x))) {
            return Err(AtlasError::InterpolationErr(
                "Extrapolation is not enabled, and the provided value is outside the range.".into(),
            ));
        }

        let y = match index {
            0 => y_[0] * (y_[1] / y_[0]).pow_expr((x - x_[0]) / (x_[1] - x_[0])),
            idx if idx == x_.len() => {
                y_[idx - 1]
                    * (y_[idx - 1] / y_[idx - 2])
                        .pow_expr((x - x_[idx - 1]) / (x_[idx - 1] - x_[idx - 2]))
            }
            _ => {
                y_[index - 1]
                    * (y_[index] / y_[index - 1])
                        .pow_expr((x - x_[index - 1]) / (x_[index] - x_[index - 1]))
            }
        };
        Ok(y.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loglinear_interpolation() {
        let x = 0.5;
        let x_ = vec![0.0, 1.0];
        let y_ = vec![0.1, 1.0]; // Change from 0.0 to 0.1
        let y = LogLinearInterpolator::interpolate(x, &x_, &y_, true).unwrap();
        // Adjust the expected value accordingly
        assert!((y - 0.31622776601683794).abs() < 1e-10);
    }
}

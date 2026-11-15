use std::cmp::Ordering;

use crate::{
    ad::Recorded,
    math::interpolation::interpolator::StaticInterpolate,
    utils::errors::{QSError, Result},
};

/// Basic linear interpolator.
///
/// Knots must be strictly increasing, with at least two points and matching
/// coordinate/value lengths. These preconditions are not validated; malformed
/// arrays can panic. Enabled extrapolation extends the nearest endpoint segment.
#[derive(Clone)]
pub struct LinearInterpolator {}

impl StaticInterpolate<f64> for LinearInterpolator {
    fn interpolate(x: f64, x_: &[f64], y_: &[f64], enable_extrapolation: bool) -> Result<f64> {
        let index =
            match x_.binary_search_by(|&probe| probe.partial_cmp(&x).unwrap_or(Ordering::Equal)) {
                Ok(index) | Err(index) => index,
            };

        let (Some(first_x), Some(last_x)) = (x_.first(), x_.last()) else {
            return Err(QSError::InterpolationErr(
                "Interpolation data must contain at least one x value.".into(),
            ));
        };

        if !enable_extrapolation && (x < *first_x || x > *last_x) {
            return Err(QSError::InterpolationErr(
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

impl StaticInterpolate<Recorded> for LinearInterpolator {
    fn interpolate(
        x: Recorded,
        x_: &[Recorded],
        y_: &[Recorded],
        enable_extrapolation: bool,
    ) -> Result<Recorded> {
        let index =
            match x_.binary_search_by(|&probe| probe.partial_cmp(&x).unwrap_or(Ordering::Equal)) {
                Ok(index) | Err(index) => index,
            };

        let (Some(first_x), Some(last_x)) = (x_.first(), x_.last()) else {
            return Err(QSError::InterpolationErr(
                "Interpolation data must contain at least one x value.".into(),
            ));
        };

        if !enable_extrapolation && (x < *first_x || x > *last_x) {
            return Err(QSError::InterpolationErr(
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
    use crate::ad::Recorded;
    use crate::ad::Recording;

    use super::LinearInterpolator;
    use super::StaticInterpolate;

    #[test]
    fn test_linear_interpolation() {
        let x = 0.5;
        let x_ = vec![0.0, 1.0];
        let y_ = vec![0.0, 1.0];
        let y = LinearInterpolator::interpolate(x, &x_, &y_, true).unwrap();
        assert!((y - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_linear_interpolation_adreal() {
        let x = Recorded::from(0.5);
        let x_ = vec![Recorded::from(0.0), Recorded::from(1.0)];
        let y_ = vec![Recorded::new(0.0), Recorded::new(1.0)];
        let y = LinearInterpolator::interpolate(x, &x_, &y_, true).unwrap();
        assert!((y.value() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_sens_to_pillars() {
        Recording::start();
        let x = Recorded::from(0.5);
        let x_ = vec![Recorded::from(0.0), Recorded::from(1.0)];
        let y_ = vec![Recorded::new(0.0), Recorded::new(1.0)];
        let y = LinearInterpolator::interpolate(x, &x_, &y_, true).unwrap();
        let _ = y.differentiate();
        assert!(y_.first().unwrap().derivative().unwrap() - 0.5 < 1e-12);
    }
}

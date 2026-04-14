use std::cmp::Ordering;

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    math::interpolation::interpolator::StaticInterpolate,
    utils::errors::{QSError, Result},
};

/// Natural cubic spline interpolator.
///
/// Uses natural boundary conditions (second derivatives are zero at the endpoints)
/// and solves the tridiagonal system via the Thomas algorithm on each call.
#[derive(Clone)]
pub struct CubicSplineInterpolator {}

// ═══════════════════════════════════════════════════════════════════════════
//  f64 implementation
// ═══════════════════════════════════════════════════════════════════════════

impl StaticInterpolate<f64> for CubicSplineInterpolator {
    #[allow(clippy::many_single_char_names)]
    fn interpolate(x: f64, x_: &[f64], y_: &[f64], enable_extrapolation: bool) -> Result<f64> {
        let n = x_.len();
        if n < 2 {
            return Err(QSError::InterpolationErr(
                "Cubic spline requires at least 2 data points.".into(),
            ));
        }
        if n != y_.len() {
            return Err(QSError::InterpolationErr(
                "x and y arrays must have the same length.".into(),
            ));
        }

        let (Some(&first_x), Some(&last_x)) = (x_.first(), x_.last()) else {
            return Err(QSError::InterpolationErr(
                "Interpolation data must contain at least one x value.".into(),
            ));
        };

        if !enable_extrapolation && (x < first_x || x > last_x) {
            return Err(QSError::InterpolationErr(
                "Extrapolation is not enabled, and the provided value is outside the range.".into(),
            ));
        }

        // If only 2 points, fall back to linear.
        if n == 2 {
            let slope = (y_[1] - y_[0]) / (x_[1] - x_[0]);
            return Ok(slope.mul_add(x - x_[0], y_[0]));
        }

        // Compute second derivatives (moments) M via the Thomas algorithm.
        let m = compute_moments_f64(x_, y_);

        // Locate interval.
        let idx =
            match x_.binary_search_by(|&probe| probe.partial_cmp(&x).unwrap_or(Ordering::Equal)) {
                Ok(i) => i.min(n - 2),
                Err(i) => {
                    if i == 0 {
                        0
                    } else {
                        (i - 1).min(n - 2)
                    }
                }
            };

        let h = x_[idx + 1] - x_[idx];
        let a = (x_[idx + 1] - x) / h;
        let b = (x - x_[idx]) / h;

        let y = a.mul_add(y_[idx], b * y_[idx + 1])
            + (a * a).mul_add(a, -a).mul_add(m[idx], (b * b).mul_add(b, -b) * m[idx + 1]) * (h * h) / 6.0;

        Ok(y)
    }
}

/// Solve the tridiagonal system for natural cubic spline second derivatives.
#[allow(clippy::many_single_char_names)]
fn compute_moments_f64(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len();
    let mut m = vec![0.0; n]; // M_0 = M_{n-1} = 0 (natural BC)

    if n <= 2 {
        return m;
    }

    let inner = n - 2;
    // Diagonals and RHS for the inner system (indices 1..n-2).
    let mut cp = vec![0.0; inner]; // modified upper diagonal
    let mut dp = vec![0.0; inner]; // modified RHS

    // Build the system.
    let h: Vec<f64> = (0..n - 1).map(|i| x[i + 1] - x[i]).collect();
    let mut d: Vec<f64> = Vec::with_capacity(inner);
    let mut rhs: Vec<f64> = Vec::with_capacity(inner);
    let mut upper: Vec<f64> = Vec::with_capacity(inner);
    let mut lower: Vec<f64> = Vec::with_capacity(inner);

    for i in 0..inner {
        let ii = i + 1; // index into x/y
        d.push(2.0 * (h[ii - 1] + h[ii]));
        rhs.push(6.0 * ((y[ii + 1] - y[ii]) / h[ii] - (y[ii] - y[ii - 1]) / h[ii - 1]));
        upper.push(h[ii]);
        lower.push(h[ii - 1]);
    }

    // Forward sweep
    cp[0] = upper[0] / d[0];
    dp[0] = rhs[0] / d[0];
    for i in 1..inner {
        let denom = lower[i].mul_add(-cp[i - 1], d[i]);
        cp[i] = upper[i] / denom;
        dp[i] = lower[i].mul_add(-dp[i - 1], rhs[i]) / denom;
    }

    // Back substitution
    m[inner] = dp[inner - 1]; // m[inner] maps to M_{inner} = M_{n-2}
    for i in (0..inner - 1).rev() {
        m[i + 1] = cp[i].mul_add(-m[i + 2], dp[i]);
    }

    m
}

// ═══════════════════════════════════════════════════════════════════════════
//  DualFwd implementation
// ═══════════════════════════════════════════════════════════════════════════

impl StaticInterpolate<DualFwd> for CubicSplineInterpolator {
    #[allow(clippy::many_single_char_names)]
    fn interpolate(
        x: DualFwd,
        x_: &[DualFwd],
        y_: &[DualFwd],
        enable_extrapolation: bool,
    ) -> Result<DualFwd> {
        let n = x_.len();
        if n < 2 {
            return Err(QSError::InterpolationErr(
                "Cubic spline requires at least 2 data points.".into(),
            ));
        }
        if n != y_.len() {
            return Err(QSError::InterpolationErr(
                "x and y arrays must have the same length.".into(),
            ));
        }

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

        // If only 2 points, fall back to linear.
        if n == 2 {
            let slope = y_[1].sub_val(y_[0]).div_val(x_[1].sub_val(x_[0]));
            return Ok(y_[0].add_val(slope.mul_val(x.sub_val(x_[0]))));
        }

        // Compute second derivatives (moments) M via the Thomas algorithm.
        let m = compute_moments_dual(x_, y_);

        // Locate interval using the real part.
        let idx =
            match x_.binary_search_by(|&probe| probe.partial_cmp(&x).unwrap_or(Ordering::Equal)) {
                Ok(i) => i.min(n - 2),
                Err(i) => {
                    if i == 0 {
                        0
                    } else {
                        (i - 1).min(n - 2)
                    }
                }
            };

        let h = x_[idx + 1].sub_val(x_[idx]);
        let a = x_[idx + 1].sub_val(x).div_val(h);
        let b = x.sub_val(x_[idx]).div_val(h);
        let six = DualFwd::from(6.0);
        let h2 = h.mul_val(h);

        // S(x) = a*y_i + b*y_{i+1} + ((a³-a)*M_i + (b³-b)*M_{i+1}) * h² / 6
        let a_term = a.mul_val(a).mul_val(a).sub_val(a).mul_val(m[idx]);
        let b_term = b.mul_val(b).mul_val(b).sub_val(b).mul_val(m[idx + 1]);
        let y = a
            .mul_val(y_[idx])
            .add_val(b.mul_val(y_[idx + 1]))
            .add_val(a_term.add_val(b_term).mul_val(h2).div_val(six));

        Ok(y)
    }
}

/// Solve the tridiagonal system for natural cubic spline second derivatives (`DualFwd`).
#[allow(clippy::many_single_char_names)]
fn compute_moments_dual(x: &[DualFwd], y: &[DualFwd]) -> Vec<DualFwd> {
    let n = x.len();
    let zero = DualFwd::from(0.0);
    let two = DualFwd::from(2.0);
    let six = DualFwd::from(6.0);
    let mut m = vec![zero; n];

    if n <= 2 {
        return m;
    }

    let inner = n - 2;
    let h: Vec<DualFwd> = (0..n - 1).map(|i| x[i + 1].sub_val(x[i])).collect();

    let mut d: Vec<DualFwd> = Vec::with_capacity(inner);
    let mut rhs: Vec<DualFwd> = Vec::with_capacity(inner);
    let mut upper: Vec<DualFwd> = Vec::with_capacity(inner);
    let mut lower: Vec<DualFwd> = Vec::with_capacity(inner);

    for i in 0..inner {
        let ii = i + 1;
        d.push(two.mul_val(h[ii - 1].add_val(h[ii])));
        rhs.push(
            six.mul_val(
                y[ii + 1]
                    .sub_val(y[ii])
                    .div_val(h[ii])
                    .sub_val(y[ii].sub_val(y[ii - 1]).div_val(h[ii - 1])),
            ),
        );
        upper.push(h[ii]);
        lower.push(h[ii - 1]);
    }

    let mut cp: Vec<DualFwd> = vec![zero; inner];
    let mut dp: Vec<DualFwd> = vec![zero; inner];

    cp[0] = upper[0].div_val(d[0]);
    dp[0] = rhs[0].div_val(d[0]);
    for i in 1..inner {
        let denom = d[i].sub_val(lower[i].mul_val(cp[i - 1]));
        cp[i] = upper[i].div_val(denom);
        dp[i] = rhs[i].sub_val(lower[i].mul_val(dp[i - 1])).div_val(denom);
    }

    m[inner] = dp[inner - 1];
    for i in (0..inner - 1).rev() {
        m[i + 1] = dp[i].sub_val(cp[i].mul_val(m[i + 2]));
    }

    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ad::tape::Tape;

    #[test]
    fn test_cubic_spline_exact_at_knots() {
        let x_ = vec![0.0, 1.0, 2.0, 3.0];
        let y_ = vec![0.0, 1.0, 4.0, 9.0];
        for (xi, yi) in x_.iter().zip(y_.iter()) {
            let y = CubicSplineInterpolator::interpolate(*xi, &x_, &y_, true).unwrap();
            assert!((y - yi).abs() < 1e-12, "at x={xi}: expected {yi}, got {y}");
        }
    }

    #[test]
    fn test_cubic_spline_smooth_interior() {
        // Quadratic function y = x^2: natural spline approximates well but
        // does not reproduce quadratics exactly due to the zero-second-derivative
        // boundary conditions.
        let x_ = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let y_: Vec<f64> = x_.iter().map(|&xi| xi * xi).collect();
        let test_x = 2.5;
        let y = CubicSplineInterpolator::interpolate(test_x, &x_, &y_, true).unwrap();
        assert!((y - 6.25).abs() < 0.1, "expected ~6.25, got {y}");
    }

    #[test]
    fn test_cubic_spline_extrapolation_disabled() {
        let x_ = vec![0.0, 1.0, 2.0];
        let y_ = vec![0.0, 1.0, 4.0];
        assert!(CubicSplineInterpolator::interpolate(-0.5, &x_, &y_, false).is_err());
        assert!(CubicSplineInterpolator::interpolate(2.5, &x_, &y_, false).is_err());
    }

    #[test]
    fn test_cubic_spline_dual_fwd() {
        let x_ = vec![
            DualFwd::from(0.0),
            DualFwd::from(1.0),
            DualFwd::from(2.0),
            DualFwd::from(3.0),
        ];
        let y_ = vec![
            DualFwd::from(0.0),
            DualFwd::from(1.0),
            DualFwd::from(4.0),
            DualFwd::from(9.0),
        ];
        let x = DualFwd::from(1.5);
        let y = CubicSplineInterpolator::interpolate(x, &x_, &y_, true).unwrap();
        // Should be close to 1.5^2 = 2.25 for quadratic data
        assert!(
            (y.value() - 2.25).abs() < 0.5,
            "expected ~2.25, got {}",
            y.value()
        );
    }

    #[test]
    fn test_cubic_spline_ad_sensitivities() {
        Tape::start_recording_fwd();
        let x_ = vec![DualFwd::from(0.0), DualFwd::from(1.0), DualFwd::from(2.0)];
        let y_ = vec![DualFwd::from(0.0), DualFwd::from(1.0), DualFwd::from(4.0)];
        let x = DualFwd::from(0.5);
        let y = CubicSplineInterpolator::interpolate(x, &x_, &y_, true).unwrap();
        // Just check that backward doesn't panic and produces finite adjoints.
        let _ = y.backward();
        for yi in &y_ {
            if let Ok(adj) = yi.adjoint() {
                assert!(adj.value().is_finite(), "adjoint is not finite");
            }
        }
    }
}

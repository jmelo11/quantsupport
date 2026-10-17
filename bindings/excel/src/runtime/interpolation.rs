//! Checked interpolation operations shared by worksheet adapters and native tests.

use quantsupport::math::interpolation::interpolator::{Interpolate, Interpolator};
use quantsupport::math::interpolation::{
    bilinear::{BilinearInterpolator, BilinearPoint},
    extrapolation::ExtrapolationPolicy,
    trilinear::{TrilinearInterpolator, TrilinearPoint},
};

/// Interpolates finite, ordered one-dimensional worksheet data.
///
/// # Errors
/// Rejects mismatched or short arrays, nonfinite inputs, unordered coordinates,
/// nonpositive log-linear values, forbidden extrapolation, and nonfinite results.
pub fn interpolate(
    method: Interpolator,
    point: f64,
    coordinates: &[f64],
    values: &[f64],
    enable_extrapolation: bool,
) -> Result<f64, String> {
    if coordinates.len() < 2 || coordinates.len() != values.len() {
        return Err(
            "coordinates and values must have the same length and at least two points".into(),
        );
    }
    if !point.is_finite()
        || !coordinates
            .iter()
            .chain(values)
            .all(|value| value.is_finite())
    {
        return Err("query, coordinates, and values must be finite numbers".into());
    }
    if coordinates.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("coordinates must be strictly increasing without duplicates".into());
    }
    if matches!(method, Interpolator::LogLinear) && values.iter().any(|value| *value <= 0.0) {
        return Err("log-linear interpolation requires strictly positive values".into());
    }
    let result = method
        .interpolate(point, coordinates, values, enable_extrapolation)
        .map_err(|error| error.to_string())?;
    if !result.is_finite() {
        return Err("interpolation produced a nonfinite result".into());
    }
    Ok(result)
}

/// Interpolates aligned x/y/value ranges with native sparse-slice semantics.
///
/// # Errors
/// Rejects empty or mismatched ranges, duplicate points, nonfinite inputs/results,
/// and queries outside a slice when extrapolation is rejected.
pub fn bilinear(
    point_x: f64,
    point_y: f64,
    x_coordinates: &[f64],
    y_coordinates: &[f64],
    values: &[f64],
    extrapolation: ExtrapolationPolicy,
) -> Result<f64, String> {
    if values.is_empty()
        || x_coordinates.len() != values.len()
        || y_coordinates.len() != values.len()
    {
        return Err("coordinate and value ranges must be nonempty and have equal lengths".into());
    }
    let points = x_coordinates
        .iter()
        .zip(y_coordinates)
        .zip(values)
        .map(|((&coordinate_x, &coordinate_y), &value)| BilinearPoint {
            x: coordinate_x,
            y: coordinate_y,
            value,
        })
        .collect();
    BilinearInterpolator::interpolate_with_extrapolation(point_x, point_y, points, extrapolation)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            "invalid interpolation grid, query outside a slice, or nonfinite result".into()
        })
}

/// Interpolates aligned x/y/z/value ranges with native sparse-slice semantics.
///
/// # Errors
/// Rejects empty or mismatched ranges, duplicate points, nonfinite inputs/results,
/// and queries outside a slice when extrapolation is rejected.
pub fn trilinear(
    point_x: f64,
    point_y: f64,
    point_z: f64,
    x_coordinates: &[f64],
    y_coordinates: &[f64],
    z_coordinates: &[f64],
    values: &[f64],
    extrapolation: ExtrapolationPolicy,
) -> Result<f64, String> {
    if values.is_empty()
        || x_coordinates.len() != values.len()
        || y_coordinates.len() != values.len()
        || z_coordinates.len() != values.len()
    {
        return Err("coordinate and value ranges must be nonempty and have equal lengths".into());
    }
    let points = x_coordinates
        .iter()
        .zip(y_coordinates)
        .zip(z_coordinates)
        .zip(values)
        .map(
            |(((&coordinate_x, &coordinate_y), &coordinate_z), &value)| TrilinearPoint {
                x: coordinate_x,
                y: coordinate_y,
                z: coordinate_z,
                value,
            },
        )
        .collect();
    TrilinearInterpolator::interpolate_with_extrapolation(
        point_x,
        point_y,
        point_z,
        points,
        extrapolation,
    )
    .filter(|value| value.is_finite())
    .ok_or_else(|| "invalid interpolation grid, query outside a slice, or nonfinite result".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multilinear_methods_reproduce_nodes_and_cross_terms() {
        let x_coordinates = [0.0, 1.0, 0.0, 1.0];
        let y_coordinates = [0.0, 0.0, 1.0, 1.0];
        let values = [1.0, 3.0, 4.0, 10.0];
        let actual = bilinear(
            0.5,
            0.5,
            &x_coordinates,
            &y_coordinates,
            &values,
            ExtrapolationPolicy::Reject,
        )
        .unwrap();
        assert!((actual - 4.5).abs() < 1e-12);
        for index in 0..values.len() {
            assert_eq!(
                bilinear(
                    x_coordinates[index],
                    y_coordinates[index],
                    &x_coordinates,
                    &y_coordinates,
                    &values,
                    ExtrapolationPolicy::Reject
                )
                .unwrap(),
                values[index]
            );
        }
        let mut cube_x = Vec::new();
        let mut cube_y = Vec::new();
        let mut cube_z = Vec::new();
        let mut cube_values = Vec::new();
        for coordinate_x in [0.0, 1.0] {
            for coordinate_y in [0.0, 1.0] {
                for coordinate_z in [0.0, 1.0] {
                    cube_x.push(coordinate_x);
                    cube_y.push(coordinate_y);
                    cube_z.push(coordinate_z);
                    cube_values.push(
                        1.0 + 2.0 * coordinate_x
                            + 3.0 * coordinate_y
                            + 4.0 * coordinate_z
                            + 8.0 * coordinate_x * coordinate_y * coordinate_z,
                    );
                }
            }
        }
        let actual = trilinear(
            0.5,
            0.5,
            0.5,
            &cube_x,
            &cube_y,
            &cube_z,
            &cube_values,
            ExtrapolationPolicy::Reject,
        )
        .unwrap();
        assert!((actual - 6.5).abs() < 1e-12);
        for index in 0..cube_values.len() {
            assert_eq!(
                trilinear(
                    cube_x[index],
                    cube_y[index],
                    cube_z[index],
                    &cube_x,
                    &cube_y,
                    &cube_z,
                    &cube_values,
                    ExtrapolationPolicy::Reject
                )
                .unwrap(),
                cube_values[index]
            );
        }
    }

    #[test]
    fn multilinear_methods_preserve_sparse_slices_and_flat_extrapolation() {
        let x_coordinates = [0.0, 0.0, 1.0, 1.0];
        let y_coordinates = [0.0, 2.0, 1.0, 3.0];
        let values = [1.0, 7.0, 10.0, 24.0];
        let actual = bilinear(
            0.5,
            1.5,
            &x_coordinates,
            &y_coordinates,
            &values,
            ExtrapolationPolicy::Reject,
        )
        .unwrap();
        assert!((actual - 9.5).abs() < 1e-12);
        let actual = bilinear(
            0.5,
            4.0,
            &x_coordinates,
            &y_coordinates,
            &values,
            ExtrapolationPolicy::Flat,
        )
        .unwrap();
        assert!((actual - 15.5).abs() < 1e-12);
        assert!(bilinear(
            0.5,
            0.0,
            &x_coordinates,
            &y_coordinates,
            &values,
            ExtrapolationPolicy::Reject
        )
        .is_err());
        assert!(trilinear(
            5.0,
            5.0,
            5.0,
            &[0.0],
            &[0.0],
            &[0.0],
            &[3.0],
            ExtrapolationPolicy::Reject
        )
        .is_err());
        assert_eq!(
            trilinear(
                5.0,
                5.0,
                5.0,
                &[0.0],
                &[0.0],
                &[0.0],
                &[3.0],
                ExtrapolationPolicy::Flat
            )
            .unwrap(),
            3.0
        );
    }

    #[test]
    fn multilinear_methods_reject_invalid_ranges_grids_and_queries() {
        for policy in [ExtrapolationPolicy::Reject, ExtrapolationPolicy::Flat] {
            for (coordinates, values) in [
                (&[][..], &[][..]),
                (&[0.0, 1.0][..], &[1.0][..]),
                (&[0.0, 0.0][..], &[1.0, 2.0][..]),
                (&[f64::NAN][..], &[1.0][..]),
                (&[0.0][..], &[f64::INFINITY][..]),
            ] {
                assert!(bilinear(0.0, 0.0, coordinates, coordinates, values, policy).is_err());
                assert!(trilinear(
                    0.0,
                    0.0,
                    0.0,
                    coordinates,
                    coordinates,
                    coordinates,
                    values,
                    policy
                )
                .is_err());
            }
            assert!(bilinear(f64::NAN, 0.0, &[0.0], &[0.0], &[1.0], policy).is_err());
            assert!(trilinear(
                0.0,
                0.0,
                f64::INFINITY,
                &[0.0],
                &[0.0],
                &[0.0],
                &[1.0],
                policy
            )
            .is_err());
            assert!(bilinear(0.0, 0.0, &[0.0], &[], &[1.0], policy).is_err());
            assert!(trilinear(0.0, 0.0, 0.0, &[0.0], &[0.0], &[], &[1.0], policy).is_err());
        }
    }

    #[test]
    fn one_dimensional_methods_preserve_native_results_and_nodes() {
        for (method, values, expected) in [
            (Interpolator::Linear, [1.0, 4.0, 1.0], 2.5),
            (Interpolator::LogLinear, [1.0, 4.0, 1.0], 2.0),
            (Interpolator::CubicSpline, [0.0, 1.0, 0.0], 0.6875),
        ] {
            let coordinates = [0.0, 1.0, 2.0];
            let actual = interpolate(method, 0.5, &coordinates, &values, false).unwrap();
            assert!((actual - expected).abs() < 1e-12);
            for (coordinate, value) in coordinates.into_iter().zip(values) {
                let actual = interpolate(method, coordinate, &coordinates, &values, false).unwrap();
                assert!((actual - value).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn one_dimensional_methods_reject_invalid_worksheet_data() {
        for method in [
            Interpolator::Linear,
            Interpolator::LogLinear,
            Interpolator::CubicSpline,
        ] {
            for (coordinates, values) in [
                (&[][..], &[][..]),
                (&[0.0][..], &[1.0][..]),
                (&[0.0, 1.0][..], &[1.0][..]),
                (&[0.0, 0.0][..], &[1.0, 2.0][..]),
                (&[1.0, 0.0][..], &[1.0, 2.0][..]),
                (&[0.0, f64::NAN][..], &[1.0, 2.0][..]),
                (&[0.0, 1.0][..], &[1.0, f64::INFINITY][..]),
            ] {
                assert!(interpolate(method, 0.5, coordinates, values, true).is_err());
            }
            for point in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(interpolate(method, point, &[0.0, 1.0], &[1.0, 2.0], true).is_err());
            }
        }
        for values in [[0.0, 1.0], [-1.0, 1.0], [-2.0, -1.0]] {
            assert!(interpolate(Interpolator::LogLinear, 0.5, &[0.0, 1.0], &values, true).is_err());
        }
    }

    #[test]
    fn one_dimensional_extrapolation_is_explicit() {
        for method in [
            Interpolator::Linear,
            Interpolator::LogLinear,
            Interpolator::CubicSpline,
        ] {
            for point in [-1.0, 2.0] {
                assert!(interpolate(method, point, &[0.0, 1.0], &[1.0, 4.0], false).is_err());
                let actual = interpolate(method, point, &[0.0, 1.0], &[1.0, 4.0], true).unwrap();
                let expected = if matches!(method, Interpolator::LogLinear) {
                    4.0_f64.powf(point)
                } else {
                    1.0 + 3.0 * point
                };
                assert!((actual - expected).abs() < 1e-12);
            }
        }
        assert!(interpolate(
            Interpolator::Linear,
            0.5,
            &[0.0, 1.0],
            &[f64::MAX, -f64::MAX],
            false
        )
        .is_err());
    }
}

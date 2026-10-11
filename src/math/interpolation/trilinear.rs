use crate::ad::scalar::Scalar;

use super::{bilinear::BilinearValue, extrapolation::ExtrapolationPolicy};

/// Input point for trilinear interpolation.
#[derive(Clone)]
pub struct TrilinearPoint<T: Scalar> {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
    /// Z coordinate.
    pub z: f64,
    /// Point value.
    pub value: T,
}

/// Trilinear interpolation over rectangular or sparse x/y/z slices.
pub struct TrilinearInterpolator;

impl TrilinearInterpolator {
    /// Interpolates z within each (x, y) slice, then along y and x without extrapolation.
    ///
    /// Exact boundaries and singleton axes are supported. Slices may contain
    /// different inner coordinates. Returns `None` for out-of-range queries,
    /// empty grids, duplicate nodes, or non-finite coordinates or values.
    #[must_use]
    pub fn interpolate<T: BilinearValue>(
        x: f64,
        y: f64,
        z: f64,
        points: Vec<TrilinearPoint<T>>,
    ) -> Option<T> {
        Self::interpolate_with_extrapolation(x, y, z, points, ExtrapolationPolicy::Reject)
    }

    /// Interpolates with an explicit policy for queries outside each slice.
    ///
    /// `Flat` clamps each axis to its nearest available boundary, treating a
    /// singleton axis as constant. Invalid grids and non-finite queries
    /// return `None` under either policy.
    #[must_use]
    pub fn interpolate_with_extrapolation<T: BilinearValue>(
        x: f64,
        y: f64,
        z: f64,
        points: Vec<TrilinearPoint<T>>,
        extrapolation: ExtrapolationPolicy,
    ) -> Option<T> {
        super::multilinear::interpolate(
            [x, y, z],
            points
                .into_iter()
                .map(|point| ([point.x, point.y, point.z], point.value))
                .collect(),
            extrapolation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ExtrapolationPolicy;
    use super::{TrilinearInterpolator, TrilinearPoint};
    use crate::ad::dual::DualFwd;
    use crate::ad::tape::Tape;

    fn cube_points_f64() -> Vec<TrilinearPoint<f64>> {
        // 2×2×2 cube: x in {0,1}, y in {0,1}, z in {0,1}
        // values: v = 1 + 2x + 3y + 4z  (linear → trilinear gives exact result)
        vec![
            TrilinearPoint {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                value: 1.0,
            },
            TrilinearPoint {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                value: 3.0,
            },
            TrilinearPoint {
                x: 0.0,
                y: 1.0,
                z: 0.0,
                value: 4.0,
            },
            TrilinearPoint {
                x: 1.0,
                y: 1.0,
                z: 0.0,
                value: 6.0,
            },
            TrilinearPoint {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                value: 5.0,
            },
            TrilinearPoint {
                x: 1.0,
                y: 0.0,
                z: 1.0,
                value: 7.0,
            },
            TrilinearPoint {
                x: 0.0,
                y: 1.0,
                z: 1.0,
                value: 8.0,
            },
            TrilinearPoint {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                value: 10.0,
            },
        ]
    }

    #[test]
    fn trilinear_center_f64() {
        let expected = 1.0 + 2.0 * 0.5 + 3.0 * 0.5 + 4.0 * 0.5; // 5.5
        let val = TrilinearInterpolator::interpolate(0.5, 0.5, 0.5, cube_points_f64())
            .expect("center interpolation");
        assert!(
            (val - expected).abs() < 1e-12,
            "got {val}, expected {expected}"
        );
    }

    #[test]
    fn trilinear_center_dualfwd() {
        let points: Vec<TrilinearPoint<DualFwd>> = cube_points_f64()
            .into_iter()
            .map(|p| TrilinearPoint {
                x: p.x,
                y: p.y,
                z: p.z,
                value: DualFwd::from(p.value),
            })
            .collect();
        let val = TrilinearInterpolator::interpolate(0.5, 0.5, 0.5, points)
            .expect("center interpolation DualFwd");
        assert!((val.value() - 5.5).abs() < 1e-12);
    }

    #[test]
    fn trilinear_out_of_grid_returns_none() {
        assert!(TrilinearInterpolator::interpolate(-0.1, 0.5, 0.5, cube_points_f64()).is_none());
        assert!(TrilinearInterpolator::interpolate(0.5, 0.5, 1.5, cube_points_f64()).is_none());
    }

    #[test]
    fn trilinear_reproduces_all_nodes_faces_and_cross_terms() {
        let value = |x: f64, y: f64, z: f64| {
            1.0 + 2.0 * x
                + 3.0 * y
                + 4.0 * z
                + 5.0 * x * y
                + 6.0 * x * z
                + 7.0 * y * z
                + 8.0 * x * y * z
        };
        let mut points = cube_points_f64();
        for point in &mut points {
            point.value = value(point.x, point.y, point.z);
        }
        for x in [0.0, 0.25, 1.0] {
            for y in [0.0, 0.5, 1.0] {
                for z in [0.0, 0.75, 1.0] {
                    let actual = TrilinearInterpolator::interpolate(x, y, z, points.clone())
                        .expect("closed cube bounds");
                    assert!((actual - value(x, y, z)).abs() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn trilinear_supports_every_combination_of_singleton_axes() {
        for singleton_axes in 0..8 {
            let points: Vec<_> = cube_points_f64()
                .into_iter()
                .filter(|point| {
                    (singleton_axes & 1 == 0 || point.x == 0.0)
                        && (singleton_axes & 2 == 0 || point.y == 0.0)
                        && (singleton_axes & 4 == 0 || point.z == 0.0)
                })
                .collect();
            let x = if singleton_axes & 1 == 0 { 0.5 } else { 0.0 };
            let y = if singleton_axes & 2 == 0 { 0.5 } else { 0.0 };
            let z = if singleton_axes & 4 == 0 { 0.5 } else { 0.0 };
            let actual =
                TrilinearInterpolator::interpolate(x, y, z, points).expect("degenerate cube");
            assert!((actual - (1.0 + 2.0 * x + 3.0 * y + 4.0 * z)).abs() < 1e-12);
        }
    }

    #[test]
    fn trilinear_supports_sparse_tenors_and_smiles() {
        let points = vec![
            TrilinearPoint {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                value: 1.0,
            },
            TrilinearPoint {
                x: 0.0,
                y: 0.0,
                z: 2.0,
                value: 9.0,
            },
            TrilinearPoint {
                x: 0.0,
                y: 2.0,
                z: 1.0,
                value: 11.0,
            },
            TrilinearPoint {
                x: 0.0,
                y: 2.0,
                z: 3.0,
                value: 19.0,
            },
            TrilinearPoint {
                x: 1.0,
                y: 1.0,
                z: 0.0,
                value: 6.0,
            },
            TrilinearPoint {
                x: 1.0,
                y: 1.0,
                z: 2.0,
                value: 14.0,
            },
            TrilinearPoint {
                x: 1.0,
                y: 3.0,
                z: 1.0,
                value: 16.0,
            },
            TrilinearPoint {
                x: 1.0,
                y: 3.0,
                z: 3.0,
                value: 24.0,
            },
        ];
        let actual =
            TrilinearInterpolator::interpolate(0.5, 1.5, 1.5, points.clone()).expect("ragged cube");
        assert!((actual - 12.5).abs() < 1e-12);
        for point in &points {
            assert_eq!(
                TrilinearInterpolator::interpolate(point.x, point.y, point.z, points.clone()),
                Some(point.value)
            );
        }
    }

    #[test]
    fn trilinear_flat_extrapolation_preserves_boundary_sensitivities() {
        Tape::start_recording_fwd();
        let points: Vec<_> = cube_points_f64()
            .into_iter()
            .map(|point| TrilinearPoint {
                x: point.x,
                y: point.y,
                z: point.z,
                value: DualFwd::from(point.value),
            })
            .collect();
        let value = TrilinearInterpolator::interpolate_with_extrapolation(
            2.0,
            0.5,
            -1.0,
            points.clone(),
            ExtrapolationPolicy::Flat,
        )
        .expect("flat boundary interpolation");
        assert!((value.value() - 4.5).abs() < 1e-12);
        value.backward().expect("backward sweep");
        for point in points {
            let expected = if point.x == 1.0 && point.z == 0.0 {
                0.5
            } else {
                0.0
            };
            assert!(
                (point.value.adjoint().expect("pillar adjoint").value() - expected).abs() < 1e-12
            );
        }
        Tape::stop_recording_fwd();
    }

    #[test]
    fn trilinear_rejects_invalid_grids_and_queries() {
        for policy in [ExtrapolationPolicy::Reject, ExtrapolationPolicy::Flat] {
            assert!(TrilinearInterpolator::interpolate_with_extrapolation(
                0.5,
                0.5,
                f64::NAN,
                cube_points_f64(),
                policy
            )
            .is_none());
            for points in [
                vec![],
                vec![
                    TrilinearPoint {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                        value: 1.0
                    };
                    2
                ],
                vec![TrilinearPoint {
                    x: 0.0,
                    y: f64::INFINITY,
                    z: 0.0,
                    value: 1.0,
                }],
                vec![TrilinearPoint {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    value: f64::INFINITY,
                }],
            ] {
                assert!(TrilinearInterpolator::interpolate_with_extrapolation(
                    0.0, 0.0, 0.0, points, policy
                )
                .is_none());
            }
        }
    }
}

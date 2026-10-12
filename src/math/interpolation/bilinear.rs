use crate::ad::{dual::DualFwd, scalar::Scalar};

use super::extrapolation::ExtrapolationPolicy;

/// Numeric behavior required for bilinear interpolation.
pub trait BilinearValue: Scalar {
    /// Linearly interpolates between `a` and `b` using scalar weight `t`.
    fn lerp(a: Self, b: Self, t: f64) -> Self;
}

impl BilinearValue for f64 {
    fn lerp(a: Self, b: Self, t: f64) -> Self {
        (b - a).mul_add(t, a)
    }
}

impl BilinearValue for DualFwd {
    fn lerp(a: Self, b: Self, t: f64) -> Self {
        (a + (b - a) * t).into()
    }
}

/// Input point for bilinear interpolation.
#[derive(Clone)]
pub struct BilinearPoint<T: Scalar> {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
    /// Point value.
    pub value: T,
}

/// Bilinear interpolation over rectangular or sparse x/y slices.
pub struct BilinearInterpolator;

impl BilinearInterpolator {
    /// Interpolates y within each x slice, then along x without extrapolation.
    ///
    /// Exact boundaries and singleton axes are supported. Slices may contain
    /// different inner coordinates. Returns `None` for out-of-range queries,
    /// empty grids, duplicate nodes, or non-finite coordinates or values.
    #[must_use]
    pub fn interpolate<T: BilinearValue>(
        x: f64,
        y: f64,
        points: Vec<BilinearPoint<T>>,
    ) -> Option<T> {
        Self::interpolate_with_extrapolation(x, y, points, ExtrapolationPolicy::Reject)
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
        points: Vec<BilinearPoint<T>>,
        extrapolation: ExtrapolationPolicy,
    ) -> Option<T> {
        super::multilinear::interpolate(
            [x, y],
            points
                .into_iter()
                .map(|point| ([point.x, point.y], point.value))
                .collect(),
            extrapolation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ExtrapolationPolicy;
    use crate::{
        ad::dual::DualFwd,
        math::interpolation::bilinear::{BilinearInterpolator, BilinearPoint},
    };

    fn point(x: f64, y: f64) -> BilinearPoint<f64> {
        BilinearPoint {
            x,
            y,
            value: 1.0 + 2.0 * x + 3.0 * y + 4.0 * x * y,
        }
    }

    #[test]
    fn bilinear_reproduces_nodes_edges_and_cross_terms() {
        let points = vec![
            point(0.0, 0.0),
            point(0.0, 1.0),
            point(1.0, 0.0),
            point(1.0, 1.0),
        ];
        for x in [0.0, 0.25, 1.0] {
            for y in [0.0, 0.75, 1.0] {
                let actual =
                    BilinearInterpolator::interpolate(x, y, points.clone()).expect("in-grid query");
                assert!((actual - point(x, y).value).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn bilinear_supports_singleton_axes_and_flat_extrapolation() {
        for (x, y, points, expected) in [
            (0.0, 0.5, vec![point(0.0, 0.0), point(0.0, 1.0)], 2.5),
            (0.5, 0.0, vec![point(0.0, 0.0), point(1.0, 0.0)], 2.0),
            (0.0, 0.0, vec![point(0.0, 0.0)], 1.0),
        ] {
            let actual = BilinearInterpolator::interpolate(x, y, points).expect("singleton axes");
            assert!((actual - expected).abs() < 1e-12);
        }
        let points = vec![point(0.0, 0.0), point(0.0, 1.0)];
        assert!(BilinearInterpolator::interpolate(5.0, 0.5, points.clone()).is_none());
        let actual = BilinearInterpolator::interpolate_with_extrapolation(
            5.0,
            0.5,
            points,
            ExtrapolationPolicy::Flat,
        )
        .expect("flat singleton axis");
        assert!((actual - 2.5).abs() < 1e-12);
    }

    #[test]
    fn bilinear_interpolates_sparse_smiles_and_exact_nodes() {
        let points = vec![
            point(0.0, 0.0),
            point(0.0, 2.0),
            point(1.0, 1.0),
            point(1.0, 3.0),
        ];
        let actual = BilinearInterpolator::interpolate(0.5, 1.5, points.clone())
            .expect("overlapping sparse smiles");
        assert!((actual - point(0.5, 1.5).value).abs() < 1e-12);
        for node in &points {
            assert_eq!(
                BilinearInterpolator::interpolate(node.x, node.y, points.clone()),
                Some(node.value)
            );
        }
        assert!(BilinearInterpolator::interpolate(0.5, 0.0, points.clone()).is_none());
        let actual = BilinearInterpolator::interpolate_with_extrapolation(
            0.5,
            4.0,
            points,
            ExtrapolationPolicy::Flat,
        )
        .expect("flat extrapolation within each smile");
        assert!((actual - 15.5).abs() < 1e-12);
    }

    #[test]
    fn bilinear_rejects_invalid_grids_and_queries() {
        for policy in [ExtrapolationPolicy::Reject, ExtrapolationPolicy::Flat] {
            for query in [(f64::NAN, 0.0), (0.0, f64::INFINITY)] {
                assert!(BilinearInterpolator::interpolate_with_extrapolation(
                    query.0,
                    query.1,
                    vec![point(0.0, 0.0)],
                    policy
                )
                .is_none());
            }
            for points in [
                vec![],
                vec![point(0.0, 0.0), point(-0.0, 0.0)],
                vec![point(f64::NAN, 0.0)],
                vec![BilinearPoint {
                    x: 0.0,
                    y: 0.0,
                    value: f64::NAN,
                }],
            ] {
                assert!(BilinearInterpolator::interpolate_with_extrapolation(
                    0.0, 0.0, points, policy
                )
                .is_none());
            }
        }
    }

    #[test]
    fn bilinear_interpolates_center() {
        let points = vec![
            BilinearPoint {
                x: 0.0,
                y: 0.0,
                value: DualFwd::from(1.0),
            },
            BilinearPoint {
                x: 1.0,
                y: 0.0,
                value: DualFwd::from(3.0),
            },
            BilinearPoint {
                x: 0.0,
                y: 1.0,
                value: DualFwd::from(2.0),
            },
            BilinearPoint {
                x: 1.0,
                y: 1.0,
                value: DualFwd::from(4.0),
            },
        ];
        let out =
            BilinearInterpolator::interpolate(0.5, 0.5, points).expect("center interpolation");
        assert!((out.value() - 2.5).abs() < 1e-12);
    }
}

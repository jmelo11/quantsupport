use crate::ad::scalar::Scalar;

use super::bilinear::BilinearValue;

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

/// Trilinear interpolation over a structured 3-D grid.
pub struct TrilinearInterpolator;

impl TrilinearInterpolator {
    /// Interpolate at (`x`, `y`, `z`) from rectangular-grid points.
    ///
    /// Returns `None` when the query falls outside the grid or the grid
    /// has fewer than two unique values along any axis.
    #[must_use]
    #[allow(clippy::similar_names)]
    pub fn interpolate<T: BilinearValue>(
        x: f64,
        y: f64,
        z: f64,
        mut points: Vec<TrilinearPoint<T>>,
    ) -> Option<T> {
        if points.is_empty() {
            return None;
        }

        points.sort_by(|a, b| {
            a.x.total_cmp(&b.x)
                .then_with(|| a.y.total_cmp(&b.y))
                .then_with(|| a.z.total_cmp(&b.z))
        });

        let mut xs = points.iter().map(|p| p.x).collect::<Vec<_>>();
        xs.sort_by(f64::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);

        let mut ys = points.iter().map(|p| p.y).collect::<Vec<_>>();
        ys.sort_by(f64::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);

        let mut zs = points.iter().map(|p| p.z).collect::<Vec<_>>();
        zs.sort_by(f64::total_cmp);
        zs.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);

        if xs.len() < 2 || ys.len() < 2 || zs.len() < 2 {
            return None;
        }

        let ix = xs.partition_point(|v| *v <= x);
        let iy = ys.partition_point(|v| *v <= y);
        let iz = zs.partition_point(|v| *v <= z);

        if ix == 0 || ix >= xs.len() || iy == 0 || iy >= ys.len() || iz == 0 || iz >= zs.len() {
            return None;
        }

        let (x0, x1) = (xs[ix - 1], xs[ix]);
        let (y0, y1) = (ys[iy - 1], ys[iy]);
        let (z0, z1) = (zs[iz - 1], zs[iz]);

        let lookup = |lx: f64, ly: f64, lz: f64| {
            points.iter().find(|p| {
                (p.x - lx).abs() < f64::EPSILON
                    && (p.y - ly).abs() < f64::EPSILON
                    && (p.z - lz).abs() < f64::EPSILON
            })
        };

        let c000 = lookup(x0, y0, z0)?.value;
        let c100 = lookup(x1, y0, z0)?.value;
        let c010 = lookup(x0, y1, z0)?.value;
        let c110 = lookup(x1, y1, z0)?.value;
        let c001 = lookup(x0, y0, z1)?.value;
        let c101 = lookup(x1, y0, z1)?.value;
        let c011 = lookup(x0, y1, z1)?.value;
        let c111 = lookup(x1, y1, z1)?.value;

        let tx = if (x1 - x0).abs() < f64::EPSILON {
            0.0
        } else {
            (x - x0) / (x1 - x0)
        };
        let ty = if (y1 - y0).abs() < f64::EPSILON {
            0.0
        } else {
            (y - y0) / (y1 - y0)
        };
        let tz = if (z1 - z0).abs() < f64::EPSILON {
            0.0
        } else {
            (z - z0) / (z1 - z0)
        };

        // Interpolate along x for each (y,z) corner pair
        let c00 = T::lerp(c000, c100, tx);
        let c01 = T::lerp(c001, c101, tx);
        let c10 = T::lerp(c010, c110, tx);
        let c11 = T::lerp(c011, c111, tx);

        // Interpolate along y
        let c0 = T::lerp(c00, c10, ty);
        let c1 = T::lerp(c01, c11, ty);

        // Interpolate along z
        Some(T::lerp(c0, c1, tz))
    }
}

#[cfg(test)]
mod tests {
    use super::{TrilinearInterpolator, TrilinearPoint};
    use crate::ad::dual::DualFwd;

    fn cube_points_f64() -> Vec<TrilinearPoint<f64>> {
        // 2×2×2 cube: x in {0,1}, y in {0,1}, z in {0,1}
        // values: v = 1 + 2x + 3y + 4z  (linear → trilinear gives exact result)
        vec![
            TrilinearPoint { x: 0.0, y: 0.0, z: 0.0, value: 1.0 },
            TrilinearPoint { x: 1.0, y: 0.0, z: 0.0, value: 3.0 },
            TrilinearPoint { x: 0.0, y: 1.0, z: 0.0, value: 4.0 },
            TrilinearPoint { x: 1.0, y: 1.0, z: 0.0, value: 6.0 },
            TrilinearPoint { x: 0.0, y: 0.0, z: 1.0, value: 5.0 },
            TrilinearPoint { x: 1.0, y: 0.0, z: 1.0, value: 7.0 },
            TrilinearPoint { x: 0.0, y: 1.0, z: 1.0, value: 8.0 },
            TrilinearPoint { x: 1.0, y: 1.0, z: 1.0, value: 10.0 },
        ]
    }

    #[test]
    fn trilinear_center_f64() {
        let expected = 1.0 + 2.0 * 0.5 + 3.0 * 0.5 + 4.0 * 0.5; // 5.5
        let val = TrilinearInterpolator::interpolate(0.5, 0.5, 0.5, cube_points_f64())
            .expect("center interpolation");
        assert!((val - expected).abs() < 1e-12, "got {val}, expected {expected}");
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
}

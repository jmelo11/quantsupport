use crate::ad::adreal::ADReal;

/// # `BilinearPoint`
/// Input point for bilinear interpolation.
#[derive(Clone)]
pub struct BilinearPoint<K> {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
    /// Point value.
    pub value: ADReal,
    /// User key attached to this point.
    pub key: K,
}

/// # `BilinearInterpolationResult`
/// Result of an interpolation call.
#[derive(Clone)]
pub struct BilinearInterpolationResult<K> {
    value: ADReal,
    interpolation_keys: Vec<K>,
    colliding_keys: Vec<K>,
}

impl<K> BilinearInterpolationResult<K> {
    #[must_use]
    /// Creates a new interpolation result.
    pub fn new(value: ADReal, interpolation_keys: Vec<K>, colliding_keys: Vec<K>) -> Self {
        Self {
            value,
            interpolation_keys,
            colliding_keys,
        }
    }

    #[must_use]
    /// Returns interpolated value.
    pub const fn value(&self) -> ADReal {
        self.value
    }

    #[must_use]
    /// Returns point keys used by interpolation.
    pub fn interpolation_keys(&self) -> &[K] {
        &self.interpolation_keys
    }

    #[must_use]
    /// Returns keys that collide on same coordinates.
    pub fn colliding_keys(&self) -> &[K] {
        &self.colliding_keys
    }
}

/// # `BilinearInterpolator`
/// Bilinear interpolation implementation independent from volatility containers.
pub struct BilinearInterpolator;

impl BilinearInterpolator {
    /// Interpolate at (`x`,`y`) from rectangular points.
    #[must_use]
    pub fn interpolate<K: Clone>(
        x: f64,
        y: f64,
        mut points: Vec<BilinearPoint<K>>,
    ) -> Option<BilinearInterpolationResult<K>> {
        if points.is_empty() {
            return None;
        }

        points.sort_by(|a, b| a.x.total_cmp(&b.x).then_with(|| a.y.total_cmp(&b.y)));

        let colliding_keys = points
            .windows(2)
            .filter(|w| (w[0].x - w[1].x).abs() < f64::EPSILON && (w[0].y - w[1].y).abs() < f64::EPSILON)
            .flat_map(|w| [w[0].key.clone(), w[1].key.clone()])
            .collect::<Vec<_>>();

        let mut xs = points.iter().map(|p| p.x).collect::<Vec<_>>();
        xs.sort_by(|a, b| a.total_cmp(b));
        xs.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);

        let mut ys = points.iter().map(|p| p.y).collect::<Vec<_>>();
        ys.sort_by(|a, b| a.total_cmp(b));
        ys.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);

        if xs.len() < 2 || ys.len() < 2 {
            return None;
        }

        let ix = xs.partition_point(|v| *v <= x);
        let iy = ys.partition_point(|v| *v <= y);
        if ix == 0 || ix >= xs.len() || iy == 0 || iy >= ys.len() {
            return None;
        }

        let (x0, x1) = (xs[ix - 1], xs[ix]);
        let (y0, y1) = (ys[iy - 1], ys[iy]);

        let lookup = |lx: f64, ly: f64| {
            points
                .iter()
                .find(|p| (p.x - lx).abs() < f64::EPSILON && (p.y - ly).abs() < f64::EPSILON)
                .cloned()
        };

        let p00 = lookup(x0, y0)?;
        let p10 = lookup(x1, y0)?;
        let p01 = lookup(x0, y1)?;
        let p11 = lookup(x1, y1)?;

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

        let v0: ADReal = (p00.value + (p10.value - p00.value) * tx).into();
        let v1: ADReal = (p01.value + (p11.value - p01.value) * tx).into();
        let value: ADReal = (v0 + (v1 - v0) * ty).into();

        Some(BilinearInterpolationResult::new(
            value,
            vec![p00.key, p10.key, p01.key, p11.key],
            colliding_keys,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ad::adreal::{ADReal, IsReal},
        math::interpolation::bilinear::{BilinearInterpolator, BilinearPoint},
    };

    #[test]
    fn bilinear_interpolates_center() {
        let points = vec![
            BilinearPoint { x: 0.0, y: 0.0, value: ADReal::from(1.0), key: 0 },
            BilinearPoint { x: 1.0, y: 0.0, value: ADReal::from(3.0), key: 1 },
            BilinearPoint { x: 0.0, y: 1.0, value: ADReal::from(2.0), key: 2 },
            BilinearPoint { x: 1.0, y: 1.0, value: ADReal::from(4.0), key: 3 },
        ];
        let out = BilinearInterpolator::interpolate(0.5, 0.5, points).expect("center interpolation");
        assert!((out.value().value() - 2.5).abs() < 1e-12);
        assert_eq!(out.interpolation_keys().len(), 4);
    }
}

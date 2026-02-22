use crate::ad::adreal::{ADReal, IsReal};

/// Numeric behavior required for bilinear interpolation.
pub trait BilinearValue: IsReal {
    /// Linearly interpolates between `a` and `b` using scalar weight `t`.
    fn lerp(a: Self, b: Self, t: f64) -> Self;
}

impl BilinearValue for f64 {
    fn lerp(a: Self, b: Self, t: f64) -> Self {
        (b - a).mul_add(t, a)
    }
}

impl BilinearValue for ADReal {
    fn lerp(a: Self, b: Self, t: f64) -> Self {
        (a + (b - a) * t).into()
    }
}

/// # `BilinearPoint`
/// Input point for bilinear interpolation.
#[derive(Clone)]
pub struct BilinearPoint<T: IsReal> {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
    /// Point value.
    pub value: T,
}

/// # `BilinearInterpolator`
/// Bilinear interpolation implementation independent from volatility containers.
pub struct BilinearInterpolator;

impl BilinearInterpolator {
    /// Interpolate at (`x`,`y`) from rectangular points.
    #[must_use]
    pub fn interpolate<T: BilinearValue>(
        x: f64,
        y: f64,
        mut points: Vec<BilinearPoint<T>>,
    ) -> Option<T> {
        if points.is_empty() {
            return None;
        }

        points.sort_by(|a, b| a.x.total_cmp(&b.x).then_with(|| a.y.total_cmp(&b.y)));

        let mut xs = points.iter().map(|p| p.x).collect::<Vec<_>>();
        xs.sort_by(f64::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);

        let mut ys = points.iter().map(|p| p.y).collect::<Vec<_>>();
        ys.sort_by(f64::total_cmp);
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

        let v0 = T::lerp(p00.value, p10.value, tx);
        let v1 = T::lerp(p01.value, p11.value, tx);
        Some(T::lerp(v0, v1, ty))
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
            BilinearPoint { x: 0.0, y: 0.0, value: ADReal::from(1.0) },
            BilinearPoint { x: 1.0, y: 0.0, value: ADReal::from(3.0) },
            BilinearPoint { x: 0.0, y: 1.0, value: ADReal::from(2.0) },
            BilinearPoint { x: 1.0, y: 1.0, value: ADReal::from(4.0) },
        ];
        let out = BilinearInterpolator::interpolate(0.5, 0.5, points).expect("center interpolation");
        assert!((out.value() - 2.5).abs() < 1e-12);
    }
}

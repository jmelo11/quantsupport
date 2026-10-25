use super::{bilinear::BilinearValue, extrapolation::ExtrapolationPolicy};

// The outer axes select slices; each slice owns its inner coordinates. This
// gives ordinary multilinear interpolation on rectangular grids and supports
// different strike sets at each expiry/tenor without inventing missing nodes.
/// Interpolates a multidimensional grid, returning no value for invalid or incomplete inputs.
pub fn interpolate<T: BilinearValue, const N: usize>(
    query: [f64; N],
    mut points: Vec<([f64; N], T)>,
    extrapolation: ExtrapolationPolicy,
) -> Option<T> {
    if query.iter().any(|coordinate| !coordinate.is_finite())
        || points.is_empty()
        || points.iter().any(|(coordinates, value)| {
            coordinates.iter().any(|coordinate| !coordinate.is_finite())
                || !value.value().is_finite()
        })
    {
        return None;
    }

    // Signed zero denotes the same interpolation coordinate.
    for (coordinates, _) in &mut points {
        for coordinate in coordinates {
            if *coordinate == 0.0 {
                *coordinate = 0.0;
            }
        }
    }
    points.sort_by(|(left, _), (right, _)| {
        left.iter()
            .zip(right)
            .map(|(a, b)| a.total_cmp(b))
            .find(|ordering| !ordering.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if points.windows(2).any(|pair| {
        pair[0]
            .0
            .iter()
            .zip(&pair[1].0)
            .all(|(left, right)| left.total_cmp(right).is_eq())
    }) {
        return None;
    }

    interpolate_slice(&query, &points, 0, extrapolation)
}

/// Recursively interpolates one grid axis using the configured extrapolation policy.
fn interpolate_slice<T: BilinearValue, const N: usize>(
    query: &[f64; N],
    points: &[([f64; N], T)],
    axis: usize,
    extrapolation: ExtrapolationPolicy,
) -> Option<T> {
    let &(first_coordinates, first_value) = points.first()?;
    if axis == N {
        return Some(first_value);
    }

    let coordinate = query[axis];
    let first = first_coordinates[axis];
    let last = points.last()?.0[axis];
    if coordinate < first || coordinate > last {
        if extrapolation == ExtrapolationPolicy::Reject {
            return None;
        }
        let boundary = if coordinate < first { first } else { last };
        let start = points.partition_point(|(coordinates, _)| coordinates[axis] < boundary);
        let end = points.partition_point(|(coordinates, _)| coordinates[axis] <= boundary);
        return interpolate_slice(query, &points[start..end], axis + 1, extrapolation);
    }

    let end = points.partition_point(|(coordinates, _)| coordinates[axis] <= coordinate);
    let lower = points.get(end.checked_sub(1)?)?.0[axis];
    let start = points.partition_point(|(coordinates, _)| coordinates[axis] < lower);
    let lower_value = interpolate_slice(query, &points[start..end], axis + 1, extrapolation)?;

    // Exact nodes and boundaries use only their own slice. Neighboring sparse
    // slices need not contain the queried inner coordinates.
    if coordinate
        .partial_cmp(&lower)
        .is_some_and(std::cmp::Ordering::is_eq)
    {
        return Some(lower_value);
    }

    let upper = points.get(end)?.0[axis];
    let upper_end = points.partition_point(|(coordinates, _)| coordinates[axis] <= upper);
    let upper_value = interpolate_slice(query, &points[end..upper_end], axis + 1, extrapolation)?;
    Some(T::lerp(
        lower_value,
        upper_value,
        (coordinate - lower) / (upper - lower),
    ))
}

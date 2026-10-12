//! Extrapolation policies for multidimensional interpolation.

use serde::{Deserialize, Serialize};

/// Behavior when a query lies outside an interpolation axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtrapolationPolicy {
    /// Return an error instead of extending the grid, including singleton axes.
    #[default]
    Reject,
    /// Use the nearest boundary value along each axis.
    ///
    /// An axis containing one coordinate is constant under this policy.
    Flat,
}

use crate::{ad::scalar::Scalar, utils::errors::Result};

use super::prepaymentmodelstate::PrepaymentModelState;

/// Extension point for conditional loan prepayment probabilities, including AD models.
pub trait PrepaymentModel<T: Scalar>: Send + Sync {
    /// Whether this function needs a market refinancing observation.
    fn requires_refinancing(&self) -> bool {
        false
    }
    /// Returns a probability in [0, 1] conditional on surviving to this interval.
    ///
    /// # Errors
    /// Returns an error for invalid parameters or unavailable observations.
    fn probability(&self, state: &PrepaymentModelState<T>) -> Result<T>;
}

impl<T: Scalar, F> PrepaymentModel<T> for F
where
    F: Fn(&PrepaymentModelState<T>) -> Result<T> + Send + Sync,
{
    fn probability(&self, state: &PrepaymentModelState<T>) -> Result<T> {
        self(state)
    }
}

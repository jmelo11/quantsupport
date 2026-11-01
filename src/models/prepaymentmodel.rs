use crate::{ad::scalar::Scalar, utils::errors::Result};

use super::prepaymentmodelstate::PrepaymentModelState;

/// Extension point for conditional loan prepayment probabilities, including AD models.
///
/// The pricer calls the model after scheduled principal and reduces future
/// expected receipts by the returned interval probability. Implementations can
/// retain scalar AD dependencies on observed rates and outstanding principal.
pub trait PrepaymentModel<T: Scalar>: Send + Sync {
    /// Whether this model requires an observed refinancing benchmark index rate.
    ///
    /// Defaults to `false`. Return `true` to require an explicit refinancing
    /// benchmark index on the loan pricer; otherwise the benchmark rate is zero.
    fn requires_refinancing_benchmark(&self) -> bool {
        false
    }
    /// Returns a probability in [0, 1] conditional on surviving to this interval.
    ///
    /// An annual CPR must be converted using the state's year fraction, for
    /// example `1 - (1 - CPR)^year_fraction`. The result applies to the balance
    /// after scheduled principal, rather than the original principal.
    ///
    /// # Errors
    /// Returns an error for invalid parameters or unavailable observations.
    fn conditional_prepayment_probability(&self, state: &PrepaymentModelState<T>) -> Result<T>;
}

impl<T: Scalar, F> PrepaymentModel<T> for F
where
    F: Fn(&PrepaymentModelState<T>) -> Result<T> + Send + Sync,
{
    /// Calls the supplied closure; it does not request refinancing data by default.
    ///
    /// # Errors
    /// Propagates errors returned by the closure.
    fn conditional_prepayment_probability(&self, state: &PrepaymentModelState<T>) -> Result<T> {
        self(state)
    }
}

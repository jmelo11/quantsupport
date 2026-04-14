use crate::{ad::scalar::Scalar, utils::errors::Result};

/// Interface for Monte Carlo path generation.
///
/// Models implement this trait to fill a `scenario` buffer with simulated
/// values at each time step. Standard-normal draws are supplied externally
/// so that the generator stays deterministic and pure.
pub trait PathGenerator<T: Scalar> {
    /// Fills `scenario[i]` with the simulated value at `times[i]`, using
    /// the provided standard-normal `draws` (one per time step).
    ///
    /// `draws.len()` and `scenario.len()` must both equal `times.len()`.
    ///
    /// # Errors
    /// Returns an error if input lengths mismatch or simulation fails.
    fn generate(&self, times: &[f64], draws: &[f64], scenario: &mut [T]) -> Result<()>;
}

/// A volatility function that depends on time.
pub trait TimeDependentVolatility<T: Scalar> {
    /// Returns the volatility at time `t` (in years).
    ///
    /// # Errors
    /// Returns an error if the volatility cannot be evaluated at `t`.
    fn vol(&self, t: f64) -> Result<T>;
}

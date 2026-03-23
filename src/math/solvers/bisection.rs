use crate::math::solvers::solvertraits::{ContFunc, OptimizerSolution, SolutionStatus};
use crate::utils::errors::{QSError, Result};

/// # `Bisection`
///
/// Simple bisection solver for scalar problems.
pub struct Bisection<P> {
    _p: std::marker::PhantomData<P>,
    lower: f64,
    upper: f64,
    ftol: f64,
    max_iter: i64,
}

impl<P> Bisection<P>
where
    P: ContFunc<f64>,
{
    /// Creates a new bisection solver.
    #[must_use]
    pub const fn new(lower: f64, upper: f64, max_iter: i64) -> Self {
        Self {
            _p: std::marker::PhantomData,
            lower,
            upper,
            ftol: 1e-12,
            max_iter,
        }
    }

    /// Sets the convergence tolerance.
    #[must_use]
    pub const fn with_ftol(mut self, ftol: f64) -> Self {
        self.ftol = ftol;
        self
    }

    /// Sets the lower bound.
    #[must_use]
    pub const fn with_lower(mut self, lower: f64) -> Self {
        self.lower = lower;
        self
    }

    /// Sets the upper bound.
    #[must_use]
    pub const fn with_upper(mut self, upper: f64) -> Self {
        self.upper = upper;
        self
    }

    /// Solves the problem using bisection.
    ///
    /// ## Errors
    /// Returns an error if the function does not change sign over the interval or if the solver fails to converge within the maximum number of iterations.
    pub fn solve(&self, f: &P) -> Result<OptimizerSolution<f64>> {
        let mut low = self.lower;
        let mut high = self.upper;
        let mut f_low = f.call(&low)?;
        let f_high = f.call(&high)?;

        if f_low.signum() == f_high.signum() {
            return Err(QSError::SolverErr(
                "Bisection requires a sign change over the bracket.".into(),
            ));
        }

        for _ in 0..self.max_iter {
            let mid = 0.5 * (low + high);
            let f_mid = f.call(&mid)?;
            if f_mid.abs() < self.ftol {
                return Ok(OptimizerSolution {
                    x: mid,
                    f: f_mid,
                    status: SolutionStatus::Converged,
                    jacobian: None,
                });
            }
            if f_mid.signum() == f_low.signum() {
                low = mid;
                f_low = f_mid;
            } else {
                high = mid;
            }
        }

        let mid = 0.5 * (low + high);
        Ok(OptimizerSolution {
            x: mid,
            f: f.call(&mid)?,
            status: SolutionStatus::NotConverged,
            jacobian: None,
        })
    }
}

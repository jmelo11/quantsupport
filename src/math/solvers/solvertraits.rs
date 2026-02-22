use crate::utils::errors::Result;
use std::ops::Sub;

/// # `SolutionStatus`
///
/// Status of an optimization/solver run.
#[derive(Debug)]
pub enum SolutionStatus {
    /// Solver reached convergence criteria.
    Converged,
    /// Solver finished without meeting convergence criteria.
    NotConverged,
}

/// # `OptimizerSolution`
///
/// Solution container returned by solvers.
#[derive(Debug)]
pub struct OptimizerSolution<X> {
    /// The solution value (e.g. the root or parameter vector).
    pub x: X,
    /// Objective value at the solution (often residual / function value).
    pub f: f64,
    /// Solver status indicating convergence or not.
    pub status: SolutionStatus,
}

/// # `ContFunc`
///
/// Trait for a continuous scalar function (or objective) over `X`.
///
/// Implementors should return the function value (or an error string).
pub trait ContFunc<X> {
    /// Evaluate the function at `x`.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the function evaluation fails.
    fn call(&self, x: &X) -> Result<f64>;
}

/// # `C1Func`
///
/// First-order function: extends `ContFunc` with a gradient.
///
/// ## Errors
/// Returns an [`AtlasError`] if the gradient computation fails.
pub trait C1Func<X>: ContFunc<X> {
    /// Return the gradient at `x`.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the gradient computation fails.
    fn grad(&self, x: &X) -> Result<X>;
}

/// # `C2Func`
///
/// Second-order (or Hessian) interface.
///
/// `inv_hess` returns a type representing the inverse Hessian or an object
/// useful to compute Newton-like steps. The concrete `H` type is left generic.
///
/// ## Errors
/// Returns an [`AtlasError`] if the inverse Hessian computation fails.
pub trait C2Func<X, H>: C1Func<X> {
    /// Return the inverse of the Hessian.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the inverse Hessian computation fails.
    fn inv_hess(&self, x: &X) -> Result<H>;
}

/// # `DescentMethod`
///
/// Generic descent-method trait with a default `solve` implementation.
///
/// The trait is generic over the problem `P` and solution type `X`. Implementors
/// must provide initialization, stopping tolerance, a step rule and the
/// maximum iterations. The provided `solve` routine uses those methods to run
/// a simple loop and return an `OptimizerSolution`.
pub trait DescentMethod<P, X>
where
    P: ContFunc<X>,
    X: Sub<X, Output = X> + Copy,
{
    /// Maximum number of iterations.
    fn max_iter(&self) -> i64;

    /// Initial guess.
    fn x0(&self) -> X;

    /// Convergence tolerance on the objective value.
    fn ftol(&self) -> f64;

    /// Compute a step given current `x`, problem `f` and current function value `fval`.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the step computation fails.
    fn step(&self, x: &X, f: &P, fval: f64) -> Result<X>;

    /// Solve the problem using the provided builder methods.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the function evaluation or step computation fails.
    fn solve(&self, f: &P) -> Result<OptimizerSolution<X>> {
        let mut x = self.x0();
        let mut fval = 0.0;

        for _ in 0..self.max_iter() {
            fval = f.call(&x)?;
            if fval.abs() < self.ftol() {
                return Ok(OptimizerSolution {
                    x,
                    f: fval,
                    status: SolutionStatus::Converged,
                });
            }
            x = x - self.step(&x, f, fval)?;
        }
        Ok(OptimizerSolution {
            x,
            f: fval,
            status: SolutionStatus::NotConverged,
        })
    }
}

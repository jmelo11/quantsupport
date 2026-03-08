use crate::ad::{
    adreal::{ADReal, IsReal},
    tape::Tape,
};
use crate::utils::errors::{QSError, Result};
use std::ops::Sub;

/// Dense matrix alias used by solver interfaces.
pub type Matrix<T> = Vec<Vec<T>>;

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

/// Solution container returned by solvers.
#[derive(Debug)]
pub struct OptimizerSolution<X, F = f64> {
    /// The solution value (e.g. the root or parameter vector).
    pub x: X,
    /// Objective value at the solution (often residual / function value).
    pub f: F,
    /// Solver status indicating convergence or not.
    pub status: SolutionStatus,
}

/// Trait for a continuous function (or objective) over `X`.
///
/// Implementors should return the function value (or an error string).
pub trait ContFunc<X: ?Sized, Y = f64> {
    /// Evaluate the function at `x`.
    ///
    /// ## Errors
    /// Returns an [`QSError`] if the function evaluation fails.
    fn call(&self, x: &X) -> Result<Y>;
}

/// First-order function: extends `ContFunc` with a gradient.
///
/// ## Errors
/// Returns an [`QSError`] if the gradient computation fails.
pub trait C1Func<X>: ContFunc<X, f64> {
    /// Return the gradient at `x`.
    ///
    /// ## Errors
    /// Returns an [`QSError`] if the gradient computation fails.
    fn grad(&self, x: &X) -> Result<X>;
}

/// Second-order (or Hessian) interface.
///
/// [`Self::inv_hess`] returns a type representing the inverse Hessian or an object
/// useful to compute Newton-like steps. The concrete `H` type is left generic.
///
/// ## Errors
/// Returns an [`QSError`] if the inverse Hessian computation fails.
pub trait C2Func<X, H>: C1Func<X> {
    /// Return the inverse of the Hessian.
    ///
    /// ## Errors
    /// Returns an [`QSError`] if the inverse Hessian computation fails.
    fn inv_hess(&self, x: &X) -> Result<H>;
}

/// Generic descent-method trait with a default `solve` implementation.
///
/// The trait is generic over the problem `P` and solution type `X`. Implementors
/// must provide initialization, stopping tolerance, a step rule and the
/// maximum iterations. The provided `solve` routine uses those methods to run
/// a simple loop and return an `OptimizerSolution`.
pub trait DescentMethod<P, X>
where
    P: ContFunc<X, f64>,
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
    /// Returns an [`QSError`] if the step computation fails.
    fn step(&self, x: &X, f: &P, fval: f64) -> Result<X>;

    /// Solve the problem using the provided builder methods.
    ///
    /// ## Errors
    /// Returns an [`QSError`] if the function evaluation or step computation fails.
    fn solve(&self, f: &P) -> Result<OptimizerSolution<X, f64>> {
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

/// Trait for vector-valued systems over generic input/output scalar types.
///
/// The primary use-case is solving systems of equations $F(x)=0$ where `x`
/// and `F(x)` are vectors.
pub trait VectorFunc<X, Y>: ContFunc<[X], Vec<Y>> {}

/// Trait for vector-valued systems that can provide Jacobians.
///
/// This extends [`VectorFunc`] with a Jacobian interface used by Newton-like
/// methods.
pub trait JacobianFunc<X, Y, J>: VectorFunc<X, Y> {
    /// Computes the Jacobian matrix of the residual vector at `x`.
    ///
    /// # Errors
    /// Returns an error if Jacobian evaluation fails.
    fn jacobian(&self, x: &[X]) -> Result<Matrix<J>>;
}

/// AD specialization for Jacobian evaluation using reverse-mode autodiff.
///
/// Implement this trait for an AD vector function to automatically obtain a
/// [`JacobianFunc<ADReal, ADReal>`] implementation.
pub trait ADJacobian: VectorFunc<ADReal, ADReal> {
    /// Computes an autodiff Jacobian at `x`.
    ///
    /// # Errors
    /// Returns an error if residual evaluation fails, dimensions mismatch, or
    /// if AD backpropagation fails.
    fn jacobian_ad(&self, x: &[ADReal]) -> Result<Matrix<f64>> {
        let started_locally = if Tape::is_active() {
            false
        } else {
            Tape::start_recording();
            true
        };

        let result = (|| {
            Tape::set_mark();
            let local_x = x
                .iter()
                .map(|value| ADReal::new(value.value()))
                .collect::<Vec<_>>();
            let residual = self.call(&local_x)?;
            let n = x.len();

            if residual.len() != n {
                return Err(QSError::SolverErr(
                    "Vector function must return residual size equal to variable size".into(),
                ));
            }

            for row in 0..n {
                if !residual[row].is_on_tape() {
                    return Err(QSError::NodeNotIndexedInTapeErr);
                }
            }

            let mut j = vec![vec![0.0; n]; n];
            for row in 0..n {
                Tape::reset_adjoints();
                residual[row].backward_to_mark()?;
                for col in 0..n {
                    j[row][col] = local_x[col].adjoint()?;
                }
            }

            Tape::reset_adjoints();

            Ok(j)
        })();

        if started_locally {
            Tape::stop_recording();
            Tape::rewind_to_init();
        } else {
            Tape::rewind_to_mark();
        }

        result
    }
}

impl<T> JacobianFunc<ADReal, ADReal, f64> for T
where
    T: ADJacobian,
{
    fn jacobian(&self, x: &[ADReal]) -> Result<Matrix<f64>> {
        self.jacobian_ad(x)
    }
}

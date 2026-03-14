use crate::utils::errors::{QSError, Result};
use nalgebra::{DMatrix, DVector};
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
    /// Optional Jacobian captured at the solution point.
    pub jacobian: Option<Matrix<f64>>,
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
                    jacobian: None,
                });
            }
            x = x - self.step(&x, f, fval)?;
        }
        Ok(OptimizerSolution {
            x,
            f: fval,
            status: SolutionStatus::NotConverged,
            jacobian: None,
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

    /// Solves the implicit-function-theorem system `J × S = -diag(g)`.
    ///
    /// # Errors
    /// Returns an error if the Jacobian dimensions are inconsistent or the
    /// linear solves fail.
    fn solve_ift(&self, x: &[X], g_diag: &[f64]) -> Result<Matrix<f64>>
    where
        J: Copy + Into<f64>,
    {
        let j = self.jacobian(x)?;
        let n = g_diag.len();
        if j.len() != n || j.iter().any(|row| row.len() != n) {
            return Err(QSError::SolverErr(
                "IFT requires a square Jacobian matching the quote sensitivity size".into(),
            ));
        }

        let jacobian = j
            .iter()
            .map(|row| row.iter().copied().map(Into::into).collect::<Vec<f64>>())
            .collect::<Vec<_>>();

        let data = jacobian
            .iter()
            .flat_map(|row| row.iter().copied())
            .collect::<Vec<_>>();
        let matrix = DMatrix::from_row_slice(n, n, &data);

        let mut sensitivities = vec![vec![0.0; n]; n];
        for j_col in 0..n {
            let mut rhs = vec![0.0; n];
            rhs[j_col] = -g_diag[j_col];
            let rhs = DVector::from_row_slice(&rhs);
            let column = matrix
                .clone()
                .lu()
                .solve(&rhs)
                .ok_or_else(|| QSError::SolverErr("Singular Jacobian in IFT".into()))?;
            for i in 0..n {
                sensitivities[i][j_col] = column[i];
            }
        }

        Ok(sensitivities)
    }
}

use crate::{
    math::solvers::solvertraits::{JacobianFunc, OptimizerSolution, SolutionStatus},
    utils::errors::{QSError, Result},
};
use nalgebra::{DMatrix, DVector};

/// Dense Newton solver for vector systems $F(x)=0$.
///
/// This solver operates on `f64` vectors and relies on the
/// [`JacobianFunc`] interface for residual and Jacobian evaluation.
pub struct VectorNewton<P> {
    _p: std::marker::PhantomData<P>,
    tol: f64,
    max_iter: usize,
}

impl<P> VectorNewton<P>
where
    P: JacobianFunc<f64, f64, f64>,
{
    /// Creates a new vector Newton solver.
    #[must_use]
    pub const fn new(tol: f64, max_iter: usize) -> Self {
        Self {
            _p: std::marker::PhantomData,
            tol,
            max_iter,
        }
    }

    /// Solves the vector system.
    ///
    /// # Errors
    /// Returns an error if dimensions mismatch, Jacobian is singular, or the
    /// solver does not converge.
    pub fn solve(&self, problem: &P, x0: &[f64]) -> Result<OptimizerSolution<Vec<f64>>> {
        let mut x = x0.to_vec();
        let mut terminal_jacobian = None;

        for _ in 0..self.max_iter {
            let r = problem.call(&x)?;
            if r.len() != x.len() {
                return Err(QSError::SolverErr(
                    "Vector solver requires residual size == variable size".into(),
                ));
            }

            let norm = Self::norm_inf(&r);
            if norm < self.tol {
                let jacobian = if let Some(jacobian) = terminal_jacobian.take() {
                    jacobian
                } else {
                    problem.jacobian(&x)?
                };
                return Ok(OptimizerSolution {
                    x,
                    f: norm,
                    status: SolutionStatus::Converged,
                    jacobian: Some(jacobian),
                });
            }

            let j = problem.jacobian(&x)?;
            let n = j.len();
            if r.len() != n || j.iter().any(|row| row.len() != n) {
                return Err(QSError::SolverErr(
                    "Linear system dimensions are inconsistent".into(),
                ));
            }

            let dx = if n == 0 {
                Vec::new()
            } else {
                let data = j
                    .iter()
                    .flat_map(|row| row.iter().copied())
                    .collect::<Vec<_>>();
                let matrix = DMatrix::from_row_slice(n, n, &data);
                let rhs = DVector::from_row_slice(&r);
                matrix
                    .lu()
                    .solve(&rhs)
                    .ok_or_else(|| QSError::SolverErr("Singular Jacobian".into()))?
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
            };

            let current_norm = norm;
            let mut step = 1.0;
            let mut accepted = false;

            for _ in 0..12 {
                let candidate = x
                    .iter()
                    .zip(dx.iter())
                    .map(|(xi, dxi)| *xi - *dxi * step)
                    .collect::<Vec<_>>();

                if candidate.iter().any(|v| !v.is_finite() || *v <= 0.0) {
                    step *= 0.5;
                    continue;
                }

                let cand_norm = Self::norm_inf(&problem.call(&candidate)?);
                if cand_norm <= current_norm {
                    terminal_jacobian = if cand_norm < self.tol {
                        Some(problem.jacobian(&candidate)?)
                    } else {
                        None
                    };
                    x = candidate;
                    accepted = true;
                    break;
                }
                step *= 0.5;
            }

            if !accepted {
                return Err(QSError::SolverErr(
                    "Vector Newton step failed to improve residual".into(),
                ));
            }
        }

        let final_norm = Self::norm_inf(&problem.call(&x)?);
        let final_jacobian = problem.jacobian(&x)?;
        Ok(OptimizerSolution {
            x,
            f: final_norm,
            status: SolutionStatus::NotConverged,
            jacobian: Some(final_jacobian),
        })
    }

    fn norm_inf(v: &[f64]) -> f64 {
        v.iter().map(|x| x.abs()).fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::{
        math::solvers::{
            solvertraits::{ContFunc, JacobianFunc, VectorFunc},
            vectornewton::VectorNewton,
        },
        utils::errors::Result,
    };

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    struct SquareRootProblem {
        target: f64,
    }

    impl ContFunc<[f64], Vec<f64>> for SquareRootProblem {
        fn call(&self, x: &[f64]) -> Result<Vec<f64>> {
            Ok(vec![x[0] * x[0] - self.target])
        }
    }

    impl JacobianFunc<f64, f64, f64> for SquareRootProblem {
        fn jacobian(&self, x: &[f64]) -> Result<Vec<Vec<f64>>> {
            Ok(vec![vec![2.0 * x[0]]])
        }
    }

    impl VectorFunc<f64, f64> for SquareRootProblem {}

    #[test]
    fn solves_f64_system_and_reuses_jacobian() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let solver = VectorNewton::<SquareRootProblem>::new(1e-12, 32);
        let solution = solver
            .solve(&SquareRootProblem { target: 4.0 }, &[1.5])
            .expect("vector Newton should converge");

        assert!((solution.x[0] - 2.0).abs() < 1e-10);
        let jacobian = solution.jacobian.as_ref().expect("solver jacobian");
        assert!((jacobian[0][0] - 4.0).abs() < 1e-8);

        let problem = SquareRootProblem { target: 4.0 };
        let ift = problem.solve_ift(&solution.x, &[1.0]).expect("ift solve");
        assert!((ift[0][0] + 0.25).abs() < 1e-8);
    }
}

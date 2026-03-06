use crate::{
    ad::adreal::{ADReal, FloatExt, IsReal},
    math::solvers::solvertraits::{JacobianFunc, Matrix, OptimizerSolution, SolutionStatus},
    utils::errors::{QSError, Result},
};

/// Dense Newton solver for vector systems $F(x)=0$.
///
/// This solver operates on vectors of [`ADReal`] and relies on the
/// [`JacobianFunc`] interface for residual and Jacobian evaluation.
pub struct VectorNewton<P> {
    _p: std::marker::PhantomData<P>,
    tol: f64,
    max_iter: usize,
}

impl<P> VectorNewton<P>
where
    P: JacobianFunc<ADReal, ADReal, ADReal>,
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
    pub fn solve(&self, problem: &P, x0: &[ADReal]) -> Result<OptimizerSolution<Vec<ADReal>>> {
        let mut x = x0.to_vec();

        for _ in 0..self.max_iter {
            let r = problem.call(&x)?;
            if r.len() != x.len() {
                return Err(QSError::SolverErr(
                    "Vector solver requires residual size == variable size".into(),
                ));
            }

            let norm = Self::norm_inf(&r);
            if norm < self.tol {
                return Ok(OptimizerSolution {
                    x,
                    f: norm,
                    status: SolutionStatus::Converged,
                });
            }

            let j = problem.jacobian(&x)?;
            let dx = Self::solve_linear_system(j, r)?;

            let current_norm = norm;
            let mut step = 1.0;
            let mut accepted = false;

            for _ in 0..12 {
                let candidate = x
                    .iter()
                    .zip(dx.iter())
                    .map(|(xi, dxi)| (*xi - *dxi * step).into())
                    .collect::<Vec<_>>();

                if candidate
                    .iter()
                    .any(|v: &ADReal| !v.value().is_finite() || v.value() <= 0.0)
                {
                    step *= 0.5;
                    continue;
                }

                let cand_norm = Self::norm_inf(&problem.call(&candidate)?);
                if cand_norm <= current_norm {
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
        Ok(OptimizerSolution {
            x,
            f: final_norm,
            status: SolutionStatus::NotConverged,
        })
    }

    fn solve_linear_system(mut a: Matrix<ADReal>, mut b: Vec<ADReal>) -> Result<Vec<ADReal>> {
        let n = a.len();
        if b.len() != n || a.iter().any(|row| row.len() != n) {
            return Err(QSError::SolverErr(
                "Linear system dimensions are inconsistent".into(),
            ));
        }

        for i in 0..n {
            let mut pivot = i;
            let mut max_val = a[i][i].abs();
            for (r, row) in a.iter().enumerate().skip(i + 1) {
                if row[i].abs() > max_val {
                    max_val = row[i].abs();
                    pivot = r;
                }
            }

            if max_val < 1e-14 {
                return Err(QSError::SolverErr("Singular Jacobian".into()));
            }

            if pivot != i {
                a.swap(i, pivot);
                b.swap(i, pivot);
            }

            let diag = a[i][i];
            for c in i..n {
                a[i][c] /= diag;
            }
            b[i] /= diag;

            for r in 0..n {
                if r == i {
                    continue;
                }
                let factor = a[r][i];
                if factor == 0.0 {
                    continue;
                }
                for c in i..n {
                    a[r][c] -= factor * a[i][c];
                }
                b[r] -= factor * b[i];
            }
        }

        Ok(b)
    }

    fn norm_inf(v: &[ADReal]) -> f64 {
        v.iter().map(|x| x.value().abs()).fold(0.0, f64::max)
    }
}

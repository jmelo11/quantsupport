use crate::math::solvers::solvertraits::{
    C1Func, DescentMethod, OptimizerSolution, SolutionStatus,
};
use crate::utils::errors::{AtlasError, Result};

/// # `NewtonRaphson`
///
/// Simple Newton–Raphson solver for scalar problems (`f64`).
///
/// This struct stores solver parameters and provides builder-style setters
/// for convenience. It implements [`DescentMethod`] for `f64` when the problem
/// implements [`C1Func<f64>`].
pub struct NewtonRaphson<P, X> {
    _p: std::marker::PhantomData<P>,
    _x: std::marker::PhantomData<X>,
    ftol: f64,
    x0: f64,
    max_iter: i64,
}

impl<P, X> NewtonRaphson<P, X>
where
    P: C1Func<X>,
{
    /// Create a new [`NewtonRaphson`] solver with initial guess `x0` and `max_iter`.
    #[must_use]
    pub const fn new(x0: f64, max_iter: i64) -> Self {
        Self {
            _p: std::marker::PhantomData,
            _x: std::marker::PhantomData,
            ftol: 1e-16,
            x0,
            max_iter,
        }
    }

    /// Set initial guess.
    #[must_use]
    pub const fn with_x0(mut self, x0: f64) -> Self {
        self.x0 = x0;
        self
    }

    /// Set maximum iterations.
    #[must_use]
    pub const fn with_max_iter(mut self, max_iter: i64) -> Self {
        self.max_iter = max_iter;
        self
    }

    /// Set the tolerance on the objective value for convergence.
    #[must_use]
    pub const fn with_ftol(mut self, ftol: f64) -> Self {
        self.ftol = ftol;
        self
    }
}

impl<P> DescentMethod<P, f64> for NewtonRaphson<P, f64>
where
    P: C1Func<f64>,
{
    fn ftol(&self) -> f64 {
        self.ftol
    }

    fn max_iter(&self) -> i64 {
        self.max_iter
    }
    fn step(&self, x: &f64, f: &P, fval: f64) -> Result<f64> {
        let g = f.grad(x)?;
        if g == 0.0 {
            return Err(AtlasError::SolverErr("Gradient == 0.".into()));
        }
        Ok(fval / g)
    }

    fn x0(&self) -> f64 {
        self.x0
    }

    fn solve(&self, f: &P) -> Result<OptimizerSolution<f64>> {
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

#[cfg(test)]
mod test {
    use crate::math::solvers::{
        newtonraphson::NewtonRaphson,
        solvertraits::{C1Func, ContFunc, DescentMethod},
    };
    use std::f64::consts::PI;

    fn norm_pdf(x: f64) -> f64 {
        (1.0 / (2.0 * PI).sqrt()) * (-0.5 * x * x).exp()
    }

    fn norm_cdf(x: f64) -> f64 {
        // Abramowitz and Stegun approximation for the error function
        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();
        let t = 1.0 / 0.3275911f64.mul_add(x, 1.0);
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let poly = (a5 * t + a4).mul_add(t, a3).mul_add(t, a2).mul_add(t, a1);
        let y = (poly * t).mul_add(-(-x * x).exp(), 1.0);
        let erf = sign * y;
        0.5 * (1.0 + erf)
    }

    fn bs_price(
        spot: f64,
        sigma: f64,
        tau: f64,
        strike: f64,
        r: f64,
    ) -> crate::utils::errors::Result<f64> {
        if tau < 0.0 {
            return Err(crate::utils::errors::AtlasError::SolverErr(
                "Negative tau.".into(),
            ));
        }
        if sigma < 0.0 {
            return Err(crate::utils::errors::AtlasError::SolverErr(
                "Negative sigma.".into(),
            ));
        }
        if spot < 0.0 || strike < 0.0 {
            return Err(crate::utils::errors::AtlasError::SolverErr(
                "Negative spot|strike.".into(),
            ));
        }

        let d1 = 0.5f64.mul_add(sigma.powi(2), r).mul_add(tau, (spot / strike).ln()) / (sigma * tau.sqrt());
        let d2 = sigma.mul_add(-tau.sqrt(), d1);
        let discount = (-r * tau).exp();
        Ok(spot.mul_add(norm_cdf(d1), -(strike * discount * norm_cdf(d2))))
    }

    fn bs_vega(
        spot: f64,
        sigma: f64,
        tau: f64,
        strike: f64,
        r: f64,
    ) -> crate::utils::errors::Result<f64> {
        if tau < 0.0 {
            return Err(crate::utils::errors::AtlasError::SolverErr(
                "Negative tau.".into(),
            ));
        }
        if sigma < 0.0 {
            return Err(crate::utils::errors::AtlasError::SolverErr(
                "Negative sigma.".into(),
            ));
        }
        if spot < 0.0 || strike < 0.0 {
            return Err(crate::utils::errors::AtlasError::SolverErr(
                "Negative spot|strike.".into(),
            ));
        }
        let d1 = 0.5f64.mul_add(sigma.powi(2), r).mul_add(tau, (spot / strike).ln()) / (sigma * tau.sqrt());

        Ok(spot * norm_pdf(d1) * tau.sqrt())
    }

    struct ImpliedBlackVol {
        spot: f64,
        strike: f64,
        tau: f64,
        r: f64,
        target_price: f64,
    }

    impl ImpliedBlackVol {
        pub fn new(spot: f64, strike: f64, tau: f64, r: f64, target_price: f64) -> Self {
            Self {
                spot,
                strike,
                tau,
                r,
                target_price,
            }
        }
    }

    impl ContFunc<f64> for ImpliedBlackVol {
        fn call(&self, x: &f64) -> crate::utils::errors::Result<f64> {
            Ok(bs_price(self.spot, *x, self.tau, self.strike, self.r)? - self.target_price)
        }
    }

    impl C1Func<f64> for ImpliedBlackVol {
        fn grad(&self, x: &f64) -> crate::utils::errors::Result<f64> {
            bs_vega(self.spot, *x, self.tau, self.strike, self.r)
        }
    }

    #[test]
    fn example_test() {
        let x0 = 0.5;
        let max_iter = 100;
        let solver = NewtonRaphson::new(x0, max_iter);

        let spot = 100.0;
        let strike = 100.0;
        let tau = 1.0;
        let r = 0.05;
        let target_sigma = 0.2;
        let target_price = bs_price(spot, target_sigma, tau, strike, r).unwrap();
        let problem = ImpliedBlackVol::new(spot, strike, tau, r, target_price);

        let result = solver.solve(&problem);
        println!("{result:?}");
    }
}

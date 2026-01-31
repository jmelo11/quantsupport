// use crate::{
//     errors::errors::LibError,
//     solvers::solver::{Solver, SolverResults, SolverStatus},
// };

// pub struct NewtonRaphson {
//     f: Box<dyn Fn(f64) -> f64>,
//     f_prime: Box<dyn Fn(f64) -> f64>,
//     f_tol: f64,
//     x0: f64,
//     max_iter: i16,
// }

// impl NewtonRaphson {
//     pub fn new(f: Box<dyn Fn(f64) -> f64>, f_prime: Box<dyn Fn(f64) -> f64>) -> Self {
//         Self {
//             f: f,
//             f_prime: f_prime,
//             f_tol: 1e-8,
//             x0: 0.0,
//             max_iter: 100,
//         }
//     }

//     pub fn with_x0(mut self, x0: f64) -> Self {
//         self.x0 = x0;
//         self
//     }

//     pub fn with_max_iter(mut self, max_iter: i16) -> Self {
//         self.max_iter = max_iter;
//         self
//     }

//     pub fn f_tol(&self) -> f64 {
//         self.f_tol
//     }
// }

// impl Solver for NewtonRaphson {
//     fn solve(&self) -> Result<SolverResults, LibError> {
//         let mut x = self.x0;
//         let mut fval = (self.f)(x);

//         for i in 0..self.max_iter {
//             // Condition to stop (example: tolerance on f(x))
//             let fprime = (self.f_prime)(x);

//             if fval.abs() < self.f_tol || fprime.abs() < f64::EPSILON {
//                 return Ok(SolverResults {
//                     f: fval,
//                     x: x,
//                     status: SolverStatus::FoundSolution,
//                     iters: i,
//                 });
//             }

//             x = x - fval / fprime;
//             fval = (self.f)(x);
//         }

//         Ok(SolverResults {
//             f: fval,
//             x: x,
//             status: SolverStatus::MaxIterReached,
//             iters: self.max_iter,
//         })
//     }
// }

// #[cfg(test)]
// mod tests {
//     use crate::{
//         errors::errors::LibError,
//         solvers::newtonraphson::{NewtonRaphson, Solver},
//     };

//     #[test]
//     fn minimze_cuadratic() -> Result<(), LibError> {
//         let f = Box::new(|x: f64| x * x);
//         let g = Box::new(|x: f64| 2.0 * x);
//         let solver = NewtonRaphson::new(f, g).with_x0(5.0).with_max_iter(100);
//         let result = solver.solve()?;
//         println!("{result:?}");
//         assert!(result.f < solver.f_tol());
//         Ok(())
//     }
// }

/// Finds a bracketed scalar root by bisection.
pub mod bisection;
/// Finds a scalar root with Newton–Raphson iteration.
pub mod newtonraphson;
/// Defines the common solver interfaces and convergence results.
pub mod solvertraits;
/// Solves systems of residual equations with vector Newton iteration.
pub mod vectornewton;

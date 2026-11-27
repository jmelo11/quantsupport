//! Mathematical functions and utilities.
//!
//! Interpolation methods, probability distributions, and numerical solvers.

/// Evaluates the forward fast Fourier transform for numerical scalar inputs.
pub mod fft;
/// Evaluates the inverse fast Fourier transform for numerical scalar inputs.
pub mod ifft;
/// Interpolates curves, surfaces and cubes under their selected numerical conventions.
pub mod interpolation;
/// Solves the linear systems used by numerical calibration.
pub mod linalg;
/// Provides the probability distributions and related functions.
pub mod probability;
/// Generates the pseudo-random and low-discrepancy samples used by the simulation models.
pub mod random;
/// Provides the scalar and vector numerical solvers.
pub mod solvers;

/// Accumulates finite scalar values while limiting floating-point summation error.
pub mod compensatedsum;

//! Mathematical functions and utilities.
//!
//! Interpolation methods, probability distributions, and numerical solvers.

pub mod fft;
pub mod ifft;
pub mod interpolation;
/// Linear-algebra helpers.
pub mod linalg;
/// Probability distributions and related functions.
pub mod probability;
pub mod random;
/// Scalar and vector numerical solvers.
pub mod solvers;

/// Numerically stable finite scalar accumulation.
pub mod compensatedsum;

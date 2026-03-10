//! Interest rates, term structures, and bootstrapping.
//!
//! Compounding conventions, interest-rate arithmetic, yield term
//! structures, and multi-curve bootstrapping algorithms.

/// Bootstrapping algorithms for interest rate curves.
pub mod bootstrapping;
/// Compounding types for interest rate calculations.
pub mod compounding;
/// Interest rate calculations and operations.
pub mod interestrate;
/// Yield term structure and related calculations.
pub mod yieldtermstructure;

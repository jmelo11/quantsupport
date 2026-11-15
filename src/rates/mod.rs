//! Interest rates, term structures, and bootstrapping.
//!
//! Compounding conventions, interest-rate arithmetic, yield term
//! structures, and multi-curve bootstrapping algorithms.

/// Calibrates discount and forwarding curves to quoted market instruments.
pub mod bootstrapping;
/// Defines how quoted interest rates accumulate over an accrual interval.
pub mod compounding;
/// Interprets rates under their day-count, compounding and frequency conventions.
pub mod interestrate;
/// Represents discount and forwarding curves under a common query interface.
pub mod yieldtermstructure;

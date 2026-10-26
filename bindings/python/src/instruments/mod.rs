//! Instruments bindings and crate-internal implementation.

pub mod coupons;
pub mod loan;
pub mod loan_terms;
pub mod products;
pub mod scripting;

/// Product terms resolved automatically when a Trade is evaluated.
pub mod unresolved;

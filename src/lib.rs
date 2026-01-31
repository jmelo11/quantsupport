//! `RustAtlas` is a Rust library for financial calculations and analysis.
//!
//! This library provides tools for working with asset-liability management,
//! cash flows, financial instruments, interest rates, and related computations.

/// Automatic differentiation
pub mod ad;
/// Core types and utilities.
pub mod core;
/// Currency-related types and utilities.
pub mod currencies;
/// Indices module.
pub mod indices;
/// Financial instruments module.
pub mod instruments;
/// Market data module.
pub mod marketdata;
/// Mathematical functions and utilities.
pub mod math;
/// Financial models module.
pub mod models;
/// Prelude module for convenient imports.
pub mod prelude;
/// Pricer implementations.
pub mod pricers;
/// Interest rates module.
pub mod rates;
/// Time and date utilities.
pub mod time;
/// General utilities.
pub mod utils;

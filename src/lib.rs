//! `RustAtlas` is a Rust library for financial calculations and analysis.
//!
//! This library provides tools for working with asset-liability management,
//! cash flows, financial instruments, interest rates, and related computations.

/// Automatic differentiation
pub mod ad;
/// Cash flows module.
pub mod cashflows;
/// Core types and utilities.
pub mod core;
/// New Core
pub mod core2;
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
/// Interest rates module.
pub mod rates;
/// Time and date utilities.
pub mod time;
/// General utilities.
pub mod utils;
/// Visitor pattern implementations.
pub mod visitors;

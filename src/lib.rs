//! `QuantSupport` is a Rust library for financial calculations and analysis.
//!
//! This library provides tools for computing prices, sensitivities and other metrics of financial products.

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
/// Mathematical functions and utilities.
pub mod math;
/// Financial models module.
pub mod models;
/// Prelude module for convenient imports.
pub mod prelude;
/// Pricer implementations.
pub mod pricers;
/// Market data module.
pub mod quotes;
/// Interest rates module.
pub mod rates;
/// Simulation module.
pub mod simulation;
/// Time and date utilities.
pub mod time;
/// General utilities.
pub mod utils;
/// Volatility surface and cube definitions.
pub mod volatility;

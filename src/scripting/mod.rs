//! Payoff scripting language.
//!
//! Scripts are parsed into an event stream, indexed into the library's
//! market-data request types, and evaluated against paths produced by the
//! existing [`MarketModel`](crate::xva::visitors::marketmodel::MarketModel)
//! abstraction.

// The parser and AST intentionally favor explicit match arms and stateful
// visitors. Those patterns are clearer for this small interpreter even when
// they conflict with crate-wide style lints aimed at numerical kernels.
#![allow(clippy::cargo, clippy::nursery, clippy::pedantic)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

pub mod data;
pub mod nodes;
pub mod parsing;
pub mod product;
pub mod request;
pub mod runtime;
pub mod utils;
pub mod visitors;

/// Numeric representation used while evaluating scripts.
pub type NumericType = crate::ad::dual::DualFwd;

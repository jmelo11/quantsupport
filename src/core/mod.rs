//! Core types and utilities.
//!
//! Contains the pricing context ([`ContextManager`](crate::core::contextmanager::ContextManager)),
//! instrument and trade abstractions, evaluation results, market-data handling,
//! and collateral / CSA discount policy definitions.

/// Collateral / CSA discount policy module.
pub mod collateral;
/// Constructed element module.
pub mod elements;
/// Evaluation results module.
pub mod evaluationresults;
/// Trade evaluator that dispatches pricing requests to registered pricers.
pub mod evaluator;
/// Instrument module.
pub mod instrument;
/// Market data handling module.
pub mod marketdatahandling;
/// Pillars module.
pub mod pillars;
/// Pricer module.
pub mod pricer;
/// Pricer state module.
pub mod pricerstate;
/// Pricing data context module.
pub mod pricingcontext;
/// Pricing request module.
pub mod request;
/// Trade module.
pub mod trade;
/// Visitor pattern module.
pub mod visitable;

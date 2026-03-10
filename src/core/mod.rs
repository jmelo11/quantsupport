//! Core types and utilities.
//!
//! Contains the pricing context ([`ContextManager`](crate::core::contextmanager::ContextManager)),
//! instrument and trade abstractions, evaluation results, market-data handling,
//! and collateral / CSA discount policy definitions.

/// Collateral / CSA discount policy module.
pub mod collateral;
/// Pricing data context module.
pub mod contextmanager;
/// Constructed element module.
pub mod elements;
/// Evaluation results module.
pub mod evaluationresults;
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
/// Pricing request module.
pub mod request;
/// Trade module.
pub mod trade;
/// Visitor pattern module.
pub mod visitable;

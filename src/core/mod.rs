//! Core types and utilities.
//!
//! Contains the pricing context ([`PricingContext`](crate::core::pricingcontext::PricingContext)),
//! instrument and trade abstractions, evaluation results, market-data handling,
//! and collateral / CSA discount policy definitions.

/// Injectable cashflow aggregation and convention-based implied coupon rates.
pub mod cashflowaggregation;
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
/// Builder for streaming portfolio accumulators.
pub mod makeportfolioaccumulator;
/// Market data handling module.
pub mod marketdatahandling;
/// Serializable timestamped market inputs.
pub mod marketsnapshot;
/// Historical market snapshot archive.
pub mod marketsnapshotstore;
/// Pillars module.
pub mod pillars;
/// Streaming portfolio valuation and numeric aggregation.
pub mod portfolio;
/// Pricer module.
pub mod pricer;
/// Typed instrument-to-pricer configuration.
pub mod pricerconfiguration;
/// Pricer state module.
pub mod pricerstate;
/// Pricing data context module.
pub mod pricingcontext;
/// Pricing request module.
pub mod request;
/// Context-based resolution of reusable instrument definitions.
pub mod resolution;
/// Historical market timestamp and quote-side selection.
pub mod snapshotselector;
/// Trade module.
pub mod trade;
/// Visitor pattern module.
pub mod visitable;

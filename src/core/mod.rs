//! Core types and utilities.
//!
//! Contains the pricing context ([`PricingContext`](crate::core::pricingcontext::PricingContext)),
//! instrument and trade abstractions, evaluation results, market-data handling,
//! and collateral / CSA discount policy definitions.

/// Aggregates reported cashflows and calculates convention-based implied coupon rates.
pub mod cashflowaggregation;
/// Selects the curves and currencies used to discount collateralized positions.
pub mod collateral;
/// Wraps constructed curves, volatility data and simulations for market-data consumers.
pub mod elements;
/// Stores the values, cashflows and sensitivities returned by a pricing request.
pub mod evaluationresults;
/// Dispatches a trade and its requests to the registered pricing method.
pub mod evaluator;
/// Defines the common interface implemented by financial instruments.
pub mod instrument;
/// Defines how instrument makers resolve their terms against a market.
pub mod makeinstrument;
/// Builds streaming accumulators for portfolio valuation results.
pub mod makeportfolioaccumulator;
/// Describes, resolves and stores the market inputs required by pricing methods.
pub mod marketdatahandling;
/// Stores serializable market inputs at a specified valuation timestamp.
pub mod marketsnapshot;
/// Archives market snapshots for historical valuation.
pub mod marketsnapshotstore;
/// Registers labelled market inputs so derivatives can be reported in their quote coordinates.
pub mod pillars;
/// Values collections of trades and aggregates their numerical results.
pub mod portfolio;
/// Defines the pricing interface implemented by the supported valuation methods.
pub mod pricer;
/// Selects and configures a pricing method for an instrument type.
pub mod pricerconfiguration;
/// Stores the market inputs and intermediate values used during one valuation.
pub mod pricerstate;
/// Constructs and supplies the market dependencies required by a valuation.
pub mod pricingcontext;
/// Defines pricing requests and their handlers for values, cashflows, fair rates and risk.
pub mod request;
/// Changes the coordinates of reported curve risk while preserving curve values and prices.
pub mod riskcontext;
/// Selects the timestamp and quote side used for historical market observations.
pub mod snapshotselector;
/// Associates contractual instruments with position information such as side and notional.
pub mod trade;
/// Defines the visitor interfaces used to inspect supported financial objects.
pub mod visitable;

/// Constructs deterministic future markets as ordinary pricing-context inputs.
pub mod forwardmarket;

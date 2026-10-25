//! Pricer implementations.
//!
//! Cashflow-discounting and Monte Carlo pricers for
//! fixed-income, equity, FX, and rates instruments.

/// Cashflow discounting pricers.
pub mod cashflows;
/// Credit pricers.
pub mod credit;
/// Equity pricers
pub mod equity;
/// Fixed-income security pricers.
pub mod fixedincome;
/// FX pricers.
pub mod fx;
/// Rates pricers.
pub mod rates;
/// Streaming portfolio valuation and aggregation.
pub mod portfoliopricer;

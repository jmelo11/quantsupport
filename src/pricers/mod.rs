//! Pricer implementations.
//!
//! Cashflow-discounting and Monte Carlo pricers for
//! fixed-income, equity, FX, and rates instruments.

/// Cashflow discounting pricers.
pub mod cashflows;
/// Equity pricers
pub mod equity;
/// FX pricers.
pub mod fx;
/// Monte Carlo simulation engine and planner.
pub mod montecarloengine;
/// Available pricers
pub mod pricerdefinitions;
/// Rates pricers.
pub mod rates;

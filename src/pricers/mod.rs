//! Pricer implementations.
//!
//! Cashflow-discounting and Monte Carlo pricers for
//! fixed-income, equity, FX, and rates instruments.

/// Provides the cashflow discounting pricers.
pub mod cashflows;
/// Provides the credit pricers.
pub mod credit;
/// Provides the equity pricers.
pub mod equity;
/// Values bonds, deposits, notes and amortizing loans.
pub mod fixedincome;
/// Provides the FX pricers.
pub mod fx;
/// Streams trade valuations into portfolio accumulators.
pub mod portfoliopricer;
/// Provides the rates pricers.
pub mod rates;

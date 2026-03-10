//! Currency-related types and utilities.
//!
//! Defines the [`Currency`](crate::currencies::currency::Currency) enumeration, per-currency
//! detail traits, and an [`ExchangeRateStore`](crate::currencies::exchangeratestore::ExchangeRateStore)
//! for FX spot rates.

/// Currency enumeration types.
pub mod currency;
/// Trait definitions for currency operations.
pub mod currencydetails;
/// Exchange rate storage functionality.
pub mod exchangeratestore;

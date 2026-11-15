//! Currency-related types and utilities.
//!
//! Defines the [`Currency`](crate::currencies::currency::Currency) enumeration, per-currency
//! detail traits, and an [`FxStore`](crate::quotes::fxstore::FxStore)
//! for FX spot rates.

/// Provides the currency enumeration types.
pub mod currency;
/// Provides the trait definitions for currency operations.
pub mod currencydetails;

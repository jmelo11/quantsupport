//! Currency-related types and utilities.
//!
//! Defines the [`Currency`](crate::currencies::currency::Currency) enumeration, per-currency
//! detail traits, and an [`FxStore`](crate::quotes::fxstore::FxStore)
//! for FX spot rates.

/// Currency enumeration types.
pub mod currency;
/// Trait definitions for currency operations.
pub mod currencydetails;

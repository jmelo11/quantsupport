//! Market data: quotes and fixings.
//!
//! Provides [`Quote`](crate::quotes::quote::Quote) representations, a
//! [`QuoteStore`](crate::quotes::quotestore::QuoteStore) for live market data, and a
//! [`FixingStore`](crate::quotes::fixingstore::FixingStore) for historical fixings.

/// Provider of fixings
pub mod fixingstore;

/// Module for quote handeling.
pub mod quote;

/// Provider of market data
pub mod quotestore;

/// Exchange rate storage functionality.
pub mod fxstore;

/// Quote selection.
pub mod quoteselector;

/// Calibration instrument used in models and bootstrapping.
pub mod calibrationinstrument;

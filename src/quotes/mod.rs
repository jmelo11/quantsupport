//! Market data: quotes and fixings.
//!
//! Provides [`Quote`](crate::quotes::quote::Quote) representations, a
//! [`QuoteStore`](crate::quotes::quotestore::QuoteStore) for live market data, and a
//! [`FixingStore`](crate::quotes::fixingstore::FixingStore) for historical fixings.

/// Stores historical fixings by market index and observation date.
pub mod fixingstore;

/// Defines quote identifiers, quote levels and market-side selection rules.
pub mod quote;

/// Stores dated market quotes and resolves the requested quote side.
pub mod quotestore;

/// Stores exchange rates and resolves direct or triangulated currency conversions.
pub mod fxstore;

/// Defines how market consumers select quotes and their reference date.
pub mod quoteselector;

/// Applies quote shocks and combinations of shocks to a pricing context.
pub mod scenario;

/// Defines the quote-bearing instruments used by model and curve calibration.
pub mod calibrationinstrument;

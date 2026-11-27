//! Market index definitions.
//!
//! Provides the [`MarketIndex`](crate::indices::marketindex::MarketIndex) enumeration,
//! the [`FxPair`](crate::indices::fxpair::FxPair) value type for FX currency pairs,
//! rate-index trait definitions, and concrete implementations for
//! major overnight and term indices (SOFR, ESTR, EURIBOR, SONIA, etc.).

/// Defines typed currency pairs and their market-index representation.
pub mod fxpair;
/// Provides the unified market-index identifiers.
pub mod marketindex;
/// Provides the market quote-type identifiers.
pub mod quotetype;
/// Provides the interest-rate index traits and metadata.
pub mod rateindex;
/// Provides the concrete interest-rate index definitions.
pub mod rateindices;

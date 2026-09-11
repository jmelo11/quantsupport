//! Market index definitions.
//!
//! Provides the [`MarketIndex`](crate::indices::marketindex::MarketIndex) enumeration,
//! the [`FxPair`](crate::indices::fxpair::FxPair) value type for FX currency pairs,
//! rate-index trait definitions, and concrete implementations for
//! major overnight and term indices (SOFR, ESTR, EURIBOR, SONIA, etc.).

pub mod fxpair;
/// Unified market-index identifiers.
pub mod marketindex;
/// Market quote-type identifiers.
pub mod quotetype;
/// Interest-rate index traits and metadata.
pub mod rateindex;
/// Concrete interest-rate index definitions.
pub mod rateindices;

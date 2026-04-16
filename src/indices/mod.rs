//! Market index definitions.
//!
//! Provides the [`MarketIndex`](crate::indices::marketindex::MarketIndex) enumeration,
//! the [`FxPair`](crate::indices::fxpair::FxPair) value type for FX currency pairs,
//! rate-index trait definitions, and concrete implementations for
//! major overnight and term indices (SOFR, ESTR, EURIBOR, SONIA, etc.).

/// FX currency-pair type.
pub mod fxpair;
/// Base index module
pub mod marketindex;
/// Quote types module.
pub mod quotetype;
/// Interest rate indices module.
pub mod rateindex;
/// Implementations of different indices.
pub mod rateindices;

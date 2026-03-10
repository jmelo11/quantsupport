//! Market index definitions.
//!
//! Provides the [`MarketIndex`](crate::indices::marketindex::MarketIndex) enumeration,
//! rate-index trait definitions, and concrete implementations for
//! major overnight and term indices (SOFR, ESTR, EURIBOR, SONIA, etc.).

/// Base index module
pub mod marketindex;
/// Quote types module.
pub mod quotetype;
/// Interest rate indices module.
pub mod rateindex;
/// Implementations of different indices.
pub mod rateindices;

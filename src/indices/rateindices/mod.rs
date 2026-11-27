//! Concrete interest-rate index definitions.
//!
//! Each index uses the shared `rate_index!` declaration with named convention
//! fields and explicit imports. Observation frequency and rate compounding
//! frequency are specified independently; swap fixed-leg frequency may depend
//! on the swap tenor.

/// Generates consistent metadata and accessors for supported floating-rate indices.
mod macros;

/// Provides the AONIA (AUD) index implementation.
pub mod aonia;
/// Provides the BBSW (AUD) index implementation.
pub mod bbsw;
/// Provides the BKBM (NZD) index implementation.
pub mod bkbm;
/// Provides the CDI (BRL) index implementation.
pub mod cdi;
/// Provides the CORRA (CAD) index implementation.
pub mod corra;
/// Provides the DESTR (DKK) index implementation.
pub mod destr;
/// Provides the €STR (EUR) index implementation.
pub mod estr;
/// Provides the EURIBOR (EUR) index implementation.
pub mod euribor;
/// Provides the `FedFunds` (USD) index implementation.
pub mod fedfunds;
/// Provides the IBR (COP) index implementation.
pub mod ibr;
/// Provides the ICP (CLP) index implementation.
pub mod icp;
/// Provides the KOFR (KRW) index implementation.
pub mod kofr;
/// Provides the NIBOR (NOK) index implementation.
pub mod nibor;
/// Provides the NOWA (NOK) index implementation.
pub mod nowa;
/// Provides the NZONIA (NZD) index implementation.
pub mod nzonia;
/// Provides the SARON (CHF) index implementation.
pub mod saron;
/// Provides the SHIBOR (CNY) index implementation.
pub mod shibor;
/// Provides the SOFR (USD) index implementation.
pub mod sofr;
/// Provides the SONIA (GBP) index implementation.
pub mod sonia;
/// Provides the SWESTR (SEK) index implementation.
pub mod swestr;
/// Provides the TIBOR (JPY) index implementation.
pub mod tibor;
/// Provides the TONAR (JPY) index implementation.
pub mod tonar;
/// Provides the ZARONIA (ZAR) index implementation.
pub mod zaronia;

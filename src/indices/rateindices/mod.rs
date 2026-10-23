//! Concrete interest-rate index definitions.
//!
//! Each index uses the shared `rate_index!` declaration with named convention
//! fields and explicit imports. Observation frequency and rate compounding
//! frequency are specified independently; swap fixed-leg frequency may depend
//! on the swap tenor.

mod macros;

/// AONIA (AUD) index implementation.
pub mod aonia;
/// BBSW (AUD) index implementation.
pub mod bbsw;
/// BKBM (NZD) index implementation.
pub mod bkbm;
/// CDI (BRL) index implementation.
pub mod cdi;
/// CORRA (CAD) index implementation.
pub mod corra;
/// DESTR (DKK) index implementation.
pub mod destr;
/// €STR (EUR) index implementation.
pub mod estr;
/// EURIBOR (EUR) index implementation.
pub mod euribor;
/// `FedFunds` (USD) index implementation.
pub mod fedfunds;
/// ICP (CLP) index implementation.
pub mod icp;
/// KOFR (KRW) index implementation.
pub mod kofr;
/// NIBOR (NOK) index implementation.
pub mod nibor;
/// NOWA (NOK) index implementation.
pub mod nowa;
/// NZONIA (NZD) index implementation.
pub mod nzonia;
/// SARON (CHF) index implementation.
pub mod saron;
/// SHIBOR (CNY) index implementation.
pub mod shibor;
/// SOFR (USD) index implementation.
pub mod sofr;
/// SONIA (GBP) index implementation.
pub mod sonia;
/// SWESTR (SEK) index implementation.
pub mod swestr;
/// TIBOR (JPY) index implementation.
pub mod tibor;
/// TONAR (JPY) index implementation.
pub mod tonar;
/// ZARONIA (ZAR) index implementation.
pub mod zaronia;

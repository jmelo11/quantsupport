//! Reproducible fixtures and standalone comparison drivers for library benchmarks.

/// Calibrated market and configuration loading for the XVA benchmark fixture.
pub mod xva_fixture;

/// Calibrated portfolio pricing with prepared execution and current-market refreshes.
pub mod xvapricing;

/// Swap portfolio, simulation and full exposure-cube benchmark workload.
pub mod pfe_fixture;

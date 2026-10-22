//! Owned Excel objects, revisioned handles, and tabular results.

#[cfg_attr(not(windows), allow(dead_code))]
pub mod registry;

#[cfg_attr(not(windows), allow(dead_code))]
pub mod browser;

#[cfg_attr(not(windows), allow(dead_code))]
pub mod browser_queries;

#[cfg_attr(not(windows), allow(dead_code))]
pub mod historical;

#[cfg_attr(not(windows), allow(dead_code))]
pub mod market_inputs;

#[cfg_attr(not(windows), allow(dead_code))]
pub mod interpolation;

#[cfg_attr(not(windows), allow(dead_code))]
pub mod inputs;

#[cfg_attr(not(windows), allow(dead_code))]
pub mod loan_inputs;
#[cfg_attr(not(windows), allow(dead_code))]
pub mod loans;
#[cfg_attr(not(windows), allow(dead_code))]
pub mod output;
#[cfg_attr(not(windows), allow(dead_code))]
pub mod pricing;
#[cfg_attr(not(windows), allow(dead_code))]
pub mod portfolios;
#[cfg_attr(not(windows), allow(dead_code))]
pub mod products;
#[cfg_attr(not(windows), allow(dead_code))]
pub mod registration;

#[cfg_attr(not(windows), allow(dead_code, unused_imports))]
pub mod lifecycle;

#[cfg_attr(not(windows), allow(dead_code))]
pub mod configuration_inputs;

#[cfg(windows)]
pub mod arguments;

//! Pricers for quoted fixed-income securities.
//!
//! Unlike a generic discounted-cashflow pricer, a quoted bond pricer must
//! understand settlement, accrued interest, price normalization, yield, and
//! market spread anchors. The
//! [`bondpricer`](crate::pricers::fixedincome::bondpricer) module provides those semantics
//! for fixed-rate bonds.

/// Fixed-rate bond pricing from clean-price, dirty-price, or spread anchors.
pub mod bondpricer;

/// Callable-bond pricing with a recombining short-rate tree.
pub mod treebasedbondpricer;

/// Callable-bond pricing with backward Fourier evolution.
pub mod backwardfourierevolutionbondpricer;

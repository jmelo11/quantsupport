//! Pricers for quoted fixed-income securities.
//!
//! Unlike a generic discounted-cashflow pricer, a quoted bond pricer must
//! understand settlement, accrued interest, price normalization, yield, and
//! market spread anchors. Shared internal analytics provide those semantics
//! using the anchor and settlement terms stored on the trade. Each public
//! pricer module implements its own valuation engine.

/// Fixed-rate bond pricing from clean-price, dirty-price, or spread anchors.
pub mod bondpricer;

/// Callable-bond pricing with a recombining short-rate tree.
pub mod treebasedbondpricer;

/// Callable-bond pricing with backward Fourier evolution.
pub mod backwardfourierevolutionbondpricer;

mod bondanalytics;
mod bondevents;
mod callablebond;

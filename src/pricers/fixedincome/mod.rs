//! Pricers for quoted fixed-income securities.
//!
//! Unlike a generic discounted-cashflow pricer, a quoted bond pricer must
//! understand settlement, accrued interest, price normalization, yield, and
//! market spread anchors. Contract and trade terms live with the instruments;
//! the bond pricers share quote handling and numerical analytics. Each public
//! pricer implements the standard pricing interface and its valuation engine.

/// Fixed-rate bond pricing from clean-price, dirty-price, or spread anchors.
pub mod bondpricer;

/// Loan valuation using contractual amortization and market curves.
pub mod loanpricer;

/// Callable-bond pricing with a recombining short-rate tree.
pub mod treebasedbondpricer;

/// Callable-bond pricing with backward Fourier evolution.
pub mod backwardfourierevolutionbondpricer;

mod bondanalytics;
mod bondevents;
mod callablebond;

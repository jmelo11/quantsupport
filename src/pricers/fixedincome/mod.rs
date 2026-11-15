//! Pricers for quoted fixed-income securities.
//!
//! Unlike a generic discounted-cashflow pricer, a quoted bond pricer must
//! understand settlement, accrued interest, price normalization, yield, and
//! market spread anchors. Contract and trade terms live with the instruments;
//! the bond pricers share quote handling and numerical analytics. Each public
//! pricer implements the standard pricing interface and its valuation engine.

/// Values fixed-rate bonds from clean-price, dirty-price or spread anchors.
pub mod bondpricer;

/// Provides the loan valuation using contractual amortization and market curves.
pub mod loanpricer;

/// Provides the callable-bond pricing with a recombining short-rate tree.
pub mod treebasedbondpricer;

/// Provides the callable-bond pricing with backward Fourier evolution.
pub mod backwardfourierevolutionbondpricer;

/// Provides the yield, spread and duration calculations shared by the bond pricers.
mod bondanalytics;
/// Maps contractual cashflows and issuer-call decisions onto a bond-pricing time grid.
mod bondevents;
/// Prepares the model and exercise information shared by callable-bond pricing methods.
mod callablebond;

//! Geometric Brownian Motion model.

/// Generates Brownian states and lognormal spot paths for equity valuation.
pub mod brownianmotionmodel;
pub use brownianmotionmodel::BrownianMotion;

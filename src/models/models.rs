use crate::models::{brownianmotion::BrownianMotion, hullwhite::HullWhite};

/// Supported pricing-model variants.
pub enum Model {
    /// Geometric Brownian Motion (Black-Scholes) dynamics.
    Gbm(BrownianMotion<f64>),
    /// Shifted Hull-White one-factor short-rate model.
    HullWhite(HullWhite<f64>),
}

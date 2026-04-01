use crate::models::{geometricbrownianmotion::GeometricBrownianMotion, hullwhite::HullWhite};

/// Supported pricing-model variants.
pub enum Model {
    /// Geometric Brownian Motion (Black-Scholes) dynamics.
    Gbm(GeometricBrownianMotion<f64>),
    /// Shifted Hull-White one-factor short-rate model.
    HullWhite(HullWhite<f64>),
}

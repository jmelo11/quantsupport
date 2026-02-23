use rand::{rngs::StdRng, Rng, SeedableRng};
use std::f64::consts::PI;

/// # `ModelParameters`
///
/// A tagged union of per-model parameter sets. Stored as a `Vec<ModelParameters>` in
/// `ContextManager`, `MarketDataRequest`, and `MarketData` so that multiple model
/// configurations can coexist and providers can inspect them at request time.
#[derive(Clone, Debug)]
pub enum ModelParameters {
    /// Parameters for the GBM (Black-Scholes) Monte Carlo model.
    Gbm(GbmModelParameters),
    /// Placeholder for Hull-White short-rate model parameters (to be extended).
    HullWhite,
}

/// # `GbmModelParameters`
///
/// Parameters for the Geometric Brownian Motion (GBM) model used in Monte Carlo simulation.
/// Specifies the number of simulation paths and the random seed for reproducibility.
#[derive(Clone, Copy, Debug)]
pub struct GbmModelParameters {
    /// Number of simulation paths.
    n_paths: usize,
    /// Random seed for reproducible path generation.
    seed: u64,
}

impl Default for GbmModelParameters {
    fn default() -> Self {
        Self {
            n_paths: 10_000,
            seed: 0,
        }
    }
}

impl GbmModelParameters {
    /// Creates a new [`GbmModelParameters`] with the given number of paths and seed.
    #[must_use]
    pub const fn new(n_paths: usize, seed: u64) -> Self {
        Self { n_paths, seed }
    }

    /// Returns the number of simulation paths.
    #[must_use]
    pub const fn n_paths(&self) -> usize {
        self.n_paths
    }

    /// Returns the random seed.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Generates `n_paths` standard-normal draws using a Box-Muller transform
    /// seeded with `self.seed`.
    #[must_use]
    pub fn generate_draws(&self) -> Vec<f64> {
        let mut rng = StdRng::seed_from_u64(self.seed);
        let mut draws = Vec::with_capacity(self.n_paths);
        while draws.len() < self.n_paths {
            let u1: f64 = rng.gen::<f64>().max(f64::EPSILON);
            let u2: f64 = rng.gen::<f64>();
            let r = (-2.0 * u1.ln()).sqrt();
            let theta = 2.0 * PI * u2;
            draws.push(r * theta.cos());
            if draws.len() < self.n_paths {
                draws.push(r * theta.sin());
            }
        }
        draws
    }
}

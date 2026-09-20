//! Serde-enabled model and simulation configurations.
//!
//! [`ModelConfiguration`] selects the stochastic dynamics and describes how
//! the model obtains its parameters. [`SimulationConfiguration`] combines that
//! model definition with the path count, random seed, horizon, and time-step
//! frequency used by
//! [`SimulationBuilder`](crate::simulations::simulationbuilder::SimulationBuilder)
//! during [`PricingContext::initialize`](crate::core::pricingcontext::PricingContext::initialize).
//!
//! ## JSON example
//! ```json
//! {
//!     "market_index": "SOFR",
//!     "model": {
//!         "HullWhite": {
//!             "alpha": 0.1,
//!             "parameter_source": {
//!                 "Calibrated": {
//!                     "source": { "Surface": { "market_index": "SOFR" } },
//!                     "calibration_basket": { "strike": "Atm" }
//!                 }
//!             }
//!         }
//!     },
//!     "n_paths": 1000,
//!     "seed": 42,
//!     "horizon": "5Y",
//!     "frequency": "Monthly"
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::{
    indices::marketindex::MarketIndex,
    time::{daycounter::DayCounter, enums::Frequency, period::Period},
    utils::errors::{QSError, Result},
    volatility::modelcalibration::ModelCalibrationConfiguration,
};

/// Describes how a stochastic model obtains its parameters.
///
/// `Fixed` carries a fully specified parameter set. `Calibrated` carries a
/// market target that the model's calibrator converts into a parameter set.
/// The type parameters preserve each model's natural representation. For
/// example, a one-factor Gaussian rate model uses a scalar short-rate
/// volatility, while an HJM model can use factor-loading functions and a
/// correlation matrix.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ParameterSource<P, C> {
    /// Uses the supplied parameters when the model is constructed.
    Fixed(P),
    /// Fits the model parameters to the supplied market target when the model
    /// is constructed.
    Calibrated(C),
}

/// Fixed parameters for one-factor Gaussian rate models.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GaussianRateModelParameters {
    /// Annualized absolute short-rate volatility in rate units per square-root
    /// year. `0.01` means 100 bp/√year.
    pub sigma: f64,
}

impl GaussianRateModelParameters {
    /// Creates fixed one-factor Gaussian parameters.
    #[must_use]
    pub const fn new(sigma: f64) -> Self {
        Self { sigma }
    }

    /// Validates that sigma is finite and non-negative.
    ///
    /// # Errors
    /// Returns an error for negative, infinite, or NaN volatility.
    pub fn validate(&self) -> Result<()> {
        if self.sigma.is_finite() && self.sigma >= 0.0 {
            Ok(())
        } else {
            Err(QSError::InvalidValueErr(format!(
                "Gaussian short-rate sigma must be finite and non-negative, got {}",
                self.sigma
            )))
        }
    }
}

/// Fixed or calibrated parameters for Hull-White and one-factor LGM.
pub type GaussianRateParameterSource =
    ParameterSource<GaussianRateModelParameters, ModelCalibrationConfiguration>;

/// Fixed parameters for a lognormal Brownian-motion model.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LognormalModelParameters {
    /// Annualized lognormal volatility as a decimal. `0.20` means 20% per
    /// square-root year.
    pub volatility: f64,
}

impl LognormalModelParameters {
    /// Creates fixed lognormal model parameters.
    #[must_use]
    pub const fn new(volatility: f64) -> Self {
        Self { volatility }
    }

    /// Validates that volatility is finite and non-negative.
    ///
    /// # Errors
    /// Returns an error for negative, infinite, or NaN volatility.
    pub fn validate(&self) -> Result<()> {
        if self.volatility.is_finite() && self.volatility >= 0.0 {
            Ok(())
        } else {
            Err(QSError::InvalidValueErr(format!(
                "Lognormal volatility must be finite and non-negative, got {}",
                self.volatility
            )))
        }
    }
}

/// Fixed or calibrated parameters for lognormal Brownian motion.
pub type LognormalParameterSource =
    ParameterSource<LognormalModelParameters, ModelCalibrationConfiguration>;

/// Configures the stochastic dynamics used to generate simulation paths.
///
/// Hull-White and LGM resolve `GaussianRateModelParameters` from a fixed value
/// or from caplet and swaption calibration. Brownian motion resolves
/// `LognormalModelParameters` from a fixed value or strips a
/// piecewise-constant forward-volatility curve from Black implied
/// volatilities. The stripping algorithm reproduces total variance at each
/// selected expiry and is implemented by
/// [`bootstrap_black_term_volatility`](crate::volatility::volatilitysource::bootstrap_black_term_volatility).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ModelConfiguration {
    /// Hull-White one-factor short-rate model anchored to the constructed
    /// discount curve for the simulation's market index.
    HullWhite {
        /// Mean-reversion speed.
        alpha: f64,
        /// Short-rate volatility supplied directly or fitted to option prices.
        parameter_source: GaussianRateParameterSource,
    },
    /// Geometric Brownian motion using the reference-date fixing as spot and
    /// the constructed discount curve as the risk-neutral drift input.
    BrownianMotion {
        /// Lognormal volatility supplied directly or stripped from a Black
        /// volatility market.
        parameter_source: LognormalParameterSource,
        /// Optional continuous dividend rate.
        #[serde(default)]
        dividend_rate: Option<f64>,
    },
    /// One-factor Linear Gaussian Markov rate model fitted to the initial
    /// discount curve.
    ///
    /// This uses the convention documented by
    /// [`LgmRateModel`](crate::models::lgm::lgmcomponents::LgmRateModel):
    /// `H(t) = (1 - exp(-lambda * t)) / lambda`, with a Gaussian state whose
    /// diffusion is derived from the configured short-rate volatility.
    Lgm {
        /// Mean-reversion speed in inverse years. Larger positive values
        /// reduce the effect of a factor shock on distant maturities. Zero is
        /// supported and selects the non-mean-reverting limit.
        lambda: f64,
        /// Short-rate volatility supplied directly or fitted to option prices.
        parameter_source: GaussianRateParameterSource,
    },
}

const fn default_n_paths() -> usize {
    1000
}

const fn default_seed() -> u64 {
    42
}

const fn default_frequency() -> Frequency {
    Frequency::Monthly
}

const fn default_day_counter() -> DayCounter {
    DayCounter::Actual365
}

/// Configuration for a Monte Carlo simulation driven by a [`ModelConfiguration`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationConfiguration {
    /// Market index under which the simulation is stored (and whose curve /
    /// fixings feed the model).
    market_index: MarketIndex,
    /// The model driving the paths.
    model: ModelConfiguration,
    /// Number of Monte Carlo paths.
    #[serde(default = "default_n_paths")]
    n_paths: usize,
    /// RNG seed.
    #[serde(default = "default_seed")]
    seed: u64,
    /// Simulation horizon from the reference date.
    horizon: Period,
    /// Time-step frequency of the simulation date grid.
    #[serde(default = "default_frequency")]
    frequency: Frequency,
    /// Day counter used to convert simulation dates into year fractions.
    #[serde(default = "default_day_counter")]
    day_counter: DayCounter,
}

impl SimulationConfiguration {
    /// Creates a new simulation configuration.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        model: ModelConfiguration,
        n_paths: usize,
        seed: u64,
        horizon: Period,
        frequency: Frequency,
    ) -> Self {
        Self {
            market_index,
            model,
            n_paths,
            seed,
            horizon,
            frequency,
            day_counter: DayCounter::Actual365,
        }
    }

    /// Returns the market index under which the simulation is stored.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the model configuration.
    #[must_use]
    pub const fn model(&self) -> &ModelConfiguration {
        &self.model
    }

    /// Returns the number of Monte Carlo paths.
    #[must_use]
    pub const fn n_paths(&self) -> usize {
        self.n_paths
    }

    /// Returns the RNG seed.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the simulation horizon.
    #[must_use]
    pub const fn horizon(&self) -> Period {
        self.horizon
    }

    /// Returns the time-step frequency.
    #[must_use]
    pub const fn frequency(&self) -> Frequency {
        self.frequency
    }

    /// Returns the day counter used for year fractions.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }
}

//! Serde-enabled model and simulation configurations.
//!
//! [`ModelConfiguration`] describes a stochastic model and how it sources its
//! volatility (constant, surface/cube-sampled, or calibrated).
//! [`SimulationConfiguration`] pairs a model with a Monte Carlo setup (paths,
//! seed, horizon, frequency) and is consumed by
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
//!             "volatility": {
//!                 "Calibrated": {
//!                     "source": { "Surface": { "market_index": "SOFR" } },
//!                     "quote_ids": ["CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black"],
//!                     "alpha": 0.1
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
    volatility::volatilitysource::VolatilitySourceConfiguration,
};

/// Describes a stochastic model and its volatility source.
///
/// Supported volatility sources per model:
///
/// | Model            | `Constant` | `Surface`/`Cube` | `Calibrated` |
/// |------------------|------------|------------------|--------------|
/// | `HullWhite`      | yes        | no               | yes          |
/// | `BrownianMotion` | yes        | yes              | yes          |
/// | `Lgm`            | yes        | no               | yes          |
///
/// For `HullWhite` and `Lgm`, `Calibrated` bootstraps a piecewise-constant
/// short-rate volatility to the configured caplet/swaption vols. For
/// `BrownianMotion`, `Calibrated` bootstraps a piecewise-constant forward
/// volatility that reproduces the Black total variance at each quoted expiry
/// (see
/// [`bootstrap_black_term_volatility`](crate::volatility::volatilitysource::bootstrap_black_term_volatility)).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ModelConfiguration {
    /// Hull-White one-factor short-rate model. The discount curve is taken
    /// from the constructed curve for the simulation's market index.
    HullWhite {
        /// Mean-reversion speed.
        alpha: f64,
        /// Short-rate volatility source (`Constant` or `Calibrated`).
        volatility: VolatilitySourceConfiguration,
    },
    /// Geometric Brownian motion (Black-Scholes). The spot is read from the
    /// fixing store at the reference date and the drift from the constructed
    /// discount curve for the simulation's market index.
    BrownianMotion {
        /// Volatility source (`Constant`, `Surface`, `Cube`, or `Calibrated`).
        volatility: VolatilitySourceConfiguration,
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
        /// Absolute short-rate volatility source. `Constant { value: 0.01 }`
        /// means 100 bp per square-root year. `Calibrated` solves a
        /// piecewise-constant short-rate volatility schedule against caplet or
        /// swaption prices. `Surface` and `Cube` quotes enter through the
        /// calibration instruments and produce model sigma values.
        volatility: VolatilitySourceConfiguration,
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

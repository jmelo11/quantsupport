//! Typed pricing-engine configuration.
//!
//! A [`PricerConfiguration`](crate::core::pricerconfiguration::PricerConfiguration)
//! selects one implementation for an
//! [`InstrumentType`](crate::core::pricerconfiguration::InstrumentType) and
//! carries any stochastic models required by that
//! implementation. The configuration belongs to
//! [`PricingContext`](crate::core::pricingcontext::PricingContext), rather
//! than to an individual pricer, so the same JSON mechanism can configure all
//! instrument families.
//!
//! Models are represented by
//! [`ModelConfiguration`](crate::models::modelconfiguration::ModelConfiguration)
//! values, not free-form names. A model is assigned to a typed
//! [`MarketIndex`](crate::indices::marketindex::MarketIndex); a pricer therefore
//! resolves the model from the same index it uses to resolve its market curve.
//!
//! # Callable-bond JSON example
//!
//! ```json
//! {
//!   "instrument_type": "SteppedCouponBond",
//!   "pricer": {
//!     "TreeBased": {
//!       "time_grid": {
//!         "steps_per_year": 96,
//!         "max_steps": 4096,
//!         "day_counter": "Actual365"
//!       }
//!     }
//!   },
//!   "models": [{
//!     "market_index": "SOFR",
//!     "model": {
//!       "HullWhite": {
//!         "alpha": 0.05,
//!         "parameter_source": { "Fixed": { "sigma": 0.01 } }
//!       }
//!     }
//!   }]
//! }
//! ```

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    indices::marketindex::MarketIndex,
    models::modelconfiguration::ModelConfiguration,
    time::daycounter::DayCounter,
    utils::errors::{QSError, Result},
};

const fn default_steps_per_year() -> usize {
    48
}

const fn default_max_steps() -> usize {
    2048
}

const fn default_day_counter() -> DayCounter {
    DayCounter::Actual365
}

const fn default_fourier_grid_points() -> usize {
    1024
}

const fn default_fourier_standard_deviations() -> f64 {
    10.0
}

/// Stable instrument-family key used by pricing configuration lookup.
///
/// This enum deliberately names concrete contractual instrument types instead
/// of broad asset classes.  Two instruments in the same asset class can need
/// different engines and model inputs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum InstrumentType {
    /// Fixed-rate debt security.
    FixedRateBond,
    /// Debt security with stepped fixed/floating coupons and issuer calls.
    SteppedCouponBond,
    /// Floating-rate debt security.
    FloatingRateNote,
    /// Fixed-rate deposit.
    FixedRateDeposit,
    /// Fixed-versus-floating interest-rate swap.
    Swap,
    /// Floating-versus-floating basis swap.
    BasisSwap,
    /// Floating-versus-floating cross-currency swap.
    FloatFloatCrossCurrencySwap,
    /// Fixed-versus-floating cross-currency swap.
    FixFloatCrossCurrencySwap,
    /// Foreign-exchange forward.
    FxForward,
    /// European foreign-exchange option.
    FxEuropeanOption,
    /// European equity option.
    EquityEuropeanOption,
    /// Credit default swap.
    CreditDefaultSwap,
    /// Cap or floor.
    CapFloor,
    /// Caplet or floorlet.
    CapletFloorlet,
    /// European swaption.
    EuropeanSwaption,
    /// Short-rate future.
    RateFutures,
}

/// Uniform contractual-time grid shared by the callable-bond engines.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeGridConfiguration {
    /// Target number of time intervals per year.
    #[serde(default = "default_steps_per_year")]
    steps_per_year: usize,
    /// Hard upper bound on the total number of time intervals.
    #[serde(default = "default_max_steps")]
    max_steps: usize,
    /// Day counter mapping contractual dates to model time.
    #[serde(default = "default_day_counter")]
    day_counter: DayCounter,
}

impl TimeGridConfiguration {
    /// Creates a uniform time-grid configuration.
    #[must_use]
    pub const fn new(steps_per_year: usize, max_steps: usize, day_counter: DayCounter) -> Self {
        Self {
            steps_per_year,
            max_steps,
            day_counter,
        }
    }

    /// Returns the target time-step density.
    #[must_use]
    pub const fn steps_per_year(&self) -> usize {
        self.steps_per_year
    }

    /// Returns the maximum total number of time steps.
    #[must_use]
    pub const fn max_steps(&self) -> usize {
        self.max_steps
    }

    /// Returns the day counter used to calculate model times.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    /// Checks that the grid contains at least one admissible time step.
    ///
    /// # Errors
    /// Returns an error when either step count is zero.
    pub fn validate(&self) -> Result<()> {
        if self.steps_per_year == 0 || self.max_steps == 0 {
            return Err(QSError::InvalidValueErr(
                "Time-grid steps_per_year and max_steps must be positive".into(),
            ));
        }
        Ok(())
    }
}

impl Default for TimeGridConfiguration {
    fn default() -> Self {
        Self::new(
            default_steps_per_year(),
            default_max_steps(),
            default_day_counter(),
        )
    }
}

/// Spatial grid used by backward Fourier evolution.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FourierGridConfiguration {
    /// Number of equally spaced state points. It must be a power of two.
    #[serde(default = "default_fourier_grid_points")]
    points: usize,
    /// Half-width of the state domain, expressed in terminal standard
    /// deviations of the Gaussian state.
    #[serde(default = "default_fourier_standard_deviations")]
    standard_deviations: f64,
}

impl FourierGridConfiguration {
    /// Creates Fourier state-grid controls.
    #[must_use]
    pub const fn new(points: usize, standard_deviations: f64) -> Self {
        Self {
            points,
            standard_deviations,
        }
    }

    /// Returns the number of FFT state points.
    #[must_use]
    pub const fn points(&self) -> usize {
        self.points
    }

    /// Returns the Gaussian-domain half-width multiplier.
    #[must_use]
    pub const fn standard_deviations(&self) -> f64 {
        self.standard_deviations
    }

    /// Checks the FFT and truncation-domain requirements.
    ///
    /// # Errors
    /// Returns an error unless `points` is a power of two with at least eight
    /// nodes, or when the domain multiplier is non-finite or non-positive.
    pub fn validate(&self) -> Result<()> {
        if self.points < 8 || !self.points.is_power_of_two() {
            return Err(QSError::InvalidValueErr(
                "Fourier grid points must be a power of two and at least 8".into(),
            ));
        }
        if !self.standard_deviations.is_finite() || self.standard_deviations <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Fourier grid standard_deviations must be finite and positive".into(),
            ));
        }
        Ok(())
    }
}

impl Default for FourierGridConfiguration {
    fn default() -> Self {
        Self::new(
            default_fourier_grid_points(),
            default_fourier_standard_deviations(),
        )
    }
}

/// A typed model associated with the market index that supplies its curve.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingModelConfiguration {
    /// Market index used to select this model and its input curve.
    market_index: MarketIndex,
    /// Typed stochastic model definition.
    model: ModelConfiguration,
}

impl PricingModelConfiguration {
    /// Creates a typed model assignment.
    #[must_use]
    pub const fn new(market_index: MarketIndex, model: ModelConfiguration) -> Self {
        Self {
            market_index,
            model,
        }
    }

    /// Returns the model's market-index key.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the typed model definition.
    #[must_use]
    pub const fn model(&self) -> &ModelConfiguration {
        &self.model
    }
}

/// Concrete pricing implementation selected for an instrument family.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum PricerImplementation {
    /// Directly discounts contractual cashflows.
    DiscountedCashflow,
    /// Deterministic straight-bond price, yield, duration, and Z-spread
    /// valuation.
    Bond,
    /// Recombining trinomial-tree backward induction.
    TreeBased {
        /// Contractual-time discretisation.
        #[serde(default)]
        time_grid: TimeGridConfiguration,
    },
    /// Backward evolution on a uniform state grid using FFT Gaussian
    /// convolution at every time step.
    BackwardFourierEvolution {
        /// Contractual-time discretisation.
        #[serde(default)]
        time_grid: TimeGridConfiguration,
        /// State-space and FFT controls.
        #[serde(default)]
        fourier_grid: FourierGridConfiguration,
    },
    /// Closed-form Black-family option valuation.
    BlackClosedForm,
    /// Monte Carlo Black-family option valuation.
    BlackMonteCarlo,
    /// Closed-form Hull-White option valuation.
    HullWhiteClosedForm,
    /// Dedicated FX-forward valuation.
    FxForward,
    /// Dedicated credit-default-swap valuation.
    CreditDefaultSwap,
    /// Dedicated short-rate-futures valuation.
    RateFutures,
}

/// Selects the pricer and typed model assignments for one instrument type.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricerConfiguration {
    /// Instrument family to which this configuration applies.
    instrument_type: InstrumentType,
    /// Numerical or analytic implementation to use.
    pricer: PricerImplementation,
    /// Typed model assignments, indexed by market index.
    #[serde(default)]
    models: Vec<PricingModelConfiguration>,
}

impl PricerConfiguration {
    /// Creates an instrument-level pricer configuration.
    #[must_use]
    pub const fn new(
        instrument_type: InstrumentType,
        pricer: PricerImplementation,
        models: Vec<PricingModelConfiguration>,
    ) -> Self {
        Self {
            instrument_type,
            pricer,
            models,
        }
    }

    /// Returns the configured instrument family.
    #[must_use]
    pub const fn instrument_type(&self) -> InstrumentType {
        self.instrument_type
    }

    /// Returns the selected pricing implementation.
    #[must_use]
    pub const fn pricer(&self) -> &PricerImplementation {
        &self.pricer
    }

    /// Returns all model assignments used by this pricer.
    #[must_use]
    pub const fn models(&self) -> &Vec<PricingModelConfiguration> {
        &self.models
    }

    /// Resolves a typed model by its market index.
    #[must_use]
    pub fn model(&self, market_index: &MarketIndex) -> Option<&ModelConfiguration> {
        self.models
            .iter()
            .find(|entry| entry.market_index() == market_index)
            .map(PricingModelConfiguration::model)
    }

    /// Validates the instrument/pricer pairing, numerical controls, and
    /// uniqueness of model-index assignments.
    ///
    /// # Errors
    /// Returns an error for an incompatible implementation, invalid numerical
    /// grid, or duplicate model assignment.
    pub fn validate(&self) -> Result<()> {
        let compatible = match self.instrument_type {
            InstrumentType::SteppedCouponBond => matches!(
                self.pricer,
                PricerImplementation::TreeBased { .. }
                    | PricerImplementation::BackwardFourierEvolution { .. }
            ),
            InstrumentType::FixedRateBond => matches!(self.pricer, PricerImplementation::Bond),
            InstrumentType::FloatingRateNote
            | InstrumentType::FixedRateDeposit
            | InstrumentType::Swap
            | InstrumentType::BasisSwap
            | InstrumentType::FloatFloatCrossCurrencySwap
            | InstrumentType::FixFloatCrossCurrencySwap => {
                matches!(self.pricer, PricerImplementation::DiscountedCashflow)
            }
            InstrumentType::FxForward => {
                matches!(self.pricer, PricerImplementation::FxForward)
            }
            InstrumentType::FxEuropeanOption
            | InstrumentType::EquityEuropeanOption
            | InstrumentType::CapFloor
            | InstrumentType::CapletFloorlet => matches!(
                self.pricer,
                PricerImplementation::BlackClosedForm
                    | PricerImplementation::BlackMonteCarlo
                    | PricerImplementation::HullWhiteClosedForm
            ),
            InstrumentType::EuropeanSwaption => matches!(
                self.pricer,
                PricerImplementation::BlackClosedForm | PricerImplementation::HullWhiteClosedForm
            ),
            InstrumentType::CreditDefaultSwap => {
                matches!(self.pricer, PricerImplementation::CreditDefaultSwap)
            }
            InstrumentType::RateFutures => {
                matches!(self.pricer, PricerImplementation::RateFutures)
            }
        };
        if !compatible {
            return Err(QSError::InvalidValueErr(format!(
                "Pricer {:?} is incompatible with instrument type {:?}",
                self.pricer, self.instrument_type
            )));
        }

        match self.pricer {
            PricerImplementation::TreeBased { time_grid } => time_grid.validate()?,
            PricerImplementation::BackwardFourierEvolution {
                time_grid,
                fourier_grid,
            } => {
                time_grid.validate()?;
                fourier_grid.validate()?;
            }
            PricerImplementation::DiscountedCashflow
            | PricerImplementation::Bond
            | PricerImplementation::BlackClosedForm
            | PricerImplementation::BlackMonteCarlo
            | PricerImplementation::HullWhiteClosedForm
            | PricerImplementation::FxForward
            | PricerImplementation::CreditDefaultSwap
            | PricerImplementation::RateFutures => {}
        }

        let mut indices = HashSet::new();
        for model in &self.models {
            if !indices.insert(model.market_index().clone()) {
                return Err(QSError::InvalidValueErr(format!(
                    "Duplicate model assignment for index {}",
                    model.market_index()
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_tree_pricer_with_typed_hull_white_model() {
        let json = r#"{
            "instrument_type": "SteppedCouponBond",
            "pricer": { "TreeBased": { "time_grid": { "steps_per_year": 96 } } },
            "models": [{
                "market_index": "SOFR",
                "model": {
                    "HullWhite": {
                        "alpha": 0.03,
                        "parameter_source": { "Fixed": { "sigma": 0.01 } }
                    }
                }
            }]
        }"#;
        let configuration: PricerConfiguration =
            serde_json::from_str(json).unwrap_or_else(|error| panic!("valid JSON: {error}"));
        configuration
            .validate()
            .unwrap_or_else(|error| panic!("valid configuration: {error}"));
        assert_eq!(
            configuration.instrument_type(),
            InstrumentType::SteppedCouponBond
        );
        assert!(configuration.model(&MarketIndex::SOFR).is_some());
    }

    #[test]
    fn rejects_incompatible_instrument_and_pricer() {
        let configuration = PricerConfiguration::new(
            InstrumentType::Swap,
            PricerImplementation::TreeBased {
                time_grid: TimeGridConfiguration::default(),
            },
            Vec::new(),
        );
        assert!(configuration.validate().is_err());
    }

    #[test]
    fn straight_bond_uses_its_dedicated_pricer_configuration() {
        let valid = PricerConfiguration::new(
            InstrumentType::FixedRateBond,
            PricerImplementation::Bond,
            Vec::new(),
        );
        valid
            .validate()
            .unwrap_or_else(|error| panic!("valid bond configuration: {error}"));

        let generic_cashflows = PricerConfiguration::new(
            InstrumentType::FixedRateBond,
            PricerImplementation::DiscountedCashflow,
            Vec::new(),
        );
        assert!(generic_cashflows.validate().is_err());
    }
}

//! Typed model configuration for exercise-aware bond pricing.
//!
//! A bond lattice is deliberately configured separately from a Monte Carlo
//! simulation. Both reuse [`ModelConfiguration`] for the stochastic dynamics,
//! while this module owns numerical controls specific to a short-rate lattice.

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

/// Numerical controls for the recombining short-rate lattice.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShortRateLatticeConfiguration {
    /// Target number of time steps per year.
    #[serde(default = "default_steps_per_year")]
    steps_per_year: usize,
    /// Hard upper bound on the total number of time steps.
    #[serde(default = "default_max_steps")]
    max_steps: usize,
    /// Day counter used to map contractual dates to lattice times.
    #[serde(default = "default_day_counter")]
    day_counter: DayCounter,
}

impl ShortRateLatticeConfiguration {
    /// Creates lattice controls.
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

    /// Returns the maximum number of time steps.
    #[must_use]
    pub const fn max_steps(&self) -> usize {
        self.max_steps
    }

    /// Returns the day counter used by the lattice.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    /// Validates the numerical controls.
    ///
    /// # Errors
    /// Returns an error when either step count is zero.
    pub fn validate(&self) -> Result<()> {
        if self.steps_per_year == 0 {
            return Err(QSError::InvalidValueErr(
                "Bond lattice steps_per_year must be positive".into(),
            ));
        }
        if self.max_steps == 0 {
            return Err(QSError::InvalidValueErr(
                "Bond lattice max_steps must be positive".into(),
            ));
        }
        Ok(())
    }
}

impl Default for ShortRateLatticeConfiguration {
    fn default() -> Self {
        Self::new(
            default_steps_per_year(),
            default_max_steps(),
            default_day_counter(),
        )
    }
}

/// Associates a typed short-rate model and lattice with a market index.
///
/// The configuration is serde-enabled and intended to be loaded from the
/// same JSON configuration layer as curves, volatility objects, and
/// simulations. The initial lattice implementation supports the `HullWhite`
/// variant with fixed Gaussian volatility parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BondModelConfiguration {
    /// Discount-curve identity and lookup key for the model.
    market_index: MarketIndex,
    /// Typed stochastic model definition.
    model: ModelConfiguration,
    /// Lattice numerical controls.
    #[serde(default)]
    lattice: ShortRateLatticeConfiguration,
}

impl BondModelConfiguration {
    /// Creates a bond-model configuration.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        model: ModelConfiguration,
        lattice: ShortRateLatticeConfiguration,
    ) -> Self {
        Self {
            market_index,
            model,
            lattice,
        }
    }

    /// Returns the market index used to select the configuration.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the typed stochastic model definition.
    #[must_use]
    pub const fn model(&self) -> &ModelConfiguration {
        &self.model
    }

    /// Returns the lattice controls.
    #[must_use]
    pub const fn lattice(&self) -> ShortRateLatticeConfiguration {
        self.lattice
    }

    /// Validates the configuration and supported model domain.
    ///
    /// # Errors
    /// Returns an error for invalid lattice controls, invalid short-rate
    /// mean reversion, or a model family that is not a short-rate model.
    pub fn validate(&self) -> Result<()> {
        self.lattice.validate()?;
        match &self.model {
            ModelConfiguration::HullWhite { alpha, .. } => {
                if !alpha.is_finite() || *alpha < 0.0 {
                    return Err(QSError::InvalidValueErr(format!(
                        "Hull-White alpha must be finite and non-negative, got {alpha}"
                    )));
                }
            }
            ModelConfiguration::Lgm { lambda, .. } => {
                if !lambda.is_finite() || *lambda < 0.0 {
                    return Err(QSError::InvalidValueErr(format!(
                        "LGM lambda must be finite and non-negative, got {lambda}"
                    )));
                }
            }
            ModelConfiguration::BrownianMotion { .. } => {
                return Err(QSError::InvalidValueErr(
                    "BrownianMotion is not a short-rate bond model".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::modelconfiguration::{
        GaussianRateModelParameters, LognormalModelParameters, ParameterSource,
    };

    #[test]
    fn json_uses_typed_model_variant_and_lattice_defaults() {
        let json = r#"{
            "market_index": "SOFR",
            "model": {
                "HullWhite": {
                    "alpha": 0.03,
                    "parameter_source": { "Fixed": { "sigma": 0.01 } }
                }
            }
        }"#;
        let configuration: BondModelConfiguration =
            serde_json::from_str(json).unwrap_or_else(|error| panic!("valid JSON: {error}"));
        assert_eq!(configuration.market_index(), &MarketIndex::SOFR);
        assert_eq!(configuration.lattice().steps_per_year(), 48);
        configuration
            .validate()
            .unwrap_or_else(|error| panic!("valid configuration: {error}"));
    }

    #[test]
    fn rejects_equity_model() {
        let configuration = BondModelConfiguration::new(
            MarketIndex::SOFR,
            ModelConfiguration::BrownianMotion {
                parameter_source: ParameterSource::Fixed(LognormalModelParameters::new(0.2)),
                dividend_rate: None,
            },
            ShortRateLatticeConfiguration::default(),
        );
        assert!(configuration.validate().is_err());

        let valid = BondModelConfiguration::new(
            MarketIndex::SOFR,
            ModelConfiguration::HullWhite {
                alpha: 0.03,
                parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(0.01)),
            },
            ShortRateLatticeConfiguration::default(),
        );
        assert!(valid.validate().is_ok());
    }
}

//! Credit curve (hazard-rate) configuration.
//!
//! A [`CreditCurveConfiguration`] describes how to bootstrap a survival
//! curve for a single reference entity from its CDS par-spread quotes.
//! The struct is serde-deserializable so it can be loaded straight from
//! JSON configuration files:
//!
//! ```json
//! {
//!     "market_index": "Credit(SOMECORP)",
//!     "currency": "USD",
//!     "discount_index": "SOFR",
//!     "recovery": 0.4,
//!     "quotes": ["Cds_SOMECORP_USD_1Y", "Cds_SOMECORP_USD_5Y", "Cds_SOMECORP_USD_10Y"]
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::{
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::Interpolator,
    time::{daycounter::DayCounter, enums::Frequency},
};

const fn default_day_counter() -> DayCounter {
    DayCounter::Actual360
}

const fn default_premium_frequency() -> Frequency {
    Frequency::Quarterly
}

const fn default_interpolator() -> Interpolator {
    Interpolator::LogLinear
}

const fn default_enable_extrapolation() -> bool {
    true
}

/// Configuration for bootstrapping one credit (survival) curve.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreditCurveConfiguration {
    /// Credit curve index of the reference entity (`MarketIndex::Credit`).
    market_index: MarketIndex,
    /// Currency of the CDS quotes.
    currency: Currency,
    /// Discount curve used to price the CDS pillar instruments.
    discount_index: MarketIndex,
    /// Assumed recovery rate of the reference entity.
    recovery: f64,
    /// Premium accrual day counter.
    #[serde(default = "default_day_counter")]
    day_counter: DayCounter,
    /// Premium payment frequency.
    #[serde(default = "default_premium_frequency")]
    premium_frequency: Frequency,
    /// Interpolation on survival probabilities.
    #[serde(default = "default_interpolator")]
    interpolator: Interpolator,
    /// Whether extrapolation beyond the last pillar is allowed.
    #[serde(default = "default_enable_extrapolation")]
    enable_extrapolation: bool,
    /// CDS par-spread quote identifiers (e.g. `Cds_ACME_USD_5Y`).
    #[serde(default)]
    quotes: Vec<String>,
}

impl CreditCurveConfiguration {
    /// Creates a new credit curve configuration.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        currency: Currency,
        discount_index: MarketIndex,
        recovery: f64,
        quotes: Vec<String>,
    ) -> Self {
        Self {
            market_index,
            currency,
            discount_index,
            recovery,
            day_counter: default_day_counter(),
            premium_frequency: default_premium_frequency(),
            interpolator: default_interpolator(),
            enable_extrapolation: default_enable_extrapolation(),
            quotes,
        }
    }

    /// Sets the premium day counter.
    #[must_use]
    pub const fn with_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.day_counter = day_counter;
        self
    }

    /// Sets the premium payment frequency.
    #[must_use]
    pub const fn with_premium_frequency(mut self, frequency: Frequency) -> Self {
        self.premium_frequency = frequency;
        self
    }

    /// Credit curve index.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Quote currency.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Discount curve index used for pillar pricing.
    #[must_use]
    pub const fn discount_index(&self) -> &MarketIndex {
        &self.discount_index
    }

    /// Assumed recovery rate.
    #[must_use]
    pub const fn recovery(&self) -> f64 {
        self.recovery
    }

    /// Premium day counter.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    /// Premium payment frequency.
    #[must_use]
    pub const fn premium_frequency(&self) -> Frequency {
        self.premium_frequency
    }

    /// Interpolator applied to survival probabilities.
    #[must_use]
    pub const fn interpolator(&self) -> Interpolator {
        self.interpolator
    }

    /// Whether extrapolation is enabled.
    #[must_use]
    pub const fn enable_extrapolation(&self) -> bool {
        self.enable_extrapolation
    }

    /// CDS quote identifiers.
    #[must_use]
    pub fn quotes(&self) -> &[String] {
        &self.quotes
    }
}

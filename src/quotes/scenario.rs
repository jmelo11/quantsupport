//! Market scenarios: shocks applied to quotes before curve bootstrapping and
//! volatility construction.
//!
//! A [`Scenario`] assigns a shock (absolute or relative, see
//! [`ScenarioType`]) to one or more quotes of a [`QuoteStore`]. The target of
//! the shock is either:
//!
//! - a full quote identifier (e.g. `"OIS_USD_SOFR_1Y"`), shocking that single
//!   quote, or
//! - a partial, underscore-separated selector whose segments are matched
//!   against the segments of every quote identifier (e.g. `"SOFR"` shocks all
//!   SOFR quotes — a parallel curve shift; `"OIS_USD_SOFR"` shocks all USD
//!   SOFR OIS pillars; `"Swaption_USD"` shocks the USD swaption vol slide).
//!
//! Scenarios are typically attached to a
//! [`PricingContext`](crate::core::pricingcontext::PricingContext) via
//! `with_scenarios`; on `initialize` the shocks are applied to a copy of the
//! quote store and the explicit zero-rate curve inputs. All curves, surfaces,
//! cubes and simulations are rebuilt from the shocked market. Explicit zero
//! pillars use their sensitivity labels, `<index>_<date>`; an index selector
//! matches every zero-rate pillar of that curve. Base inputs stay unchanged.
//!
//! # Example
//!
//! ```
//! use quantsupport::prelude::*;
//! use std::str::FromStr;
//!
//! let mut store = QuoteStore::new(Date::new(2025, 11, 11));
//! let details = QuoteDetails::from_str("OIS_USD_SOFR_1Y").expect("valid identifier");
//! store.add_quote(Quote::new(details, QuoteLevels::with_mid(0.04)));
//!
//! // Parallel +100bp shock on every SOFR quote.
//! let scenario = Scenario::new("SOFR", 0.01, ScenarioType::Absolute);
//! let shocked = scenario.apply(&mut store).expect("scenario must match");
//! assert_eq!(shocked, 1);
//!
//! let quote = store.quote("OIS_USD_SOFR_1Y").expect("quote exists");
//! assert!((quote.levels().mid().expect("mid") - 0.05).abs() < 1e-12);
//! ```

use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::{
    quotes::{
        quote::{Quote, QuoteLevels},
        quotestore::QuoteStore,
    },
    utils::errors::{QSError, Result},
};

/// How a [`Scenario`] shock is applied to a quote value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioType {
    /// The shock is added to the quote value: `v + shock`.
    Absolute,
    /// The shock scales the quote value: `v * (1 + shock)`.
    Relative,
}

impl FromStr for ScenarioType {
    type Err = QSError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("absolute") {
            Ok(Self::Absolute)
        } else if s.eq_ignore_ascii_case("relative") {
            Ok(Self::Relative)
        } else {
            Err(QSError::InvalidValueErr(format!(
                "Unknown scenario type: {s} (expected 'Absolute' or 'Relative')"
            )))
        }
    }
}

/// A shock applied to one or more quotes of a [`QuoteStore`].
///
/// The `target` selects the quotes to shock: either a full quote identifier
/// or a partial underscore-separated selector matched segment-wise against
/// each quote identifier (see the [module documentation](self)).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct Scenario {
    /// Quote identifier or partial (segment) selector.
    target: String,
    /// Shock size (in rate/vol units for absolute, fraction for relative).
    shock: f64,
    /// How the shock is applied.
    scenario_type: ScenarioType,
}

impl Scenario {
    /// Creates a new scenario.
    #[must_use]
    pub fn new(target: impl Into<String>, shock: f64, scenario_type: ScenarioType) -> Self {
        Self {
            target: target.into(),
            shock,
            scenario_type,
        }
    }

    /// Returns the target selector.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Returns the shock size.
    #[must_use]
    pub const fn shock(&self) -> f64 {
        self.shock
    }

    /// Returns the shock application type.
    #[must_use]
    pub const fn scenario_type(&self) -> ScenarioType {
        self.scenario_type
    }

    /// Returns `true` if the scenario targets the given quote identifier.
    ///
    /// A quote matches when every underscore-separated segment of the target
    /// appears among the underscore-separated segments of the identifier
    /// (exact identifiers therefore always match themselves).
    #[must_use]
    pub fn matches(&self, identifier: &str) -> bool {
        if self.target == identifier {
            return true;
        }
        let segments: Vec<&str> = identifier.split('_').collect();
        self.target.split('_').all(|part| segments.contains(&part))
    }

    /// Applies the shock to a single value.
    #[must_use]
    pub fn shocked_value(&self, value: f64) -> f64 {
        match self.scenario_type {
            ScenarioType::Absolute => value + self.shock,
            ScenarioType::Relative => value * (1.0 + self.shock),
        }
    }

    /// Applies the scenario to all matching quotes of the store, returning
    /// the number of shocked quotes.
    ///
    /// # Errors
    /// Returns an error if the target matches no quote in the store.
    pub fn apply(&self, store: &mut QuoteStore) -> Result<usize> {
        let count = self.apply_matching_quotes(store);
        if count == 0 {
            return Err(QSError::NotFoundErr(format!(
                "Scenario target '{}' matched no quotes in the store",
                self.target
            )));
        }
        Ok(count)
    }

    /// Shocks matching quotes, allowing a context to also match explicit curve pillars.
    pub(crate) fn apply_matching_quotes(&self, store: &mut QuoteStore) -> usize {
        let shocked: Vec<Quote> = store
            .quotes()
            .values()
            .filter(|q| self.matches(&q.details().identifier()))
            .map(|q| {
                let levels = q.levels();
                let shocked_levels = QuoteLevels::new(
                    levels.mid().map(|v| self.shocked_value(v)),
                    levels.bid().map(|v| self.shocked_value(v)),
                    levels.ask().map(|v| self.shocked_value(v)),
                );
                Quote::new(q.details().clone(), shocked_levels)
            })
            .collect();

        let count = shocked.len();
        for quote in shocked {
            store.add_quote(quote);
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::{Scenario, ScenarioType};
    use crate::{
        quotes::{
            quote::{Quote, QuoteDetails, QuoteLevels},
            quotestore::QuoteStore,
        },
        time::date::Date,
    };
    use std::str::FromStr;

    fn store_with(identifiers: &[(&str, f64)]) -> QuoteStore {
        let mut store = QuoteStore::new(Date::new(2025, 11, 11));
        for (id, mid) in identifiers {
            let details = QuoteDetails::from_str(id).expect("valid identifier");
            store.add_quote(Quote::new(details, QuoteLevels::with_mid(*mid)));
        }
        store
    }

    #[test]
    fn matches_exact_identifier() {
        let scenario = Scenario::new("OIS_USD_SOFR_1Y", 0.01, ScenarioType::Absolute);
        assert!(scenario.matches("OIS_USD_SOFR_1Y"));
        assert!(!scenario.matches("OIS_USD_SOFR_2Y"));
    }

    #[test]
    fn matches_segment_subset() {
        let scenario = Scenario::new("SOFR", 0.01, ScenarioType::Absolute);
        assert!(scenario.matches("OIS_USD_SOFR_1Y"));
        assert!(scenario.matches("OIS_USD_SOFR_10Y"));
        // Distinct index segment must not match.
        assert!(!scenario.matches("BasisSwap_USD_TermSOFR3m_TermSOFR3m_1Y"));

        let partial = Scenario::new("OIS_USD_SOFR", 0.01, ScenarioType::Absolute);
        assert!(partial.matches("OIS_USD_SOFR_2Y"));
        // Deposit has USD and SOFR segments but no OIS segment.
        assert!(!partial.matches("FixedRateDeposit_USD_SOFR_3M"));
    }

    #[test]
    fn applies_absolute_shock() {
        let mut store = store_with(&[("OIS_USD_SOFR_1Y", 0.04), ("OIS_USD_SOFR_2Y", 0.042)]);
        let scenario = Scenario::new("SOFR", 0.01, ScenarioType::Absolute);
        let count = scenario.apply(&mut store).expect("scenario applies");
        assert_eq!(count, 2);
        let mid = store
            .quote("OIS_USD_SOFR_1Y")
            .and_then(|q| q.levels().mid())
            .expect("mid exists");
        assert!((mid - 0.05).abs() < 1e-12);
    }

    #[test]
    fn applies_relative_shock() {
        let mut store = store_with(&[("OIS_USD_SOFR_1Y", 0.04)]);
        let scenario = Scenario::new("OIS_USD_SOFR_1Y", 0.5, ScenarioType::Relative);
        scenario.apply(&mut store).expect("scenario applies");
        let mid = store
            .quote("OIS_USD_SOFR_1Y")
            .and_then(|q| q.levels().mid())
            .expect("mid exists");
        assert!((mid - 0.06).abs() < 1e-12);
    }

    #[test]
    fn errors_when_no_quote_matches() {
        let mut store = store_with(&[("OIS_USD_SOFR_1Y", 0.04)]);
        let scenario = Scenario::new("EURIBOR", 0.01, ScenarioType::Absolute);
        assert!(scenario.apply(&mut store).is_err());
    }

    #[test]
    fn parses_scenario_type() {
        assert_eq!(
            "absolute".parse::<ScenarioType>().expect("parses"),
            ScenarioType::Absolute
        );
        assert_eq!(
            "Relative".parse::<ScenarioType>().expect("parses"),
            ScenarioType::Relative
        );
        assert!("banana".parse::<ScenarioType>().is_err());
    }
}

//! Combined market shocks applied through an independent, request-driven view.
//!
//! A shock target matches an exact identifier or an underscore-separated set of
//! identifier segments. For example, `EURIBOR12m` selects its deposit and swap
//! quotes. Explicit zero-rate pillars use `<index>_<date>` labels; `FUNDINGSPREAD`
//! selects every pillar of that spread curve. Absolute changes use decimal units
//! (`0.01` means 100 bp). Relative changes multiply by `1 + value`. Overlapping
//! shocks compose in their supplied order.
//!
//! ```
//! use quantsupport::prelude::*;
//! let context = PricingContext::new();
//! let scenario = Scenario::new(vec![]);
//! let market = scenario.apply(&context)?;
//! # Ok::<(), QSError>(())
//! ```

use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::{
    core::{marketdatahandling::marketview::MarketView, pricingcontext::PricingContext},
    quotes::quote::{Quote, QuoteLevels},
    rates::bootstrapping::curveconfiguration::CurveConstruction,
    utils::errors::{QSError, Result},
};

/// How a market shock is applied to a quote value.
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

/// A shock targeting quoted inputs or explicit zero-rate pillars.
///
/// The `target` selects the quotes to shock: either a full quote identifier
/// or a partial underscore-separated selector matched segment-wise against
/// each quote identifier (see the [module documentation](self)).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shock {
    /// Quote identifier or partial (segment) selector.
    target: String,
    /// Shock size (in rate/vol units for absolute, fraction for relative).
    value: f64,
    /// How the shock is applied.
    #[serde(rename = "type")]
    kind: ScenarioType,
}

impl Shock {
    /// Creates a market shock.
    #[must_use]
    pub fn new(target: impl Into<String>, value: f64, shock_type: ScenarioType) -> Self {
        Self {
            target: target.into(),
            value,
            kind: shock_type,
        }
    }

    /// Returns the target selector.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Returns the shock size.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// Returns the shock application type.
    #[must_use]
    pub const fn shock_type(&self) -> ScenarioType {
        self.kind
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
        match self.kind {
            ScenarioType::Absolute => value + self.value,
            ScenarioType::Relative => value * (1.0 + self.value),
        }
    }
}

/// A combination of ordered market shocks forming one scenario.
///
/// Applying a scenario borrows the base inputs and creates an independent cache.
/// Curves, volatility markets and simulations are resolved only when requested.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    shocks: Vec<Shock>,
}

impl Scenario {
    /// Creates a scenario from ordered shocks. Overlapping shocks compose in order.
    #[must_use]
    pub const fn new(shocks: Vec<Shock>) -> Self {
        Self { shocks }
    }

    /// Returns the shocks defining this scenario.
    #[must_use]
    pub fn shocks(&self) -> &[Shock] {
        &self.shocks
    }

    /// Borrows a base context without changing its inputs or constructed market.
    ///
    /// # Errors
    /// Returns an error for nonfinite values, empty targets or unmatched selectors.
    pub fn apply<'a>(&self, context: &'a PricingContext) -> Result<MarketView<'a>> {
        self.validate(context)?;
        Ok(MarketView::new(context, self.clone()))
    }

    /// Checks selectors against quotes and configured explicit zero-rate pillars.
    ///
    /// # Errors
    /// Returns an error for an invalid shock or a selector matching no market inputs.
    pub fn validate(&self, context: &PricingContext) -> Result<()> {
        for shock in &self.shocks {
            if shock.target().is_empty() || !shock.value().is_finite() {
                return Err(QSError::InvalidValueErr(
                    "Scenario shocks require a nonempty target and a finite value".into(),
                ));
            }
            let quoted = context
                .quote_store()
                .quotes()
                .keys()
                .any(|id| shock.matches(id));
            let explicit = context.curve_configurations().iter().any(|configuration| {
                let CurveConstruction::ZeroRate { pillars, .. } = configuration.construction()
                else {
                    return false;
                };
                pillars.iter().any(|pillar| {
                    shock.matches(&format!(
                        "{}_{}",
                        configuration.market_index(),
                        pillar.date(context.evaluation_date())
                    ))
                })
            });
            if !quoted && !explicit {
                return Err(QSError::NotFoundErr(format!(
                    "Scenario target '{}' matched no quotes or zero-rate pillars",
                    shock.target()
                )));
            }
        }
        Ok(())
    }

    /// Applies matching shocks to one quote without copying the market store.
    pub(crate) fn shock_quote(&self, quote: &Quote) -> Quote {
        let identifier = quote.details().identifier();
        let mut levels = *quote.levels();
        for shock in &self.shocks {
            if shock.matches(&identifier) {
                levels = QuoteLevels::new(
                    levels.mid().map(|v| shock.shocked_value(v)),
                    levels.bid().map(|v| shock.shocked_value(v)),
                    levels.ask().map(|v| shock.shocked_value(v)),
                );
            }
        }
        Quote::new(quote.details().clone(), levels)
    }

    pub(crate) fn matches(&self, identifier: &str) -> bool {
        self.shocks
            .iter()
            .any(|shock| shock.value() != 0.0 && shock.matches(identifier))
    }
}

#[cfg(test)]
mod tests {
    use super::{Scenario, ScenarioType, Shock};
    use crate::{core::pricingcontext::PricingContext, quotes::quoteselector::QuoteSelector};
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
        let scenario = Shock::new("OIS_USD_SOFR_1Y", 0.01, ScenarioType::Absolute);
        assert!(scenario.matches("OIS_USD_SOFR_1Y"));
        assert!(!scenario.matches("OIS_USD_SOFR_2Y"));
    }

    #[test]
    fn matches_segment_subset() {
        let scenario = Shock::new("SOFR", 0.01, ScenarioType::Absolute);
        assert!(scenario.matches("OIS_USD_SOFR_1Y"));
        assert!(scenario.matches("OIS_USD_SOFR_10Y"));
        // Distinct index segment must not match.
        assert!(!scenario.matches("BasisSwap_USD_TermSOFR3m_TermSOFR3m_1Y"));

        let partial = Shock::new("OIS_USD_SOFR", 0.01, ScenarioType::Absolute);
        assert!(partial.matches("OIS_USD_SOFR_2Y"));
        // Deposit has USD and SOFR segments but no OIS segment.
        assert!(!partial.matches("FixedRateDeposit_USD_SOFR_3M"));
    }

    #[test]
    fn applies_absolute_shock_without_changing_base_quotes() {
        let store = store_with(&[("OIS_USD_SOFR_1Y", 0.04), ("OIS_USD_SOFR_2Y", 0.042)]);
        let context = PricingContext::new().with_quote_store(store);
        let scenario = Scenario::new(vec![Shock::new("SOFR", 0.01, ScenarioType::Absolute)]);
        let market = scenario.apply(&context).expect("scenario applies");
        for (identifier, expected) in [("OIS_USD_SOFR_1Y", 0.05), ("OIS_USD_SOFR_2Y", 0.052)] {
            let mid = market
                .select(identifier)
                .and_then(|quote| quote.levels().mid())
                .expect("mid exists");
            assert!((mid - expected).abs() < 1e-12);
        }
        assert_eq!(
            context
                .quote_store()
                .quote("OIS_USD_SOFR_1Y")
                .and_then(|quote| quote.levels().mid()),
            Some(0.04)
        );
    }

    #[test]
    fn applies_relative_shock_to_available_quote_levels() {
        let mut store = store_with(&[("OIS_USD_SOFR_1Y", 0.04)]);
        store.add_quote(Quote::new(
            "OIS_USD_SOFR_1Y".parse().expect("identifier"),
            QuoteLevels::new(Some(0.04), Some(0.03), Some(0.05)),
        ));
        let context = PricingContext::new().with_quote_store(store);
        let scenario = Scenario::new(vec![Shock::new(
            "OIS_USD_SOFR_1Y",
            0.5,
            ScenarioType::Relative,
        )]);
        let market = scenario.apply(&context).expect("scenario applies");
        let quote = market.select("OIS_USD_SOFR_1Y").expect("quote");
        assert!((quote.levels().mid().expect("mid") - 0.06).abs() < 1e-12);
        assert!((quote.levels().bid().expect("bid") - 0.045).abs() < 1e-12);
        assert!((quote.levels().ask().expect("ask") - 0.075).abs() < 1e-12);
    }

    #[test]
    fn errors_when_no_quote_matches() {
        let context =
            PricingContext::new().with_quote_store(store_with(&[("OIS_USD_SOFR_1Y", 0.04)]));
        let scenario = Scenario::new(vec![Shock::new("EURIBOR", 0.01, ScenarioType::Absolute)]);
        assert!(scenario.apply(&context).is_err());
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

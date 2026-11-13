//! Serializable assumptions for deterministic forward markets.

use serde::{Deserialize, Serialize};

use crate::{
    indices::marketindex::MarketIndex,
    quotes::{quote::QuoteDetails, scenario::Scenario},
    time::{date::Date, datetime::DateTime, period::Period},
    utils::errors::{QSError, Result},
};

/// How a rate, dividend, or survival curve evolves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveRoll {
    /// Preserve forward discount-factor ratios at absolute dates.
    #[default]
    ImpliedForward,
    /// Preserve the curve's shape as a function of remaining time.
    ConstantTenor,
}

/// How quoted option volatility evolves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolatilityRoll {
    /// Preserve quoted volatility at the same remaining expiry and smile key.
    #[default]
    ConstantTenor,
    /// Subtract elapsed total variance at the same smile coordinate.
    /// This is a deterministic scenario convention, not a conditional smile model.
    ForwardVariance,
}

/// Treatment of observations becoming historical before the horizon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixingRoll {
    /// Generate intervening observations from source-market forwards.
    #[default]
    Project,
    /// Retain source history without intervening fixings; horizon spots still advance.
    Preserve,
}

/// Assumptions used by [`super::advanceintime::AdvanceInTime`].
///
/// A forward market is a deterministic scenario. Relative quote tenors are
/// anchored at `target_date`; dated contracts retain their absolute dates.
/// Generated quotes retain the source context's selected quote level. Model
/// configurations are retained and calibrated models are rebuilt when requested.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForwardMarketConfiguration {
    target_date: Date,
    #[serde(default)]
    curve_roll: CurveRoll,
    #[serde(default)]
    volatility_roll: VolatilityRoll,
    #[serde(default)]
    fixing_roll: FixingRoll,
    #[serde(default)]
    curve_overrides: Vec<(MarketIndex, CurveRoll)>,
    #[serde(default)]
    volatility_overrides: Vec<(MarketIndex, VolatilityRoll)>,
    #[serde(default)]
    quote_replacements: Vec<(String, QuoteDetails)>,
    #[serde(default)]
    surface_grids: Vec<(MarketIndex, Vec<QuoteDetails>)>,
    #[serde(default)]
    cube_grids: Vec<(MarketIndex, Vec<QuoteDetails>)>,
    #[serde(default)]
    curve_grids: Vec<(MarketIndex, Vec<Period>)>,
    #[serde(default)]
    recovery_overrides: Vec<(MarketIndex, f64)>,
}

impl ForwardMarketConfiguration {
    /// Creates a forward-implied market with constant-tenor quoted volatility.
    #[must_use]
    pub fn new(target_date: Date) -> Self {
        Self {
            target_date,
            curve_roll: CurveRoll::default(),
            volatility_roll: VolatilityRoll::default(),
            fixing_roll: FixingRoll::default(),
            curve_overrides: Vec::new(),
            volatility_overrides: Vec::new(),
            quote_replacements: Vec::new(),
            surface_grids: Vec::new(),
            cube_grids: Vec::new(),
            curve_grids: Vec::new(),
            recovery_overrides: Vec::new(),
        }
    }

    /// Horizon evaluation date.
    #[must_use]
    pub const fn target_date(&self) -> Date {
        self.target_date
    }

    /// Selects the default curve evolution convention.
    #[must_use]
    pub const fn with_curve_roll(mut self, roll: CurveRoll) -> Self {
        self.curve_roll = roll;
        self
    }

    /// Selects the default volatility evolution convention.
    #[must_use]
    pub const fn with_volatility(mut self, roll: VolatilityRoll) -> Self {
        self.volatility_roll = roll;
        self
    }

    /// Selects the intervening-fixing convention.
    #[must_use]
    pub const fn with_fixings(mut self, roll: FixingRoll) -> Self {
        self.fixing_roll = roll;
        self
    }

    /// Overrides the curve convention for one index.
    #[must_use]
    pub fn with_curve_override(mut self, index: MarketIndex, roll: CurveRoll) -> Self {
        self.curve_overrides.push((index, roll));
        self
    }

    /// Overrides the volatility convention for one index.
    #[must_use]
    pub fn with_volatility_override(mut self, index: MarketIndex, roll: VolatilityRoll) -> Self {
        self.volatility_overrides.push((index, roll));
        self
    }

    /// Reports whether an index has an explicitly selected curve rule.
    #[must_use]
    pub fn has_curve_override(&self, index: &MarketIndex) -> bool {
        self.curve_overrides.iter().any(|(key, _)| key == index)
    }

    /// Resolves the curve convention for an index.
    #[must_use]
    pub fn curve_roll(&self, index: &MarketIndex) -> CurveRoll {
        self.curve_overrides
            .iter()
            .rev()
            .find(|(key, _)| key == index)
            .map_or(self.curve_roll, |(_, roll)| *roll)
    }

    /// Resolves the volatility convention for an index.
    #[must_use]
    pub fn volatility_roll(&self, index: &MarketIndex) -> VolatilityRoll {
        self.volatility_overrides
            .iter()
            .rev()
            .find(|(key, _)| key == index)
            .map_or(self.volatility_roll, |(_, roll)| *roll)
    }

    /// Changes a horizon recovery assumption while retaining conditional survival.
    #[must_use]
    pub fn with_recovery_override(mut self, index: MarketIndex, recovery: f64) -> Self {
        self.recovery_overrides.push((index, recovery));
        self
    }

    /// Resolves a horizon recovery assumption for an index.
    #[must_use]
    pub fn recovery(&self, index: &MarketIndex, original: f64) -> f64 {
        self.recovery_overrides
            .iter()
            .rev()
            .find(|(key, _)| key == index)
            .map_or(original, |(_, value)| *value)
    }

    /// Validates finite recovery assumptions.
    ///
    /// # Errors
    /// Returns an error when recovery is outside [0, 1).
    pub fn validate(&self) -> Result<()> {
        if self
            .recovery_overrides
            .iter()
            .any(|(_, value)| !value.is_finite() || !(0.0..1.0).contains(value))
        {
            return Err(QSError::InvalidValueErr(
                "Recovery overrides must be finite and in [0, 1)".into(),
            ));
        }
        Ok(())
    }

    /// Replaces a dated contract or changes a target calibration definition.
    #[must_use]
    pub fn with_quote_replacement(
        mut self,
        identifier: impl Into<String>,
        target: QuoteDetails,
    ) -> Self {
        self.quote_replacements.push((identifier.into(), target));
        self
    }

    /// Selects remaining-tenor pillars when exporting a supplied runtime curve.
    /// Without an explicit grid, available future nodes are used; curves without
    /// nodes use 0D, 1Y, 2Y, 5Y, 10Y and 30Y, subject to source extrapolation rules.
    #[must_use]
    pub fn with_curve_grid(mut self, index: MarketIndex, tenors: Vec<Period>) -> Self {
        self.curve_grids.push((index, tenors));
        self
    }

    /// Returns the explicit export-tenor grid for an index, if supplied.
    #[must_use]
    pub fn curve_grid(&self, index: &MarketIndex) -> Option<&[Period]> {
        self.curve_grids
            .iter()
            .rev()
            .find(|(key, _)| key == index)
            .map(|(_, grid)| grid.as_slice())
    }

    /// Supplies quotation metadata for a runtime-only volatility surface.
    #[must_use]
    pub fn with_surface_grid(mut self, index: MarketIndex, quotes: Vec<QuoteDetails>) -> Self {
        self.surface_grids.push((index, quotes));
        self
    }

    /// Supplies quotation metadata for a runtime-only volatility cube.
    #[must_use]
    pub fn with_cube_grid(mut self, index: MarketIndex, quotes: Vec<QuoteDetails>) -> Self {
        self.cube_grids.push((index, quotes));
        self
    }

    /// Returns a replacement definition for a source quote identifier.
    #[must_use]
    pub fn replacement(&self, identifier: &str) -> Option<&QuoteDetails> {
        self.quote_replacements
            .iter()
            .rev()
            .find(|(key, _)| key.eq_ignore_ascii_case(identifier))
            .map(|(_, value)| value)
    }

    /// Resolves the target identifier of a source quote.
    #[must_use]
    pub fn target_identifier(&self, identifier: &str) -> String {
        self.replacement(identifier)
            .map_or_else(|| identifier.to_owned(), QuoteDetails::identifier)
    }

    /// Returns quotation metadata for a supplied volatility surface.
    #[must_use]
    pub fn surface_grid(&self, index: &MarketIndex) -> Option<&[QuoteDetails]> {
        self.surface_grids
            .iter()
            .rev()
            .find(|(key, _)| key == index)
            .map(|(_, grid)| grid.as_slice())
    }

    /// Returns quotation metadata for a supplied volatility cube.
    #[must_use]
    pub fn cube_grid(&self, index: &MarketIndex) -> Option<&[QuoteDetails]> {
        self.cube_grids
            .iter()
            .rev()
            .find(|(key, _)| key == index)
            .map(|(_, grid)| grid.as_slice())
    }

    /// Returns the intervening-fixing convention.
    #[must_use]
    pub const fn fixing_roll(&self) -> FixingRoll {
        self.fixing_roll
    }
}

/// Origin and assumptions of generated market observations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketAdvancementDetails {
    /// Evaluation date of the source market.
    pub source_date: Date,
    /// Source observation instant, when known; this is not a horizon observation.
    pub source_timestamp: Option<DateTime>,
    /// Source quote shocks, when advancement started from a market view.
    #[serde(default)]
    pub source_scenario: Option<Scenario>,
    /// Deterministic advancement assumptions.
    pub configuration: ForwardMarketConfiguration,
}

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{
    core::marketdatahandling::constructedelementstore::ConstructedElementStore,
    indices::marketindex::MarketIndex,
    quotes::quote::QuoteDetails,
    time::period::Period,
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
};

/// Specifies which constructed volatility market a model calibrates to.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CalibrationSource {
    /// Uses option volatilities from a two-dimensional expiry and smile
    /// surface. Rate models typically use caplet surfaces, while lognormal
    /// asset models can use equity or FX option surfaces.
    Surface {
        /// Market index used to locate the surface in the constructed store.
        market_index: MarketIndex,
    },
    /// Uses option volatilities from a three-dimensional expiry, tenor, and
    /// smile cube. Rate models typically use swaption cubes.
    Cube {
        /// Market index used to locate the cube in the constructed store.
        market_index: MarketIndex,
    },
}

/// Defines the option instruments used to calibrate a model.
///
/// `expiries` and `tenors` filter the instruments recorded by the selected
/// volatility market. An omitted axis includes every available value. On a
/// surface, `tenors` refers to the floating-rate index tenor, such as the `3M`
/// in a SOFR 3M caplet. On a cube, it refers to the option's underlying swap
/// tenor, such as `5Y` in a 1Y into 5Y swaption.
///
/// Supplied expiry and tenor values are strict requirements. Each value must
/// occur in the source quote metadata and must participate in at least one
/// instrument after the expiry and tenor filters are combined. Resolution
/// returns [`QSError::NotFoundErr`] before calibration when either condition
/// fails. For example, a request for `6M` returns an error when a caplet
/// surface contains only `3M` index-tenor quotes.
///
/// `strike` supplies a common strike or moneyness rule for the selected
/// instruments. For example, `Strike::Atm` creates one at-the-money
/// calibration instrument for each selected expiry and tenor. When `strike`
/// is omitted, each selected smile node keeps its quoted strike.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationBasket {
    #[serde(default)]
    expiries: Option<Vec<Period>>,
    #[serde(default)]
    tenors: Option<Vec<Period>>,
    #[serde(default)]
    strike: Option<Strike>,
}

impl CalibrationBasket {
    /// Creates a calibration basket containing every instrument in the
    /// selected volatility market.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            expiries: None,
            tenors: None,
            strike: None,
        }
    }

    /// Selects the supplied option expiries.
    ///
    /// Resolution returns an error when the source market has no matching
    /// quote for any requested expiry.
    #[must_use]
    pub fn with_expiries(mut self, expiries: Vec<Period>) -> Self {
        self.expiries = Some(expiries);
        self
    }

    /// Selects the supplied underlying tenors.
    ///
    /// Resolution returns an error when the source market has no matching
    /// quote for any requested tenor.
    #[must_use]
    pub fn with_tenors(mut self, tenors: Vec<Period>) -> Self {
        self.tenors = Some(tenors);
        self
    }

    /// Applies the supplied strike or moneyness rule to every selected
    /// expiry-tenor pair.
    #[must_use]
    pub const fn with_strike(mut self, strike: Strike) -> Self {
        self.strike = Some(strike);
        self
    }

    /// Returns the selected option expiries. `None` includes all expiries.
    #[must_use]
    pub fn expiries(&self) -> Option<&[Period]> {
        self.expiries.as_deref()
    }

    /// Returns the selected underlying tenors. `None` includes all tenors.
    #[must_use]
    pub fn tenors(&self) -> Option<&[Period]> {
        self.tenors.as_deref()
    }

    /// Returns the common strike or moneyness rule.
    #[must_use]
    pub const fn strike(&self) -> Option<Strike> {
        self.strike
    }

    fn includes(&self, details: &QuoteDetails, source: &CalibrationSource) -> bool {
        let expiry_matches = self
            .expiries
            .as_ref()
            .is_none_or(|values| details.option_expiry().is_some_and(|e| values.contains(&e)));
        let tenor = match source {
            CalibrationSource::Surface { .. } => details.index_tenor(),
            CalibrationSource::Cube { .. } => details.tenor(),
        };
        let tenor_matches = self
            .tenors
            .as_ref()
            .is_none_or(|values| tenor.is_some_and(|t| values.contains(&t)));
        expiry_matches && tenor_matches
    }

    fn validate_available_axes(
        &self,
        available: &[QuoteDetails],
        source: &CalibrationSource,
    ) -> Result<()> {
        if let Some(expiries) = &self.expiries {
            for expiry in expiries {
                if !available
                    .iter()
                    .any(|details| details.option_expiry() == Some(*expiry))
                {
                    return Err(QSError::NotFoundErr(format!(
                        "Calibration basket requests expiry {expiry}, but the source volatility market contains no instrument at that expiry"
                    )));
                }
            }
        }
        if let Some(tenors) = &self.tenors {
            for tenor in tenors {
                let found = available.iter().any(|details| match source {
                    CalibrationSource::Surface { .. } => details.index_tenor() == Some(*tenor),
                    CalibrationSource::Cube { .. } => details.tenor() == Some(*tenor),
                });
                if !found {
                    return Err(QSError::NotFoundErr(format!(
                        "Calibration basket requests tenor {tenor}, but the source volatility market contains no instrument with that tenor"
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_selected_axes(
        &self,
        selected: &[QuoteDetails],
        source: &CalibrationSource,
    ) -> Result<()> {
        if let Some(expiries) = &self.expiries {
            for expiry in expiries {
                if !selected
                    .iter()
                    .any(|details| details.option_expiry() == Some(*expiry))
                {
                    return Err(QSError::NotFoundErr(format!(
                        "Calibration expiry {expiry} has no instrument matching the requested tenor filter"
                    )));
                }
            }
        }
        if let Some(tenors) = &self.tenors {
            for tenor in tenors {
                let found = selected.iter().any(|details| match source {
                    CalibrationSource::Surface { .. } => details.index_tenor() == Some(*tenor),
                    CalibrationSource::Cube { .. } => details.tenor() == Some(*tenor),
                });
                if !found {
                    return Err(QSError::NotFoundErr(format!(
                        "Calibration tenor {tenor} has no instrument matching the requested expiry filter"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Identifies a volatility market and the instruments used to fit model
/// parameters.
///
/// `source` locates a constructed surface or cube. `calibration_basket`
/// chooses expiries, tenors, and a strike rule from the instruments recorded
/// by that market. The surface or cube configuration owns the quote list, so
/// every model that references it sees the same market definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCalibrationConfiguration {
    /// Constructed volatility surface or cube used as the calibration target.
    source: CalibrationSource,
    /// Instrument selection applied to the target market.
    #[serde(default)]
    calibration_basket: CalibrationBasket,
}

impl ModelCalibrationConfiguration {
    /// Creates a calibration configuration selecting every source pillar.
    #[must_use]
    pub const fn new(source: CalibrationSource) -> Self {
        Self {
            source,
            calibration_basket: CalibrationBasket::all(),
        }
    }

    /// Sets the instruments used during calibration.
    #[must_use]
    pub fn with_calibration_basket(mut self, calibration_basket: CalibrationBasket) -> Self {
        self.calibration_basket = calibration_basket;
        self
    }

    /// Applies a common strike or moneyness rule to the calibration
    /// instruments.
    #[must_use]
    pub fn with_strike(mut self, strike: Strike) -> Self {
        self.calibration_basket = self.calibration_basket.with_strike(strike);
        self
    }

    /// Returns the volatility market to calibrate against.
    #[must_use]
    pub const fn source(&self) -> &CalibrationSource {
        &self.source
    }

    /// Returns the instruments selected for calibration.
    #[must_use]
    pub const fn calibration_basket(&self) -> &CalibrationBasket {
        &self.calibration_basket
    }

    /// Returns the common strike or moneyness rule.
    #[must_use]
    pub const fn strike(&self) -> Option<Strike> {
        self.calibration_basket.strike()
    }

    /// Resolves calibration instrument identifiers from the referenced
    /// surface or cube and applies the configured calibration basket.
    ///
    /// # Errors
    /// Returns an error if the market is missing, its calibration instrument
    /// identifiers are unavailable, a requested expiry or tenor is absent, a
    /// pillar label is invalid, or the combined filters select no instruments.
    pub fn resolve_instrument_ids(&self, store: &ConstructedElementStore) -> Result<Vec<String>> {
        let labels = match &self.source {
            CalibrationSource::Surface { market_index } => store
                .volatility_surface(market_index)
                .ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Volatility surface not found for index {market_index}"
                    ))
                })?
                .surface()
                .calibration_instrument_ids(),
            CalibrationSource::Cube { market_index } => store
                .volatility_cube(market_index)
                .ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "Volatility cube not found for index {market_index}"
                    ))
                })?
                .cube()
                .calibration_instrument_ids(),
        }
        .ok_or_else(|| {
            QSError::InvalidValueErr(
                "Calibration source has no instrument identifiers. Construct it with a volatility builder or attach calibration instrument identifiers"
                    .into(),
            )
        })?;

        let mut available = Vec::with_capacity(labels.len());
        let expected_index = match &self.source {
            CalibrationSource::Surface { market_index }
            | CalibrationSource::Cube { market_index } => market_index,
        };
        for label in labels {
            let details = QuoteDetails::from_str(&label)?;
            if details.market_index() != Some(expected_index) {
                return Err(QSError::InvalidValueErr(format!(
                    "Calibration instrument {label} belongs to {:?}, expected {expected_index}",
                    details.market_index()
                )));
            }
            available.push((label, details));
        }

        let available_details: Vec<QuoteDetails> = available
            .iter()
            .map(|(_, details)| details.clone())
            .collect();
        self.calibration_basket
            .validate_available_axes(&available_details, &self.source)?;

        let (selected, selected_details): (Vec<String>, Vec<QuoteDetails>) = available
            .into_iter()
            .filter(|(_, details)| self.calibration_basket.includes(details, &self.source))
            .unzip();
        if selected.is_empty() {
            return Err(QSError::NotFoundErr(
                "Calibration basket has no instruments matching the requested expiry and tenor filters"
                    .into(),
            ));
        }
        self.calibration_basket
            .validate_selected_axes(&selected_details, &self.source)?;
        Ok(selected)
    }
}

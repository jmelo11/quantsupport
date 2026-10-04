//! CSA (Credit Support Annex) terms — per-client collateral and credit configuration.
//!
//! Each counterparty (netting agreement) carries its own [`CsaTerms`]
//! describing the collateral treatment (which curve remunerates posted
//! collateral) and the credit/funding parameters used for CVA and FVA.
//! The struct is serde-deserializable so it can be loaded straight from
//! JSON configuration files.

use serde::{Deserialize, Serialize};

use crate::{
    core::collateral::SingleCurveCSADiscountPolicy,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// Term structure of funding spreads used for FVA.
///
/// Pillar dates with their (annualized, continuously-compounded) funding
/// spreads over the system curve. Spreads at intermediate dates are linearly
/// interpolated; extrapolation is flat on both sides.
///
/// ```json
/// { "dates": ["2026-09-07", "2028-09-07"], "spreads": [0.004, 0.006] }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FundingSpreadCurve {
    /// Pillar dates (strictly increasing).
    pub dates: Vec<Date>,
    /// Funding spreads at the pillar dates.
    pub spreads: Vec<f64>,
}

impl FundingSpreadCurve {
    /// Validates that the curve is non-empty, consistent and sorted.
    ///
    /// # Errors
    /// Returns an error if the curve is empty, if `dates` and `spreads` have
    /// different lengths, or if `dates` is not strictly increasing.
    pub fn validate(&self) -> Result<()> {
        if self.dates.is_empty() {
            return Err(QSError::InvalidValueErr(
                "Funding spread curve must have at least one pillar".into(),
            ));
        }
        if self.dates.len() != self.spreads.len() {
            return Err(QSError::InvalidValueErr(format!(
                "Funding spread curve has {} dates but {} spreads",
                self.dates.len(),
                self.spreads.len()
            )));
        }
        if self.dates.windows(2).any(|w| w[1] <= w[0]) {
            return Err(QSError::InvalidValueErr(
                "Funding spread curve dates must be strictly increasing".into(),
            ));
        }
        Ok(())
    }
}

/// Per-client CSA and credit configuration.
///
/// Defines the collateral treatment (discounting) and the credit/funding
/// parameters of a single netting agreement:
///
/// ```json
/// {
///     "collateral_index": "SOFR",
///     "collateral_currency": "USD",
///     "credit_spread": 0.01,
///     "recovery": 0.4,
///     "funding_spread": 0.005,
///     "credit_index": "Credit(CORPORATE-A)"
/// }
/// ```
///
/// Instead of the flat `funding_spread`, FVA can be driven by a term
/// structure of spreads (`funding_spread_curve`) or by a bootstrapped
/// funding curve (`funding_index`):
///
/// ```json
/// {
///     "funding_spread_curve": {
///         "dates": ["2026-09-07", "2030-09-07"],
///         "spreads": [0.004, 0.006]
///     }
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CsaTerms {
    /// Remuneration index of the posted collateral. Trades in other
    /// currencies are discounted with a collateral-adjusted curve.
    pub collateral_index: MarketIndex,
    /// Currency of the posted collateral.
    pub collateral_currency: Currency,
    /// Counterparty credit spread used for CVA when no bootstrapped credit
    /// curve is assigned (see [`Self::credit_index`]).
    pub credit_spread: f64,
    /// Counterparty recovery rate used for CVA.
    pub recovery: f64,
    /// Flat funding spread used for FVA. Ignored when
    /// [`Self::funding_spread_curve`] or [`Self::funding_index`] is set.
    #[serde(default)]
    pub funding_spread: f64,
    /// Explicit term structure of funding spreads used for FVA. Takes
    /// precedence over the flat [`Self::funding_spread`]. When
    /// [`Self::funding_index`] is also set, this curve is applied as an
    /// additive overlay on top of the curve-implied basis (i.e. it is a
    /// spread over the funding curve).
    #[serde(default)]
    pub funding_spread_curve: Option<FundingSpreadCurve>,
    /// Bootstrapped funding discount curve. When set, the FVA funding spreads
    /// are derived as the forward spreads of this curve over the engine's
    /// system (base) curve at the simulation dates, plus any
    /// [`Self::funding_spread_curve`] overlay. Takes precedence over the
    /// flat [`Self::funding_spread`].
    #[serde(default)]
    pub funding_index: Option<MarketIndex>,
    /// Bootstrapped credit curve of the counterparty. When set, CVA uses the
    /// survival probabilities of this curve (with sensitivities to the CDS
    /// quotes) instead of the flat `credit_spread`.
    #[serde(default)]
    pub credit_index: Option<MarketIndex>,
}

impl CsaTerms {
    /// Builds the discount policy implied by the collateral terms.
    #[must_use]
    pub fn discount_policy(&self) -> SingleCurveCSADiscountPolicy {
        SingleCurveCSADiscountPolicy::new(self.collateral_index.clone(), self.collateral_currency)
    }
}

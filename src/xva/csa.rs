//! CSA (Credit Support Annex) terms — per-client collateral and credit configuration.
//!
//! Each counterparty (netting agreement) carries its own [`CsaTerms`]
//! describing the collateral treatment (which curve remunerates posted
//! collateral) and the credit/funding parameters used for CVA and FVA.
//! The struct is serde-deserializable so it can be loaded straight from
//! JSON configuration files.

use serde::{Deserialize, Serialize};

use crate::{
    ad::scalar::Scalar,
    core::collateral::SingleCurveCSADiscountPolicy,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    rates::yieldtermstructure::zeroratetermstructure::ZeroRateTermStructure,
    utils::errors::{QSError, Result},
};

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
///     "credit_index": {"Credit": "CORPORATE-A"}
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
///         "reference_date": "2026-09-07",
///         "dates": ["2026-09-07", "2030-09-07"],
///         "rates": [0.004, 0.006],
///         "rate_definition": { "day_counter": "Actual365", "compounding": "Continuous", "frequency": "Annual" },
///         "interpolator": "Linear",
///         "enable_extrapolation": true
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
    /// Enables variation-margin calls at every simulation date. Defaults to
    /// false, preserving uncollateralized XVA for existing configurations.
    #[serde(default)]
    pub variation_margin: bool,
    /// Unsecured positive exposure allowed before the counterparty posts
    /// collateral, measured in collateral currency.
    #[serde(default)]
    pub counterparty_threshold: f64,
    /// Unsecured negative exposure allowed before we post collateral,
    /// measured in collateral currency.
    #[serde(default)]
    pub own_threshold: f64,
    /// Minimum absolute change in collateral balance triggering a transfer,
    /// measured in collateral currency. Equality triggers a transfer.
    #[serde(default)]
    pub minimum_transfer_amount: f64,
    /// Counterparty credit spread used for CVA when no bootstrapped credit
    /// curve is assigned (see [`Self::credit_index`]). This is an annual
    /// decimal spread; flat hazard is `credit_spread / (1 - recovery)`.
    pub credit_spread: f64,
    /// Counterparty recovery fraction used for CVA, e.g. `0.4` for 40%.
    pub recovery: f64,
    /// Annual decimal flat funding spread used for signed-NPV FVA. Ignored when
    /// [`Self::funding_spread_curve`] or [`Self::funding_index`] is set.
    #[serde(default)]
    pub funding_spread: f64,
    /// Explicit term structure of funding spreads used for FVA. Takes
    /// precedence over the flat [`Self::funding_spread`]. When
    /// [`Self::funding_index`] is also set, this curve is applied as an
    /// additive overlay on top of the curve-implied basis (i.e. it is a
    /// spread over the funding curve).
    #[serde(default)]
    pub funding_spread_curve: Option<ZeroRateTermStructure>,
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
    /// Validates that variation-margin amounts are finite and non-negative,
    /// including when variation margin is disabled.
    ///
    /// # Errors
    /// Returns an error for negative or non-finite thresholds or MTA.
    pub fn validate(&self) -> Result<()> {
        validate_margin_parameters(
            self.counterparty_threshold,
            self.own_threshold,
            self.minimum_transfer_amount,
        )
    }

    /// Builds the discount policy implied by the collateral terms.
    #[must_use]
    pub fn discount_policy(&self) -> SingleCurveCSADiscountPolicy {
        SingleCurveCSADiscountPolicy::new(self.collateral_index.clone(), self.collateral_currency)
    }
}

/// Tracks accrued collateral and applies threshold and minimum-transfer rules.
pub struct VariationMarginAccount<T: Scalar> {
    balance: T,
    counterparty_threshold: f64,
    own_threshold: f64,
    minimum_transfer_amount: f64,
}

impl<T: Scalar> VariationMarginAccount<T> {
    /// Creates a variation-margin account with an initially zero collateral balance.
    ///
    /// # Errors
    /// Returns an error for negative or non-finite thresholds or transfer amounts.
    pub fn new(
        counterparty_threshold: f64,
        own_threshold: f64,
        minimum_transfer_amount: f64,
    ) -> Result<Self> {
        validate_margin_parameters(
            counterparty_threshold,
            own_threshold,
            minimum_transfer_amount,
        )?;
        Ok(Self {
            balance: T::zero(),
            counterparty_threshold,
            own_threshold,
            minimum_transfer_amount,
        })
    }

    /// Accrues the retained balance, settles any margin call and returns unsecured exposure.
    pub fn exposure_after_call(&mut self, exposure: T, accrual_factor: T) -> T {
        self.balance = self.balance.mul_val(accrual_factor);
        let target = if exposure.value() > self.counterparty_threshold {
            exposure.sub_val(T::scalar(self.counterparty_threshold))
        } else if exposure.value() < -self.own_threshold {
            exposure.add_val(T::scalar(self.own_threshold))
        } else {
            T::zero()
        };
        let transfer = target.sub_val(self.balance);
        if transfer.value().abs() >= self.minimum_transfer_amount {
            self.balance = target;
        }
        exposure.sub_val(self.balance)
    }
}

fn validate_margin_parameters(
    counterparty_threshold: f64,
    own_threshold: f64,
    minimum_transfer_amount: f64,
) -> Result<()> {
    for (name, value) in [
        ("counterparty_threshold", counterparty_threshold),
        ("own_threshold", own_threshold),
        ("minimum_transfer_amount", minimum_transfer_amount),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(QSError::InvalidValueErr(format!(
                "CSA {name} must be finite and non-negative"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variation_margin_thresholds_and_transfer_changes() -> Result<()> {
        let mut account = VariationMarginAccount::<f64>::new(10.0, 20.0, 5.0)?;
        assert_eq!(account.exposure_after_call(14.0, 1.0), 14.0);
        assert_eq!(account.exposure_after_call(15.0, 1.0), 10.0);
        assert_eq!(account.exposure_after_call(18.0, 1.0), 13.0);
        assert_eq!(account.exposure_after_call(20.0, 1.0), 10.0);
        assert_eq!(account.exposure_after_call(-100.0, 1.0), -20.0);
        assert_eq!(account.exposure_after_call(0.0, 1.0), 0.0);
        Ok(())
    }

    #[test]
    fn variation_margin_accrues_retained_balance() -> Result<()> {
        let mut account = VariationMarginAccount::<f64>::new(10.0, 20.0, 5.0)?;
        assert_eq!(account.exposure_after_call(100.0, 1.0), 10.0);
        assert!((account.exposure_after_call(103.0, 1.01) - 12.1).abs() < 1e-12);
        assert_eq!(account.exposure_after_call(110.0, 1.0), 10.0);
        Ok(())
    }

    #[test]
    fn variation_margin_rejects_invalid_parameters() {
        for invalid in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(VariationMarginAccount::<f64>::new(invalid, 0.0, 0.0).is_err());
            assert!(VariationMarginAccount::<f64>::new(0.0, invalid, 0.0).is_err());
            assert!(VariationMarginAccount::<f64>::new(0.0, 0.0, invalid).is_err());
        }
    }

    #[test]
    fn legacy_serde_disables_variation_margin() -> Result<()> {
        let terms: CsaTerms = serde_json::from_str(
            r#"{"collateral_index":"SOFR","collateral_currency":"USD","credit_spread":0.01,"recovery":0.4}"#,
        )
        .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        assert!(!terms.variation_margin);
        assert_eq!(terms.counterparty_threshold, 0.0);
        assert_eq!(terms.own_threshold, 0.0);
        assert_eq!(terms.minimum_transfer_amount, 0.0);
        terms.validate()
    }

    #[test]
    fn variation_margin_preserves_dualfwd_derivatives() -> Result<()> {
        use crate::ad::{dual::DualFwd, tape::Tape};

        Tape::rewind_to_init_fwd();
        Tape::start_recording_fwd();
        let first = DualFwd::new(100.0);
        let second = DualFwd::new(103.0);
        let mut account = VariationMarginAccount::new(10.0, 20.0, 5.0)?;
        let capped = account.exposure_after_call(first, DualFwd::one());
        let retained = account.exposure_after_call(second, DualFwd::one());
        assert_eq!(capped.value(), 10.0);
        assert_eq!(retained.value(), 13.0);
        retained.backward()?;
        assert_eq!(first.adjoint()?.value(), -1.0);
        assert_eq!(second.adjoint()?.value(), 1.0);
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
        Ok(())
    }

    #[test]
    fn variation_margin_accrual_sensitivity_matches_bump() -> Result<()> {
        use crate::ad::{dual::DualFwd, tape::Tape};

        Tape::rewind_to_init_fwd();
        Tape::start_recording_fwd();
        let first = DualFwd::new(100.0);
        let second = DualFwd::new(103.0);
        let discount = DualFwd::new(0.99);
        let mut account = VariationMarginAccount::new(10.0, 20.0, 5.0)?;
        let capped = account.exposure_after_call(first, DualFwd::one());
        assert_eq!(capped.value(), 10.0);
        let residual = account.exposure_after_call(second, DualFwd::one().div_val(discount));
        residual.backward()?;
        assert!((first.adjoint()?.value() + 1.0 / 0.99).abs() < 1e-12);
        assert_eq!(second.adjoint()?.value(), 1.0);
        let bump = 1e-6;
        let evaluate = |discount: f64| -> Result<f64> {
            let mut account = VariationMarginAccount::<f64>::new(10.0, 20.0, 5.0)?;
            account.exposure_after_call(100.0, 1.0);
            Ok(account.exposure_after_call(103.0, 1.0 / discount))
        };
        let finite_difference = (evaluate(0.99 + bump)? - evaluate(0.99 - bump)?) / (2.0 * bump);
        assert!((discount.adjoint()?.value() - finite_difference).abs() < 1e-7);
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
        Ok(())
    }
}

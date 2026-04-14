use std::collections::HashMap;

use crate::{
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::leg::Leg,
    quotes::fxstore::FxStore,
    rates::bootstrapping::{
        bootstrapdiscountpolicy::BootstrapDiscountPolicy, bootstrappedcurve::BootstrappedCurve,
    },
    utils::errors::{QSError, Result},
};

/// Merges the trial curve (the one being solved) with all previously-solved
/// curves and provides per-leg curve resolution via the
/// [`BootstrapDiscountPolicy`].
pub struct BootstrapStep<'a> {
    curves: HashMap<MarketIndex, &'a BootstrappedCurve>,
    discount_policy: &'a BootstrapDiscountPolicy,
    fx_store: &'a FxStore,
}

impl<'a> BootstrapStep<'a> {
    /// Builds a curve set from the trial curve and the already-solved curves.
    #[must_use]
    pub fn new(
        trial: &'a BootstrappedCurve,
        other_curves: &'a HashMap<MarketIndex, BootstrappedCurve>,
        discount_policy: &'a BootstrapDiscountPolicy,
        fx_store: &'a FxStore,
    ) -> Self {
        let mut curves: HashMap<MarketIndex, &BootstrappedCurve> =
            other_curves.iter().map(|(k, v)| (k.clone(), v)).collect();
        curves.insert(trial.market_index(), trial);
        Self {
            curves,
            discount_policy,
            fx_store,
        }
    }

    /// Looks up a curve by market index.
    #[must_use]
    pub fn get(&self, index: &MarketIndex) -> Option<&BootstrappedCurve> {
        self.curves.get(index).copied()
    }

    /// Returns the discount policy.
    #[must_use]
    pub const fn discount_policy(&self) -> &BootstrapDiscountPolicy {
        self.discount_policy
    }

    /// Resolves the discount curve for the given leg via the discount policy.
    ///
    /// # Errors
    /// Returns an error if the leg's discount index cannot be resolved via the policy or if the corresponding curve is not found in the curve set, which typically indicates a misconfiguration in the curve
    pub fn discount_curve_for_leg(&self, leg: &Leg<f64>) -> Result<&BootstrappedCurve> {
        let index = self.discount_policy.discount_index(leg)?;
        self.curves
            .get(&index)
            .copied()
            .ok_or_else(|| QSError::NotFoundErr(format!("Missing discount curve {index}")))
    }

    /// Resolves the forward/projection curve for the given leg, if it has one.
    ///
    /// # Errors
    /// Returns an error if the leg has a forward index but the corresponding curve is not found in the curve set, which typically indicates a misconfiguration in the curve specifications or an issue with the
    pub fn forward_curve_for_leg(&self, leg: &Leg<f64>) -> Result<Option<&BootstrappedCurve>> {
        match leg.forward_index() {
            Some(idx) => {
                let curve =
                    self.curves.get(idx).copied().ok_or_else(|| {
                        QSError::NotFoundErr(format!("Missing forward curve {idx}"))
                    })?;
                Ok(Some(curve))
            }
            None => Ok(None),
        }
    }

    /// Looks up the FX spot rate.
    ///
    /// # Errors
    /// Returns an error if the exchange rate for the given currency pair is not available in the [`FxStore`].
    /// The error message will indicate the missing currency pair to aid in debugging bootstr
    pub fn fx_spot(&self, base: Currency, quote: Currency) -> Result<f64> {
        Ok(self.fx_store.get_fx_rate(base, quote)?.value())
    }
}

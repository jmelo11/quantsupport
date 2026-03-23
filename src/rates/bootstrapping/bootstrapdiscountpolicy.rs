use std::collections::HashMap;

use crate::{
    core::{
        collateral::{
            DiscountPolicy, Discountable, FixedIncomeDiscountPolicy, SingleCurveCSADiscountPolicy,
        },
        instrument::AssetClass,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::leg::Leg,
    utils::errors::{QSError, Result},
};

/// Discount-curve resolution policy used during the bootstrap.
pub struct BootstrapDiscountPolicy {
    fixed_income_discount_policy: FixedIncomeDiscountPolicy,
    csa_discount_policy: SingleCurveCSADiscountPolicy,
    /// Per-currency overrides: maps a leg currency to a specific collateral
    /// curve index instead of the default CSA.
    collateral_overrides: HashMap<Currency, MarketIndex>,
}

impl BootstrapDiscountPolicy {
    /// Creates a new bootstrap discount policy.
    ///
    /// `csa_index` and `csa_currency` describe the primary collateral curve.
    #[must_use]
    pub fn new(csa_index: MarketIndex, csa_currency: Currency) -> Self {
        Self {
            fixed_income_discount_policy: FixedIncomeDiscountPolicy::new(true),
            csa_discount_policy: SingleCurveCSADiscountPolicy::new(csa_index, csa_currency),
            collateral_overrides: HashMap::new(),
        }
    }

    /// Returns the primary CSA curve index.
    #[allow(clippy::unwrap_used)]
    #[must_use]
    pub fn csa_index(&self) -> MarketIndex {
        self.csa_discount_policy
            .discount_indices()
            .first()
            .cloned()
            .unwrap()
    }

    /// Resolves the discount curve index for the given leg.
    ///
    /// # Errors
    /// Returns an error if the leg's asset class is unsupported or if the appropriate discount index
    /// cannot be resolved based on the leg's characteristics and the policy configuration.
    pub fn discount_index(&self, leg: &Leg<f64>) -> Result<MarketIndex> {
        match leg.asset_class() {
            AssetClass::FixedIncome => self.fixed_income_discount_policy.accept(leg),
            AssetClass::InterestRate | AssetClass::Fx => self.csa_discount_policy.accept(leg),
            _ => Err(QSError::InvalidValueErr(format!(
                "Unsupported asset class for discounting: {:?}",
                leg.asset_class()
            ))),
        }
    }

    /// Resolves the discount curve index for a given currency under the FX
    /// asset class. Uses collateral overrides first, then falls back to the
    /// CSA policy.
    ///
    /// # Errors
    /// Returns an error if the currency is not supported or if the appropriate discount index
    /// cannot be resolved based on the policy configuration.
    pub fn discount_index_for_currency(&self, currency: Currency) -> Result<MarketIndex> {
        if let Some(idx) = self.collateral_overrides.get(&currency) {
            return Ok(idx.clone());
        }
        self.csa_discount_policy
            .accept(&CurrencyDiscountable { currency })
    }
}

/// Lightweight [`Discountable`] wrapper for resolving discount curves by currency.
struct CurrencyDiscountable {
    currency: Currency,
}

impl Discountable for CurrencyDiscountable {
    fn asset_class(&self) -> AssetClass {
        AssetClass::Fx
    }

    fn currency(&self) -> Currency {
        self.currency
    }
}

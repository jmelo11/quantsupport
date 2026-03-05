use std::collections::HashMap;

use crate::{currencies::currency::Currency, indices::marketindex::MarketIndex};
use crate::{
    instruments::{
        equity::equityeuropeanoption::EquityEuropeanOption,
        fixedincome::fixedratedeposit::FixedRateDeposit,
        rates::{caplet::CapletFloorlet, crosscurrencyswap::CrossCurrencySwap},
    },
    utils::errors::{AtlasError, Result},
};

/// Generic visitor-style discount policy.
///
/// The generic parameter `T` is the visited type (instrument, leg, etc.).
pub trait DiscountPolicy<T>: Send + Sync {
    /// Resolves the discount curve index for the visited target.
    fn accept(&self, target: &T) -> Result<MarketIndex>;

    /// Returns all discount curve indices referenced by this policy.
    fn discount_indices(&self) -> Vec<MarketIndex>;
}

/// Fixed-income discount policy.
///
/// Can prefer the instrument/leg index when available, or force risk-free by currency.
pub struct FixedIncomeDiscountPolicy {
    risk_free_by_currency: HashMap<Currency, MarketIndex>,
    prefer_instrument_index: bool,
}

impl FixedIncomeDiscountPolicy {
    /// Creates an empty fixed-income discount policy.
    #[must_use]
    pub fn new(prefer_instrument_index: bool) -> Self {
        Self {
            risk_free_by_currency: HashMap::new(),
            prefer_instrument_index,
        }
    }

    /// Adds or replaces a risk-free index mapping for a currency.
    #[must_use]
    pub fn with_risk_free_index(mut self, currency: Currency, market_index: MarketIndex) -> Self {
        self.risk_free_by_currency.insert(currency, market_index);
        self
    }

    fn risk_free_index(&self, currency: Currency) -> Option<MarketIndex> {
        self.risk_free_by_currency.get(&currency).cloned()
    }
}

impl DiscountPolicy<FixedRateDeposit> for FixedIncomeDiscountPolicy {
    fn accept(&self, target: &FixedRateDeposit) -> Result<MarketIndex> {
        if self.prefer_instrument_index {
            return Ok(target.market_index());
        }
        self.risk_free_index(target.currency()).ok_or_else(|| {
            AtlasError::NotFoundErr(format!(
                "No risk-free discount index configured for currency {}",
                target.currency()
            ))
        })
    }

    fn discount_indices(&self) -> Vec<MarketIndex> {
        self.risk_free_by_currency.values().cloned().collect()
    }
}

/// CSA discount policy.
///
/// Maps cashflow currency into CSA discount curve and supports a default index.
pub struct CSADiscountPolicy {
    default_index: MarketIndex,
    by_currency: HashMap<Currency, MarketIndex>,
}

impl CSADiscountPolicy {
    /// Creates a CSA discount policy with a default curve.
    #[must_use]
    pub fn new(default_index: MarketIndex) -> Self {
        Self {
            default_index,
            by_currency: HashMap::new(),
        }
    }

    /// Adds or replaces a currency-specific CSA discount curve mapping.
    #[must_use]
    pub fn with_currency(
        mut self,
        cashflow_currency: Currency,
        discount_index: MarketIndex,
    ) -> Self {
        self.by_currency.insert(cashflow_currency, discount_index);
        self
    }

    fn resolve_for_currency(&self, cashflow_currency: Currency) -> MarketIndex {
        self.by_currency
            .get(&cashflow_currency)
            .cloned()
            .unwrap_or_else(|| self.default_index.clone())
    }
}

impl DiscountPolicy<CrossCurrencySwap> for CSADiscountPolicy {
    fn accept(&self, _target: &CrossCurrencySwap) -> Result<MarketIndex> {
        Ok(self.default_index.clone())
    }

    fn discount_indices(&self) -> Vec<MarketIndex> {
        let mut unique = vec![self.default_index.clone()];
        for index in self.by_currency.values() {
            if !unique.iter().any(|idx| idx == index) {
                unique.push(index.clone());
            }
        }
        unique
    }
}

impl DiscountPolicy<EquityEuropeanOption> for CSADiscountPolicy {
    fn accept(&self, target: &EquityEuropeanOption) -> Result<MarketIndex> {
        Ok(self.resolve_for_currency(*target.currency()))
    }

    fn discount_indices(&self) -> Vec<MarketIndex> {
        let mut unique = vec![self.default_index.clone()];
        for index in self.by_currency.values() {
            if !unique.iter().any(|idx| idx == index) {
                unique.push(index.clone());
            }
        }
        unique
    }
}

impl DiscountPolicy<CapletFloorlet> for CSADiscountPolicy {
    fn accept(&self, _target: &CapletFloorlet) -> Result<MarketIndex> {
        Ok(self.default_index.clone())
    }

    fn discount_indices(&self) -> Vec<MarketIndex> {
        let mut unique = vec![self.default_index.clone()];
        for index in self.by_currency.values() {
            if !unique.iter().any(|idx| idx == index) {
                unique.push(index.clone());
            }
        }
        unique
    }
}

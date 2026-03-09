use std::collections::HashMap;

use crate::{currencies::currency::Currency, indices::marketindex::MarketIndex};
use crate::{
    instruments::fixedincome::fixedratedeposit::FixedRateDeposit,
    utils::errors::{QSError, Result},
};

/// Trait for types that have an associated currency.
pub trait HasCurrency {
    /// Returns the currency associated with this type.
    #[must_use]
    fn currency(&self) -> Currency;
}

/// Generic visitor-style discount policy.
///
/// The generic parameter `T` is the visited type (instrument, leg, etc.).
pub trait DiscountPolicy<T: HasCurrency>: Send + Sync {
    /// Resolves the discount curve index for the visited target.
    ///
    /// # Errors
    /// Returns an error if the discount index cannot be resolved.
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
            QSError::NotFoundErr(format!(
                "No risk-free discount index configured for currency {}",
                target.currency()
            ))
        })
    }

    fn discount_indices(&self) -> Vec<MarketIndex> {
        self.risk_free_by_currency.values().cloned().collect()
    }
}

/// CSA discount policy that uses a single collateral discount curve.
///
/// For legs in the same currency as the collateral, the stored discount index
/// is returned. For cross-currency legs, a [`MarketIndex::Collateral`] index
/// is returned to request a collateral-adjusted discount curve.
pub struct SingleCurveCSADiscountPolicy {
    discount_index: MarketIndex,
    currency: Currency,
}

impl SingleCurveCSADiscountPolicy {
    /// Creates a new [`SingleCurveCSADiscountPolicy`].
    #[must_use]
    pub const fn new(discount_index: MarketIndex, currency: Currency) -> Self {
        Self {
            discount_index,
            currency,
        }
    }
}

impl<T> DiscountPolicy<T> for SingleCurveCSADiscountPolicy
where
    T: HasCurrency,
{
    fn accept(&self, target: &T) -> Result<MarketIndex> {
        // we need to check if we have a currency mismatch, if so, we need
        // to check for Collateral() curves, otherwise we return the stored index
        if target.currency() == self.currency {
            Ok(self.discount_index.clone())
        } else {
            Ok(MarketIndex::Collateral(target.currency(), self.currency))
        }
    }

    fn discount_indices(&self) -> Vec<MarketIndex> {
        vec![self.discount_index.clone()]
    }
}

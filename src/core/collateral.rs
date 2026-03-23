use crate::core::instrument::AssetClass;
use crate::utils::errors::{QSError, Result};
use crate::{currencies::currency::Currency, indices::marketindex::MarketIndex};
use std::collections::HashMap;

/// A [`Discountable`] represents a type for which a discount curve can be resolved.
pub trait Discountable {
    /// Returns the asset class of the discountable type.
    #[must_use]
    fn asset_class(&self) -> AssetClass;

    /// Returns the market indices associated with this type, if any. For derivatives, discounting should be resolved via a discounting policy.
    #[must_use]
    fn discount_index(&self) -> Option<MarketIndex> {
        None
    }

    /// Returns the currency associated with this type.
    #[must_use]
    fn currency(&self) -> Currency;
}

/// Generic visitor-style discount policy.
///
/// A discount policy defines how to resolve the discount curve index for a given target
/// `T` (instrument, leg, etc.) and provides a list of all referenced discount indices for
/// bootstrapping purposes.
///
/// At least, `T` must implement [`HasCurrency`] get the currency of the target to
/// determine the appropriate discount curve.
pub trait DiscountPolicy: Send + Sync {
    /// Resolves the discount curve index for the visited target.
    ///
    /// # Errors
    /// Returns an error if the discount index cannot be resolved.
    fn accept(&self, target: &dyn Discountable) -> Result<MarketIndex>;

    /// Returns all discount curve indices referenced by this policy.
    fn discount_indices(&self) -> Vec<MarketIndex>;
}

/// Fixed-income discount policy.
///
/// For fixed-income instruments, we typically want to use risk-free discount curves or issuer-related curves. This policy allows configuring a
/// mapping of risk-free indices by currency, and optionally preferring the instrument's own index if available.
#[derive(Default)]
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

    /// Checks if there is a risk-free index associated with the currency.
    #[must_use]
    fn risk_free_index(&self, currency: Currency) -> Option<MarketIndex> {
        self.risk_free_by_currency.get(&currency).cloned()
    }
}

impl DiscountPolicy for FixedIncomeDiscountPolicy {
    fn accept(&self, target: &dyn Discountable) -> Result<MarketIndex> {
        match target.asset_class() {
            AssetClass::FixedIncome => {
                if self.prefer_instrument_index {
                    if let Some(indices) = target.discount_index() {
                        return Ok(indices);
                    }
                }
                self.risk_free_index(target.currency()).map_or_else(
                    || {
                        Err(QSError::InvalidValueErr(format!(
                            "No risk-free index configured for currency {}",
                            target.currency()
                        )))
                    },
                    Ok,
                )
            }
            _ => Err(QSError::InvalidValueErr(format!(
                "Unsupported asset class for FixedIncomeDiscountPolicy: {:?}",
                target.asset_class()
            ))),
        }
    }

    fn discount_indices(&self) -> Vec<MarketIndex> {
        self.risk_free_by_currency.values().cloned().collect()
    }
}

/// Single curve CSA discount policy that uses a collateral discount curve.
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

impl DiscountPolicy for SingleCurveCSADiscountPolicy {
    fn accept(&self, target: &dyn Discountable) -> Result<MarketIndex> {
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

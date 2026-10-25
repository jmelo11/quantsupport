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
/// At least, `T` must implement [`Discountable`] get the currency of the target to
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
/// Maps currencies to their chosen discount-curve indices and optionally
/// prefers the instrument's own discount index when one is available.
#[derive(Default)]
pub struct FixedIncomeDiscountPolicy {
    discount_curve_by_currency: HashMap<Currency, MarketIndex>,
    prefer_instrument_index: bool,
}

impl FixedIncomeDiscountPolicy {
    /// Creates an empty fixed-income discount policy.
    #[must_use]
    pub fn new(prefer_instrument_index: bool) -> Self {
        Self {
            discount_curve_by_currency: HashMap::new(),
            prefer_instrument_index,
        }
    }

    /// Adds or replaces the discount-curve index selected for a currency.
    #[must_use]
    pub fn with_discount_curve_index(mut self, currency: Currency, market_index: MarketIndex) -> Self {
        self.discount_curve_by_currency.insert(currency, market_index);
        self
    }

    /// Returns the discount-curve index configured for the currency.
    #[must_use]
    fn discount_curve_index(&self, currency: Currency) -> Option<MarketIndex> {
        self.discount_curve_by_currency.get(&currency).cloned()
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
                self.discount_curve_index(target.currency()).ok_or_else(|| {
                    QSError::InvalidValueErr(format!(
                        "No risk-free index configured for currency {}",
                        target.currency()
                    ))
                })
            }
            _ => Err(QSError::InvalidValueErr(format!(
                "Unsupported asset class for FixedIncomeDiscountPolicy: {:?}",
                target.asset_class()
            ))),
        }
    }

    fn discount_indices(&self) -> Vec<MarketIndex> {
        self.discount_curve_by_currency.values().cloned().collect()
    }
}

/// Single curve CSA discount policy that uses a collateral discount curve.
///
/// For legs in the same currency as the collateral, the stored discount index
/// is returned. For cross-currency legs, a [`MarketIndex::Collateral`] index
/// is returned to request a collateral-adjusted discount curve.
#[derive(Clone)]
pub struct SingleCurveCSADiscountPolicy {
    discount_index: MarketIndex,
    base_currency: Currency,
}

impl SingleCurveCSADiscountPolicy {
    /// Returns the collateral remuneration index.
    #[must_use]
    pub const fn collateral_index(&self) -> &MarketIndex {
        &self.discount_index
    }
    /// Returns the base currency used for collateral and monetary reporting.
    #[must_use]
    pub const fn base_currency(&self) -> Currency {
        self.base_currency
    }

    /// Creates a new [`SingleCurveCSADiscountPolicy`].
    #[must_use]
    pub const fn new(discount_index: MarketIndex, base_currency: Currency) -> Self {
        Self {
            discount_index,
            base_currency,
        }
    }
}

impl DiscountPolicy for SingleCurveCSADiscountPolicy {
    fn accept(&self, target: &dyn Discountable) -> Result<MarketIndex> {
        // we need to check if we have a currency mismatch, if so, we need
        // to check for Collateral() curves, otherwise we return the stored index
        if target.currency() == self.base_currency {
            Ok(self.discount_index.clone())
        } else {
            Ok(MarketIndex::Collateral(target.currency(), self.base_currency))
        }
    }

    fn discount_indices(&self) -> Vec<MarketIndex> {
        vec![self.discount_index.clone()]
    }
}

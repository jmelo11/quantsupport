use std::cell::{Ref, RefMut};

use crate::{
    ad::adreal::ADReal,
    core::{marketdatahandling::constructedelementstore::SharedElement, pillars::Pillars},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
};

/// Trait representing a curve element that can be used in automatic
/// differentiation contexts. It combines the properties of an interest rates
/// term structure and pillars, and allows for cloning.
pub trait ADCurveElement:
    InterestRatesTermStructure<ADReal> + Pillars<ADReal> + Send + Sync
{
}

/// Struct representing a discount curve element, which includes
/// the associated market index, currency, and the curve itself.
#[derive(Clone)]
pub struct DiscountCurveElement {
    market_index: MarketIndex,
    currency: Currency,
    curve: SharedElement<dyn ADCurveElement>,
}

impl DiscountCurveElement {
    /// Creates a new [`DiscountCurveElement`] with the specified market index, currency, and curve.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        currency: Currency,
        curve: SharedElement<dyn ADCurveElement>,
    ) -> Self {
        Self {
            market_index,
            currency,
            curve,
        }
    }

    /// Returns the market index associated with the discount curve element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the currency associated with the discount curve element.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Returns a reference to the curve associated with the discount curve element.
    #[must_use]
    pub fn curve(&self) -> Ref<'_, dyn ADCurveElement> {
        self.curve.borrow()
    }

    /// Returns a mutable reference to the curve associated with the discount curve element.
    #[must_use]
    pub fn curve_mut(&mut self) -> RefMut<'_, dyn ADCurveElement> {
        self.curve.borrow_mut()
    }
}

/// Struct representing a dividend curve element, which includes
/// the associated market index, currency, and the curve itself.
#[derive(Clone)]
pub struct DividendCurveElement {
    market_index: MarketIndex,
    currency: Currency,
    curve: SharedElement<dyn ADCurveElement>,
}

impl DividendCurveElement {
    /// Creates a new [`DividendCurveElement`] with the specified market index, currency, and curve.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        currency: Currency,
        curve: SharedElement<dyn ADCurveElement>,
    ) -> Self {
        Self {
            market_index,
            currency,
            curve,
        }
    }

    /// Returns the market index associated with the dividend curve element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the currency associated with the dividend curve element.
    #[must_use]
    pub const fn currency(&self) -> &Currency {
        &self.currency
    }

    /// Returns a reference to the curve associated with the dividend curve element.
    #[must_use]
    pub fn curve(&self) -> Ref<'_, dyn ADCurveElement> {
        self.curve.borrow()
    }

    /// Returns a mutable reference to the curve associated with the dividend curve element.
    #[must_use]
    pub fn curve_mut(&mut self) -> RefMut<'_, dyn ADCurveElement> {
        self.curve.borrow_mut()
    }
}

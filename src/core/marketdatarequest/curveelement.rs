use crate::{
    ad::adreal::ADReal, core::pillars::Pillars, currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
};

/// `ADCurveElement`
///
/// Trait representing a curve element that can be used in automatic
/// differentiation contexts. It combines the properties of an interest rates
/// term structure and pillars, and allows for cloning.
pub trait ADCurveElement:
    InterestRatesTermStructure<ADReal> + Pillars<ADReal> + Send + Sync + ADCurveElementClone
{
}

impl Clone for Box<dyn ADCurveElement> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// `ADCurveElementClone`
///
/// Trait to enable cloning of boxed [`ADCurveElement`] objects.
pub trait ADCurveElementClone {
    /// Clones the boxed [`ADCurveElement`].
    fn clone_box(&self) -> Box<dyn ADCurveElement>;
}

impl<T> ADCurveElementClone for T
where
    T: 'static + ADCurveElement + Clone,
{
    fn clone_box(&self) -> Box<dyn ADCurveElement> {
        Box::new(self.clone())
    }
}

/// `DiscountCurveElement`
///
/// Struct representing a discount curve element, which includes
/// the associated market index, currency, and the curve itself.
#[derive(Clone)]
pub struct DiscountCurveElement {
    market_index: MarketIndex,
    currency: Currency,
    curve: Box<dyn ADCurveElement>,
}

impl DiscountCurveElement {
    /// Creates a new [`DiscountCurveElement`] with the specified market index, currency, and curve.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        currency: Currency,
        curve: Box<dyn ADCurveElement>,
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
    pub const fn currency(&self) -> &Currency {
        &self.currency
    }

    /// Returns a reference to the curve associated with the discount curve element.
    #[must_use]
    pub fn curve(&self) -> &dyn ADCurveElement {
        self.curve.as_ref()
    }

    /// Returns a mutable reference to the curve associated with the discount curve element.
    #[must_use]
    pub fn curve_mut(&mut self) -> &mut dyn ADCurveElement {
        self.curve.as_mut()
    }
}

/// `DividendCurveElement`
///
/// Struct representing a dividend curve element, which includes
/// the associated market index, currency, and the curve itself.
#[derive(Clone)]
pub struct DividendCurveElement {
    market_index: MarketIndex,
    currency: Currency,
    curve: Box<dyn ADCurveElement>,
}

impl DividendCurveElement {
    /// Creates a new [`DividendCurveElement`] with the specified market index, currency, and curve.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        currency: Currency,
        curve: Box<dyn ADCurveElement>,
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
    pub fn curve(&self) -> &dyn ADCurveElement {
        self.curve.as_ref()
    }

    /// Returns a mutable reference to the curve associated with the dividend curve element.
    #[must_use]
    pub fn curve_mut(&mut self) -> &mut dyn ADCurveElement {
        self.curve.as_mut()
    }
}

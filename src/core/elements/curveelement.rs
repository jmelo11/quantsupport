use std::cell::{Ref, RefMut};

use crate::{
    ad::dual::DualFwd,
    core::{marketdatahandling::constructedelementstore::SharedElement, pillars::Pillars},
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::Interpolator,
    rates::{
        compounding::Compounding,
        yieldtermstructure::{
            discounttermstructure::DiscountTermStructure,
            interestratestermstructure::InterestRatesTermStructure,
        },
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};

/// Trait representing a curve element that can be used in automatic
/// differentiation contexts. It combines the properties of an interest rates
/// term structure and pillars, and allows for cloning.
pub trait ADCurveElement:
    InterestRatesTermStructure<DualFwd> + Pillars<DualFwd> + Send + Sync
{
    /// Copies a curve for use as an owned component of another term structure.
    ///
    /// # Errors
    /// Returns an error when the concrete curve does not support owned copies.
    fn clone_curve(&self) -> Result<Box<dyn ADCurveElement>> {
        Err(QSError::InvalidValueErr(
            "This curve does not support owned composition".into(),
        ))
    }

    /// Returns the IFT sensitivity matrix if available.
    fn ift_sensitivities(&self) -> Option<&[Vec<f64>]> {
        None
    }
}

impl InterestRatesTermStructure<DualFwd> for Box<dyn ADCurveElement> {
    fn reference_date(&self) -> Date {
        self.as_ref().reference_date()
    }
    fn discount_factor(&self, date: Date) -> Result<DualFwd> {
        self.as_ref().discount_factor(date)
    }
    fn forward_rate(
        &self,
        start: Date,
        end: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<DualFwd> {
        self.as_ref().forward_rate(start, end, comp, freq)
    }
    fn nodes(&self) -> Option<Vec<(Date, DualFwd)>> {
        self.as_ref().nodes()
    }
    fn day_counter(&self) -> Option<DayCounter> {
        self.as_ref().day_counter()
    }
    fn discount_factor_from_time(&self, time: f64) -> Result<DualFwd> {
        self.as_ref().discount_factor_from_time(time)
    }
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<DualFwd> {
        self.as_ref().forward_rate_from_time(start, end)
    }
}

impl Pillars<DualFwd> for Box<dyn ADCurveElement> {
    fn pillars(&self) -> Option<Vec<(String, &DualFwd)>> {
        self.as_ref().pillars()
    }
    fn pillar_labels(&self) -> Option<Vec<String>> {
        self.as_ref().pillar_labels()
    }
    fn put_pillars_on_tape(&mut self) {
        self.as_mut().put_pillars_on_tape();
    }
}

impl ADCurveElement for Box<dyn ADCurveElement> {
    fn clone_curve(&self) -> Result<Box<dyn ADCurveElement>> {
        self.as_ref().clone_curve()
    }
    fn ift_sensitivities(&self) -> Option<&[Vec<f64>]> {
        self.as_ref().ift_sensitivities()
    }
}

/// Struct representing a discount curve element, which includes
/// the associated market index, currency, and the curve itself.
#[derive(Clone)]
pub struct DiscountCurveElement {
    market_index: MarketIndex,
    curve: SharedElement<dyn ADCurveElement>,
}

impl DiscountCurveElement {
    /// Creates a new [`DiscountCurveElement`] with the specified market index, currency, and curve.
    #[must_use]
    pub const fn new(market_index: MarketIndex, curve: SharedElement<dyn ADCurveElement>) -> Self {
        Self {
            market_index,
            curve,
        }
    }

    /// Returns the market index associated with the discount curve element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns a reference to the curve associated with the discount curve element.
    #[must_use]
    pub fn curve(&self) -> Ref<'_, dyn ADCurveElement> {
        self.curve.borrow()
    }

    /// Extracts a plain `f64` discount term structure from the AD-enabled
    /// curve nodes. Useful for feeding models and simulations that operate on
    /// `f64` (e.g. Hull-White calibration and Monte Carlo path generation).
    ///
    /// # Errors
    /// Returns an error if the curve exposes no nodes or if the term
    /// structure cannot be constructed.
    pub fn to_f64_term_structure(
        &self,
        day_counter: DayCounter,
    ) -> Result<DiscountTermStructure<f64>> {
        let curve = self.curve();
        let nodes = curve
            .nodes()
            .ok_or_else(|| QSError::InvalidValueErr("Curve has no nodes".into()))?;
        let (dates, dfs): (Vec<_>, Vec<f64>) =
            nodes.into_iter().map(|(d, df)| (d, df.value())).unzip();
        DiscountTermStructure::<f64>::new(dates, dfs, day_counter, Interpolator::LogLinear, true)
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
    curve: SharedElement<dyn ADCurveElement>,
}

impl DividendCurveElement {
    /// Creates a new [`DividendCurveElement`] with the specified market index, currency, and curve.
    #[must_use]
    pub const fn new(market_index: MarketIndex, curve: SharedElement<dyn ADCurveElement>) -> Self {
        Self {
            market_index,
            curve,
        }
    }

    /// Returns the market index associated with the dividend curve element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
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

/// Struct representing a credit (survival) curve element.
///
/// Survival probabilities are exposed through the standard term-structure
/// interface: `discount_factor(d)` returns the survival probability `S(d)`.
#[derive(Clone)]
pub struct CreditCurveElement {
    market_index: MarketIndex,
    recovery: f64,
    curve: SharedElement<dyn ADCurveElement>,
}

impl CreditCurveElement {
    /// Creates a new [`CreditCurveElement`] with the specified market index,
    /// recovery rate, and survival curve.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        recovery: f64,
        curve: SharedElement<dyn ADCurveElement>,
    ) -> Self {
        Self {
            market_index,
            recovery,
            curve,
        }
    }

    /// Returns the market index associated with the credit curve element.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the recovery rate assumed when the curve was bootstrapped.
    #[must_use]
    pub const fn recovery(&self) -> f64 {
        self.recovery
    }

    /// Returns a reference to the survival curve.
    #[must_use]
    pub fn curve(&self) -> Ref<'_, dyn ADCurveElement> {
        self.curve.borrow()
    }

    /// Returns a mutable reference to the survival curve.
    #[must_use]
    pub fn curve_mut(&mut self) -> RefMut<'_, dyn ADCurveElement> {
        self.curve.borrow_mut()
    }
}

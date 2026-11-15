use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use crate::{
    ad::{scalar::Scalar, Recorded},
    core::{elements::curveelement::ADCurveElement, pillars::Pillars},
    rates::{
        compounding::Compounding, interestrate::InterestRate,
        yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::Result,
};

/// A term structure built by combining a spread curve and a base curve.
///
/// Discount factors are composed multiplicatively:
/// $$
///    \text{df}_{\text{composite}}(t) = \text{df}_{\text{spread}}(t) \cdot \text{df}_{\text{base}}(t)
/// $$
///
/// Forward rates are derived from the composite discount factors via
/// [`InterestRate::implied_rate`], ensuring correctness regardless of
/// the compounding convention.
pub struct CompositeTermStructure<T: Scalar, C = Rc<RefCell<dyn InterestRatesTermStructure<T>>>> {
    reference_date: Date,
    spread_curve: C,
    base_curve: C,
    scalar: PhantomData<T>,
}

impl<T: Scalar> CompositeTermStructure<T> {
    /// Creates a new `CompositeTermStructure` by combining a spread curve and a base curve.
    ///
    /// The reference date is taken from the base curve.
    pub fn new(
        spread_curve: Rc<RefCell<dyn InterestRatesTermStructure<T>>>,
        base_curve: Rc<RefCell<dyn InterestRatesTermStructure<T>>>,
    ) -> Self {
        Self::from_curves(spread_curve, base_curve)
    }
}

impl<T: Scalar, C: InterestRatesTermStructure<T>> CompositeTermStructure<T, C> {
    /// Composes curves held by value, including boxed AD curve elements.
    ///
    /// The reference date and day counter come from the base curve. Components
    /// must share these conventions when using time-based curve methods.
    pub fn from_curves(spread_curve: C, base_curve: C) -> Self {
        let reference_date = base_curve.reference_date();
        Self {
            reference_date,
            spread_curve,
            base_curve,
            scalar: PhantomData,
        }
    }
}

impl<C: InterestRatesTermStructure<f64>> InterestRatesTermStructure<f64>
    for CompositeTermStructure<f64, C>
{
    fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Returns the discount factor at a date under the curve conventions.
    fn discount_factor(&self, date: Date) -> Result<f64> {
        let spread_df = self.spread_curve.discount_factor(date)?;
        let base_df = self.base_curve.discount_factor(date)?;
        Ok(spread_df * base_df)
    }

    fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<f64> {
        let df_start = self.discount_factor(start_date)?;
        let df_end = self.discount_factor(end_date)?;
        let comp_factor = df_start / df_end;
        let dc = self.day_counter().unwrap_or(DayCounter::Actual365);
        let t = dc.year_fraction(start_date, end_date);
        Ok(InterestRate::<f64>::implied_rate(comp_factor, dc, comp, freq, t)?.rate())
    }

    /// Returns the curve nodes in their interpolation order.
    fn nodes(&self) -> Option<Vec<(Date, f64)>> {
        None
    }

    fn day_counter(&self) -> Option<DayCounter> {
        self.base_curve.day_counter()
    }

    /// Returns the discount factor for a day-count year fraction.
    fn discount_factor_from_time(&self, t: f64) -> Result<f64> {
        let spread_df = self.spread_curve.discount_factor_from_time(t)?;
        let base_df = self.base_curve.discount_factor_from_time(t)?;
        Ok(spread_df * base_df)
    }

    /// Returns the forward rate between two day-count year fractions.
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<f64> {
        let dt = end - start;
        let (s, e, dt) = if dt.abs() < 1e-10 {
            let eps = 1e-6;
            (start, start + eps, eps)
        } else {
            (start, end, dt)
        };
        let df_start = self.discount_factor_from_time(s)?;
        let df_end = self.discount_factor_from_time(e)?;
        let fwd = (df_start / df_end - 1.0) / dt;
        Ok(fwd)
    }
}

impl<C: InterestRatesTermStructure<Recorded>> InterestRatesTermStructure<Recorded>
    for CompositeTermStructure<Recorded, C>
{
    fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Returns the discount factor at a date under the curve conventions.
    fn discount_factor(&self, date: Date) -> Result<Recorded> {
        let spread_df = self.spread_curve.discount_factor(date)?;
        let base_df = self.base_curve.discount_factor(date)?;
        Ok(spread_df * base_df)
    }

    fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<Recorded> {
        let df_start = self.discount_factor(start_date)?;
        let df_end = self.discount_factor(end_date)?;
        let comp_factor = df_start / df_end;
        let dc = self.day_counter().unwrap_or(DayCounter::Actual365);
        let t = dc.year_fraction(start_date, end_date);
        Ok(InterestRate::<Recorded>::implied_rate(comp_factor, dc, comp, freq, t)?.rate())
    }

    /// Returns the curve nodes in their interpolation order.
    fn nodes(&self) -> Option<Vec<(Date, Recorded)>> {
        None
    }

    fn day_counter(&self) -> Option<DayCounter> {
        self.base_curve.day_counter()
    }

    /// Returns the discount factor for a day-count year fraction.
    fn discount_factor_from_time(&self, t: f64) -> Result<Recorded> {
        let spread_df = self.spread_curve.discount_factor_from_time(t)?;
        let base_df = self.base_curve.discount_factor_from_time(t)?;
        Ok(spread_df * base_df)
    }

    /// Returns the forward rate between two day-count year fractions.
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<Recorded> {
        let dt = end - start;
        let (s, e, dt) = if dt.abs() < 1e-10 {
            let eps = 1e-6;
            (start, start + eps, eps)
        } else {
            (start, end, dt)
        };
        let df_start = self.discount_factor_from_time(s)?;
        let df_end = self.discount_factor_from_time(e)?;
        let fwd = (df_start / df_end - Recorded::one()) / Recorded::new(dt);
        Ok(fwd)
    }
}

impl<C: Pillars<Recorded>> Pillars<Recorded> for CompositeTermStructure<Recorded, C> {
    /// Returns the labelled inputs used to report market sensitivities.
    fn pillars(&self) -> Option<Vec<(String, &Recorded)>> {
        let mut pillars = self.base_curve.pillars().unwrap_or_default();
        pillars.extend(self.spread_curve.pillars().unwrap_or_default());
        Some(pillars)
    }

    fn pillar_labels(&self) -> Option<Vec<String>> {
        let mut labels = self.base_curve.pillar_labels().unwrap_or_default();
        labels.extend(self.spread_curve.pillar_labels().unwrap_or_default());
        Some(labels)
    }

    /// Registers current labelled market pillars as independent compiler inputs.
    fn register_pillars(&mut self) {
        self.base_curve.register_pillars();
        self.spread_curve.register_pillars();
    }
}

impl<C: ADCurveElement> ADCurveElement for CompositeTermStructure<Recorded, C> {
    fn clone_curve(&self) -> Result<Box<dyn ADCurveElement>> {
        Ok(Box::new(CompositeTermStructure::from_curves(
            self.spread_curve.clone_curve()?,
            self.base_curve.clone_curve()?,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rates::interestrate::RateDefinition;
    use crate::rates::yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure;

    #[test]
    fn composite_multiplicative_df() -> Result<()> {
        let ref_date = Date::new(2024, 1, 1);
        let spread = FlatForwardTermStructure::new(ref_date, 0.01, RateDefinition::default());
        let base = FlatForwardTermStructure::new(ref_date, 0.04, RateDefinition::default());

        let target = Date::new(2025, 1, 1);
        let expected_df = spread.discount_factor(target)? * base.discount_factor(target)?;

        let composite = CompositeTermStructure::<f64>::new(
            Rc::new(RefCell::new(spread)),
            Rc::new(RefCell::new(base)),
        );
        let actual_df = composite.discount_factor(target)?;
        assert!((actual_df - expected_df).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn composite_forward_rate_consistency() -> Result<()> {
        let ref_date = Date::new(2024, 1, 1);
        let spread = FlatForwardTermStructure::new(ref_date, 0.005, RateDefinition::default());
        let base = FlatForwardTermStructure::new(ref_date, 0.03, RateDefinition::default());

        let composite = CompositeTermStructure::<f64>::new(
            Rc::new(RefCell::new(spread)),
            Rc::new(RefCell::new(base)),
        );

        let start = Date::new(2024, 6, 1);
        let end = Date::new(2025, 6, 1);
        let fwd = composite.forward_rate(start, end, Compounding::Continuous, Frequency::Annual)?;

        // Forward rate close to 3.5% (base 3% + spread 0.5%); the slight
        // mismatch comes from the underlying Simple / Actual360 convention.
        assert!((fwd - 0.035).abs() < 5e-3);
        Ok(())
    }
}

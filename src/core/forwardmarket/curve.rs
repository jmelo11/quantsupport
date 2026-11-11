//! Exact temporary curve views used while generating horizon quotes.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    ad::dual::DualFwd,
    core::{
        elements::curveelement::{
            ADCurveElement, CreditCurveElement, DiscountCurveElement, DividendCurveElement,
        },
        marketdatahandling::constructedelementstore::ConstructedElementStore,
        pillars::Pillars,
        pricingcontext::PricingContext,
    },
    indices::marketindex::MarketIndex,
    rates::{
        bootstrapping::curveconfiguration::CurveConfiguration,
        compounding::Compounding,
        interestrate::InterestRate,
        yieldtermstructure::{
            compositetermstructure::CompositeTermStructure,
            interestratestermstructure::InterestRatesTermStructure,
        },
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};

use super::{
    advanceintime::AdvanceInTime,
    configuration::{CurveRoll, ForwardMarketConfiguration},
    source::ForwardMarketSource,
};

struct AdvancedCurve {
    original: Box<dyn ADCurveElement>,
    date: Date,
    roll: CurveRoll,
}

impl AdvancedCurve {
    fn new(curve: &dyn ADCurveElement, date: Date, roll: CurveRoll) -> Result<Self> {
        if date < curve.reference_date() {
            return Err(QSError::InvalidValueErr(
                "Cannot advance a curve backwards".into(),
            ));
        }
        Ok(Self {
            original: curve.clone_curve()?,
            date,
            roll,
        })
    }

    fn offset(&self) -> Result<f64> {
        Ok(self
            .counter()?
            .year_fraction(self.original.reference_date(), self.date))
    }

    fn counter(&self) -> Result<DayCounter> {
        self.original.day_counter().ok_or_else(|| {
            QSError::InvalidValueErr("Advancing a curve requires its day counter".into())
        })
    }
}

impl InterestRatesTermStructure<DualFwd> for AdvancedCurve {
    fn reference_date(&self) -> Date {
        self.date
    }

    fn discount_factor(&self, date: Date) -> Result<DualFwd> {
        if date < self.date {
            return Err(QSError::InvalidValueErr(format!(
                "Curve query {date} precedes horizon {}",
                self.date
            )));
        }
        if date == self.date {
            return Ok(DualFwd::one());
        }
        let value: DualFwd = match self.roll {
            CurveRoll::ImpliedForward => (self.original.discount_factor(date)?
                / self.original.discount_factor(self.date)?)
            .into(),
            CurveRoll::ConstantTenor => self
                .original
                .discount_factor_from_time(self.counter()?.year_fraction(self.date, date))?,
        };
        if !value.value().is_finite() || value.value() <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Advanced curve has a non-positive or non-finite factor".into(),
            ));
        }
        Ok(value)
    }

    fn forward_rate(
        &self,
        start: Date,
        end: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<DualFwd> {
        let counter = self.counter()?;
        let end = if start == end { end + 1 } else { end };
        let compound: DualFwd = (self.discount_factor(start)? / self.discount_factor(end)?).into();
        Ok(InterestRate::<DualFwd>::implied_rate(
            compound,
            counter,
            comp,
            freq,
            counter.year_fraction(start, end),
        )?
        .rate())
    }

    fn nodes(&self) -> Option<Vec<(Date, DualFwd)>> {
        let mut dates = vec![self.date];
        dates.extend(
            self.original
                .nodes()?
                .into_iter()
                .map(|(date, _)| date)
                .filter(|date| *date > self.date),
        );
        dates
            .into_iter()
            .map(|date| self.discount_factor(date).map(|factor| (date, factor)))
            .collect::<Result<_>>()
            .ok()
    }

    fn day_counter(&self) -> Option<DayCounter> {
        self.original.day_counter()
    }

    fn discount_factor_from_time(&self, time: f64) -> Result<DualFwd> {
        if !time.is_finite() || time < 0.0 {
            return Err(QSError::InvalidValueErr(
                "Forward curve time must be finite and non-negative".into(),
            ));
        }
        let value = match self.roll {
            CurveRoll::ImpliedForward => (self
                .original
                .discount_factor_from_time(self.offset()? + time)?
                / self.original.discount_factor(self.date)?)
            .into(),
            CurveRoll::ConstantTenor => self.original.discount_factor_from_time(time)?,
        };
        Ok(value)
    }

    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<DualFwd> {
        let end = if (start - end).abs() < f64::EPSILON {
            start + 1e-6
        } else {
            end
        };
        Ok(
            ((self.discount_factor_from_time(start)? / self.discount_factor_from_time(end)? - 1.0)
                / (end - start))
                .into(),
        )
    }
}

impl Pillars<DualFwd> for AdvancedCurve {
    fn pillars(&self) -> Option<Vec<(String, &DualFwd)>> {
        self.original.pillars()
    }
    fn pillar_labels(&self) -> Option<Vec<String>> {
        self.original.pillar_labels()
    }
    fn put_pillars_on_tape(&mut self) {
        self.original.put_pillars_on_tape();
    }
}

impl ADCurveElement for AdvancedCurve {
    fn clone_curve(&self) -> Result<Box<dyn ADCurveElement>> {
        Ok(Box::new(Self {
            original: self.original.clone_curve()?,
            date: self.date,
            roll: self.roll,
        }))
    }
}

impl AdvanceInTime for DiscountCurveElement {
    type Output = Self;
    fn advance_in_time(
        &self,
        config: &ForwardMarketConfiguration,
        _source: &ForwardMarketSource<'_>,
    ) -> Result<Self> {
        Ok(Self::new(
            self.market_index().clone(),
            Rc::new(RefCell::new(AdvancedCurve::new(
                &*self.curve(),
                config.target_date(),
                config.curve_roll(self.market_index()),
            )?)),
        ))
    }
}

impl AdvanceInTime for DividendCurveElement {
    type Output = Self;
    fn advance_in_time(
        &self,
        config: &ForwardMarketConfiguration,
        _source: &ForwardMarketSource<'_>,
    ) -> Result<Self> {
        Ok(Self::new(
            self.market_index().clone(),
            Rc::new(RefCell::new(AdvancedCurve::new(
                &*self.curve(),
                config.target_date(),
                config.curve_roll(self.market_index()),
            )?)),
        ))
    }
}

impl AdvanceInTime for CreditCurveElement {
    type Output = Self;
    fn advance_in_time(
        &self,
        config: &ForwardMarketConfiguration,
        _source: &ForwardMarketSource<'_>,
    ) -> Result<Self> {
        Ok(Self::new(
            self.market_index().clone(),
            config.recovery(self.market_index(), self.recovery()),
            Rc::new(RefCell::new(AdvancedCurve::new(
                &*self.curve(),
                config.target_date(),
                config.curve_roll(self.market_index()),
            )?)),
        ))
    }
}

/// Rebuilds composite curves from their advanced component curves.
pub struct CompositeAdvancer<'a> {
    source: &'a PricingContext,
    configuration: &'a ForwardMarketConfiguration,
}

impl<'a> CompositeAdvancer<'a> {
    /// Binds composite rebuilding to the source definitions and horizon rules.
    #[must_use]
    pub const fn new(
        source: &'a PricingContext,
        configuration: &'a ForwardMarketConfiguration,
    ) -> Self {
        Self {
            source,
            configuration,
        }
    }

    /// Rebuilds configured discount and dividend composites.
    ///
    /// # Errors
    /// Rejects missing components and circular curve dependencies.
    pub fn rebuild(&self, elements: &mut ConstructedElementStore) -> Result<()> {
        let source = self.source;
        self.rebuild_curve_map(
            elements.discount_curves_mut(),
            source.curve_configurations(),
        )?;
        let mut dividends = elements
            .dividend_curves()
            .iter()
            .map(|(index, element)| {
                Ok((
                    index.clone(),
                    DiscountCurveElement::new(
                        index.clone(),
                        Rc::new(RefCell::new(element.curve().clone_curve()?)),
                    ),
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;
        self.rebuild_curve_map(&mut dividends, source.dividend_curve_configurations())?;
        for (index, element) in dividends {
            elements.dividend_curves_mut().insert(
                index.clone(),
                DividendCurveElement::new(
                    index,
                    Rc::new(RefCell::new(element.curve().clone_curve()?)),
                ),
            );
        }
        Ok(())
    }

    fn rebuild_curve_map(
        &self,
        curves: &mut HashMap<MarketIndex, DiscountCurveElement>,
        specifications: &[CurveConfiguration],
    ) -> Result<()> {
        let mut states = HashMap::new();
        for spec in specifications {
            self.rebuild_composite(spec.market_index(), curves, specifications, &mut states)?;
        }
        Ok(())
    }

    fn rebuild_composite(
        &self,
        index: &MarketIndex,
        curves: &mut HashMap<MarketIndex, DiscountCurveElement>,
        specifications: &[CurveConfiguration],
        states: &mut HashMap<MarketIndex, bool>,
    ) -> Result<()> {
        if let Some(done) = states.get(index) {
            return if *done {
                Ok(())
            } else {
                Err(QSError::InvalidValueErr(format!(
                    "Circular forward curve dependency at {index}"
                )))
            };
        }
        states.insert(index.clone(), false);
        let components = specifications
            .iter()
            .find(|spec| spec.market_index() == index)
            .and_then(CurveConfiguration::composite_components);
        if let Some((base, spread)) =
            components.filter(|_| !self.configuration.has_curve_override(index))
        {
            self.rebuild_composite(base, curves, specifications, states)?;
            self.rebuild_composite(spread, curves, specifications, states)?;
            let component = |key: &MarketIndex| -> Result<Box<dyn ADCurveElement>> {
                curves
                    .get(key)
                    .ok_or_else(|| {
                        QSError::NotFoundErr(format!("Forward composite component {key}"))
                    })?
                    .curve()
                    .clone_curve()
            };
            let curve = CompositeTermStructure::from_curves(component(spread)?, component(base)?);
            curves.insert(
                index.clone(),
                DiscountCurveElement::new(index.clone(), Rc::new(RefCell::new(curve))),
            );
        }
        states.insert(index.clone(), true);
        Ok(())
    }
}

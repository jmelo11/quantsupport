use std::rc::Rc;

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{elements::curveelement::ADCurveElement, pillars::Pillars},
    math::interpolation::interpolator::Interpolator,
    rates::{compounding::Compounding, interestrate::RateDefinition},
    time::{date::Date, daycounter::DayCounter, enums::Frequency, period::Period},
    utils::errors::{QSError, Result},
};

use super::{
    interestratestermstructure::InterestRatesTermStructure,
    zeroratetermstructure::ZeroRateTermStructure,
};

/// A zero-rate curve whose pillars are specified as tenors from its reference date.
/// With `T = DualFwd`, it exposes the same AD curve interface as the dated curve.
#[derive(Clone, Debug)]
pub struct TenorBasedZeroRateTermStructure<T: Scalar = f64> {
    tenors: Vec<Period>,
    curve: ZeroRateTermStructure<T>,
}

impl<T: Scalar> TenorBasedZeroRateTermStructure<T> {
    /// Creates a tenor-based zero-rate curve.
    ///
    /// # Errors
    /// Rejects empty, mismatched, unordered or non-finite curve data and negative tenors.
    pub fn new(
        reference_date: Date,
        tenors: Vec<Period>,
        rates: Vec<T>,
        rate_definition: RateDefinition,
        interpolator: Interpolator,
        enable_extrapolation: bool,
    ) -> Result<Self> {
        let dates = tenors.iter().map(|tenor| reference_date + *tenor).collect();
        let curve = ZeroRateTermStructure::new(
            reference_date,
            dates,
            rates,
            rate_definition,
            interpolator,
            enable_extrapolation,
        )?;
        Ok(Self { tenors, curve })
    }

    /// Returns the pillar tenors.
    #[must_use]
    pub const fn tenors(&self) -> &Vec<Period> {
        &self.tenors
    }

    /// Returns the pillar zero rates.
    #[must_use]
    pub const fn rates(&self) -> &Vec<T> {
        self.curve.rates()
    }

    /// Returns the underlying dated zero-rate curve.
    #[must_use]
    pub const fn curve(&self) -> &ZeroRateTermStructure<T> {
        &self.curve
    }

    /// Labels the zero-rate pillars for sensitivity reports.
    ///
    /// # Errors
    /// Returns an error unless there is one label per zero-rate pillar.
    pub fn with_pillar_labels(mut self, labels: Vec<String>) -> Result<Self> {
        self.curve = self.curve.with_pillar_labels(labels)?;
        Ok(self)
    }
}

impl<T: Scalar + 'static> TenorBasedZeroRateTermStructure<T>
where
    ZeroRateTermStructure<T>: InterestRatesTermStructure<T>,
{
    /// Rolls the reference date while preserving the tenors and zero rates.
    ///
    /// # Errors
    /// Returns an error for backward rolls or invalid shifted pillar dates.
    pub fn advance_to_period(
        &self,
        period: Period,
    ) -> Result<Rc<dyn InterestRatesTermStructure<T>>> {
        self.advance_to_date(self.reference_date() + period)
    }

    /// Rolls the reference date while preserving the tenors and zero rates.
    ///
    /// # Errors
    /// Returns an error for backward rolls or invalid shifted pillar dates.
    pub fn advance_to_date(&self, date: Date) -> Result<Rc<dyn InterestRatesTermStructure<T>>> {
        if date < self.reference_date() {
            return Err(QSError::InvalidValueErr(
                "Cannot roll a zero curve backwards".into(),
            ));
        }
        let mut curve = Self::new(
            date,
            self.tenors.clone(),
            self.rates().clone(),
            self.curve.rate_definition(),
            self.curve.interpolator(),
            self.curve.enable_extrapolation(),
        )?;
        if let Some(labels) = self.curve.labels() {
            curve = curve.with_pillar_labels(labels.to_vec())?;
        }
        Ok(Rc::new(curve))
    }
}

impl<T: Scalar> InterestRatesTermStructure<T> for TenorBasedZeroRateTermStructure<T>
where
    ZeroRateTermStructure<T>: InterestRatesTermStructure<T>,
{
    fn reference_date(&self) -> Date {
        self.curve.reference_date()
    }
    fn discount_factor(&self, date: Date) -> Result<T> {
        self.curve.discount_factor(date)
    }
    fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<T> {
        self.curve.forward_rate(start_date, end_date, comp, freq)
    }
    fn nodes(&self) -> Option<Vec<(Date, T)>> {
        self.curve.nodes()
    }
    fn day_counter(&self) -> Option<DayCounter> {
        Some(self.curve.rate_definition().day_counter())
    }
    fn discount_factor_from_time(&self, time: f64) -> Result<T> {
        self.curve.discount_factor_from_time(time)
    }
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<T> {
        self.curve.forward_rate_from_time(start, end)
    }
}

impl TenorBasedZeroRateTermStructure<f64> {
    /// Converts the zero rates to AD values while preserving tenors and labels.
    #[must_use]
    pub fn to_dual(&self) -> TenorBasedZeroRateTermStructure<DualFwd> {
        TenorBasedZeroRateTermStructure {
            tenors: self.tenors.clone(),
            curve: self.curve.to_dual(),
        }
    }
}

impl Pillars<DualFwd> for TenorBasedZeroRateTermStructure<DualFwd> {
    fn pillars(&self) -> Option<Vec<(String, &DualFwd)>> {
        self.curve.pillars()
    }

    fn pillar_labels(&self) -> Option<Vec<String>> {
        self.curve.pillar_labels()
    }

    fn put_pillars_on_tape(&mut self) {
        self.curve.put_pillars_on_tape();
    }
}

impl ADCurveElement for TenorBasedZeroRateTermStructure<DualFwd> {
    fn clone_curve(&self) -> Result<Box<dyn ADCurveElement>> {
        self.curve.clone_curve()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ad::tape::Tape;
    use crate::time::enums::TimeUnit;

    #[test]
    fn tenor_curve_ad_and_roll_match_finite_differences() -> Result<()> {
        let reference = Date::new(2026, 1, 1);
        let tenors = vec![
            Period::new(0, TimeUnit::Days),
            Period::new(1, TimeUnit::Years),
            Period::new(2, TimeUnit::Years),
        ];
        let rates = vec![0.01, 0.02, 0.03];
        let labels = vec!["0D".into(), "1Y".into(), "2Y".into()];
        let start = reference
            .advance(6, TimeUnit::Months)
            .expect("valid date advancement");
        let end = reference
            .advance(18, TimeUnit::Months)
            .expect("valid date advancement");
        let make_curve = |rates| {
            TenorBasedZeroRateTermStructure::new(
                reference,
                tenors.clone(),
                rates,
                RateDefinition::default(),
                Interpolator::Linear,
                true,
            )
        };
        let curve = make_curve(rates.clone())?.with_pillar_labels(labels.clone())?;
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
        let mut dual = curve.to_dual();
        Tape::start_recording_fwd();
        dual.put_pillars_on_tape();
        let element: &dyn ADCurveElement = &dual;
        assert_eq!(element.pillar_labels(), Some(labels));
        assert_eq!(
            element.pillars().map(|pillars| pillars.len()),
            Some(rates.len())
        );
        let outputs = [
            element.discount_factor(end)?,
            element.forward_rate(start, end, Compounding::Simple, Frequency::Annual)?,
            dual.advance_to_date(start)?.discount_factor(end)?,
        ];
        for (output_index, output) in outputs.iter().enumerate() {
            Tape::reset_adjoints_fwd();
            let _ = output.backward();
            let evaluate = |rates| -> Result<f64> {
                let curve = make_curve(rates)?;
                Ok([
                    curve.discount_factor(end)?,
                    curve.forward_rate(start, end, Compounding::Simple, Frequency::Annual)?,
                    curve.advance_to_date(start)?.discount_factor(end)?,
                ][output_index])
            };
            assert!((output.value() - evaluate(rates.clone())?).abs() < 1e-12);
            for (index, pillar) in dual.rates().iter().enumerate() {
                let mut plus = rates.clone();
                let mut minus = rates.clone();
                plus[index] += 1e-6;
                minus[index] -= 1e-6;
                let finite_difference = (evaluate(plus)? - evaluate(minus)?) / 2e-6;
                assert!((pillar.adjoint()? - finite_difference).abs() < 1e-7);
            }
        }
        assert!(dual
            .advance_to_date(
                reference
                    .advance(-1, TimeUnit::Days)
                    .expect("valid date advancement")
            )
            .is_err());
        assert_eq!(element.discount_factor(reference)?.value(), 1.0);
        assert_eq!(element.nodes().map(|nodes| nodes.len()), Some(rates.len()));
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
        Ok(())
    }

    #[test]
    fn tenor_zero_curve_matches_dated_curve_and_interval_forward() -> Result<()> {
        let reference = Date::new(2026, 1, 1);
        let curve = TenorBasedZeroRateTermStructure::new(
            reference,
            vec![
                Period::new(1, TimeUnit::Years),
                Period::new(2, TimeUnit::Years),
            ],
            vec![0.02, 0.03],
            RateDefinition::default(),
            Interpolator::Linear,
            true,
        )?;
        let start = reference + Period::new(1, TimeUnit::Years);
        let end = reference + Period::new(2, TimeUnit::Years);
        let expected = (curve.discount_factor(start)? / curve.discount_factor(end)? - 1.0)
            / DayCounter::Actual360.year_fraction(start, end);
        assert!(
            (curve.forward_rate(start, end, Compounding::Simple, Frequency::Annual)? - expected)
                .abs()
                < 1e-12
        );
        assert!(
            (curve.forward_rate(reference, start, Compounding::Simple, Frequency::Annual)? - 0.02)
                .abs()
                < 1e-12
        );
        let rolled = curve.advance_to_date(start)?;
        assert_eq!(rolled.reference_date(), start);
        assert!(curve
            .advance_to_period(Period::new(-1, TimeUnit::Days))
            .is_err());
        assert!(TenorBasedZeroRateTermStructure::<f64>::new(
            reference,
            vec![],
            vec![],
            RateDefinition::default(),
            Interpolator::Linear,
            true
        )
        .is_err());
        Ok(())
    }
}

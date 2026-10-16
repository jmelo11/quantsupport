use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{elements::curveelement::ADCurveElement, pillars::Pillars},
    math::interpolation::interpolator::{Interpolate, Interpolator},
    rates::{
        compounding::Compounding,
        interestrate::{InterestRate, RateDefinition},
    },
    time::{
        date::Date,
        daycounter::DayCounter,
        enums::{Frequency, TimeUnit},
        period::Period,
    },
    utils::errors::{QSError, Result},
};

use super::interestratestermstructure::InterestRatesTermStructure;

/// A dated zero-rate curve with explicit rate conventions and interpolation.
/// With `T = DualFwd`, zero rates retain their AD links and expose labeled
/// sensitivities through the [`Pillars`] trait.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    try_from = "ZeroRateCurveData<T>",
    into = "ZeroRateCurveData<T>",
    bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>")
)]
pub struct ZeroRateTermStructure<T: Scalar = f64> {
    reference_date: Date,
    dates: Vec<Date>,
    rates: Vec<T>,
    year_fractions: Vec<f64>,
    rate_definition: RateDefinition,
    interpolator: Interpolator,
    enable_extrapolation: bool,
    pillar_labels: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
struct ZeroRateCurveData<T> {
    reference_date: Date,
    dates: Vec<Date>,
    rates: Vec<T>,
    rate_definition: RateDefinition,
    interpolator: Interpolator,
    enable_extrapolation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pillar_labels: Option<Vec<String>>,
}

impl<T: Scalar> TryFrom<ZeroRateCurveData<T>> for ZeroRateTermStructure<T> {
    type Error = QSError;

    fn try_from(data: ZeroRateCurveData<T>) -> Result<Self> {
        let curve = Self::new(
            data.reference_date,
            data.dates,
            data.rates,
            data.rate_definition,
            data.interpolator,
            data.enable_extrapolation,
        )?;
        if let Some(labels) = data.pillar_labels {
            curve.with_pillar_labels(labels)
        } else {
            Ok(curve)
        }
    }
}

impl<T: Scalar> From<ZeroRateTermStructure<T>> for ZeroRateCurveData<T> {
    fn from(curve: ZeroRateTermStructure<T>) -> Self {
        Self {
            reference_date: curve.reference_date,
            dates: curve.dates,
            rates: curve.rates,
            rate_definition: curve.rate_definition,
            interpolator: curve.interpolator,
            enable_extrapolation: curve.enable_extrapolation,
            pillar_labels: curve.pillar_labels,
        }
    }
}

impl<T: Scalar> ZeroRateTermStructure<T> {
    /// Creates a curve. The first pillar may be after the reference date.
    ///
    /// # Errors
    /// Rejects empty, mismatched, unordered or non-finite data and dates before the reference date.
    pub fn new(
        reference_date: Date,
        dates: Vec<Date>,
        rates: Vec<T>,
        rate_definition: RateDefinition,
        interpolator: Interpolator,
        enable_extrapolation: bool,
    ) -> Result<Self> {
        if dates.is_empty() || dates.len() != rates.len() {
            return Err(QSError::InvalidValueErr(
                "Zero curve needs matching non-empty dates and rates".into(),
            ));
        }
        if dates[0] < reference_date || dates.windows(2).any(|pair| pair[1] <= pair[0]) {
            return Err(QSError::InvalidValueErr(
                "Zero curve dates must increase strictly from the reference date".into(),
            ));
        }
        if rates.iter().any(|rate| !rate.value().is_finite()) {
            return Err(QSError::InvalidValueErr(
                "Zero curve rates must be finite".into(),
            ));
        }
        let year_fractions = dates
            .iter()
            .map(|date| {
                rate_definition
                    .day_counter()
                    .year_fraction(reference_date, *date)
            })
            .collect::<Vec<_>>();
        if year_fractions.windows(2).any(|pair| pair[1] <= pair[0]) {
            return Err(QSError::InvalidValueErr(
                "Zero curve pillar times must increase strictly".into(),
            ));
        }
        Ok(Self {
            reference_date,
            dates,
            rates,
            year_fractions,
            rate_definition,
            interpolator,
            enable_extrapolation,
            pillar_labels: None,
        })
    }

    /// Returns the reference date.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Returns the pillar dates.
    #[must_use]
    pub const fn dates(&self) -> &Vec<Date> {
        &self.dates
    }

    /// Returns the pillar zero rates.
    #[must_use]
    pub const fn rates(&self) -> &Vec<T> {
        &self.rates
    }

    /// Returns the rate conventions.
    #[must_use]
    pub const fn rate_definition(&self) -> RateDefinition {
        self.rate_definition
    }

    /// Returns the interpolation method.
    #[must_use]
    pub const fn interpolator(&self) -> Interpolator {
        self.interpolator
    }

    /// Returns whether extrapolation is enabled.
    #[must_use]
    pub const fn enable_extrapolation(&self) -> bool {
        self.enable_extrapolation
    }

    /// Returns the optional sensitivity labels of the zero-rate pillars.
    #[must_use]
    pub fn labels(&self) -> Option<&[String]> {
        self.pillar_labels.as_deref()
    }

    /// Labels the zero-rate pillars for sensitivity reports.
    ///
    /// # Errors
    /// Returns an error unless there is one label per zero-rate pillar.
    pub fn with_pillar_labels(mut self, labels: Vec<String>) -> Result<Self> {
        if labels.len() != self.rates.len() {
            return Err(QSError::InvalidValueErr(
                "Zero curve needs one label per pillar".into(),
            ));
        }
        self.pillar_labels = Some(labels);
        Ok(self)
    }

    /// Interpolates the zero rate at a date.
    ///
    /// # Errors
    /// Returns an error for dates outside the permitted curve range.
    pub fn zero_rate(&self, date: Date) -> Result<T>
    where
        Interpolator: Interpolate<T>,
    {
        self.zero_rate_from_time(
            self.rate_definition
                .day_counter()
                .year_fraction(self.reference_date, date),
        )
    }

    /// Interpolates the zero rate at a year fraction from the reference date.
    ///
    /// # Errors
    /// Returns an error for invalid times or disabled extrapolation.
    pub fn zero_rate_from_time(&self, time: f64) -> Result<T>
    where
        Interpolator: Interpolate<T>,
    {
        if !time.is_finite() || time < 0.0 {
            return Err(QSError::InvalidValueErr(
                "Zero curve time must be finite and non-negative".into(),
            ));
        }
        if !self.enable_extrapolation
            && (time < self.year_fractions[0]
                || time > self.year_fractions[self.year_fractions.len() - 1])
        {
            return Err(QSError::InterpolationErr(
                "Zero curve extrapolation is disabled".into(),
            ));
        }
        if self.rates.len() == 1 {
            return Ok(self.rates[0]);
        }
        let times = self
            .year_fractions
            .iter()
            .map(|value| T::scalar(*value))
            .collect::<Vec<_>>();
        self.interpolator.interpolate(
            T::scalar(time),
            &times,
            &self.rates,
            self.enable_extrapolation,
        )
    }
}

// InterestRate has scalar-specific compounding and implied-rate methods.
// Share the curve behavior while selecting those methods for each scalar.
macro_rules! impl_zero_curve {
    ($scalar:ty) => {
        impl ZeroRateTermStructure<$scalar> {
            /// Rolls the curve forward, preserving discount-factor ratios at shifted pillars.
            ///
            /// # Errors
            /// Returns an error for backward rolls or unavailable shifted discount factors.
            pub fn advance_to_period(
                &self,
                period: Period,
            ) -> Result<Rc<dyn InterestRatesTermStructure<$scalar>>> {
                self.advance_to_date(self.reference_date + period)
            }

            /// Rolls the curve to a new reference date.
            ///
            /// # Errors
            /// Returns an error for backward rolls or unavailable shifted discount factors.
            pub fn advance_to_date(
                &self,
                date: Date,
            ) -> Result<Rc<dyn InterestRatesTermStructure<$scalar>>> {
                let days = i32::try_from(date - self.reference_date)
                    .map_err(|_| QSError::InvalidValueErr("Day count must fit in i32".into()))?;
                if days < 0 {
                    return Err(QSError::InvalidValueErr(
                        "Cannot roll a zero curve backwards".into(),
                    ));
                }
                let start_df = self.discount_factor(date)?;
                let dates = self
                    .dates
                    .iter()
                    .map(|pillar| pillar.advance(days, TimeUnit::Days))
                    .collect::<Vec<_>>();
                let rates = dates
                    .iter()
                    .map(|pillar| {
                        let time = self
                            .rate_definition
                            .day_counter()
                            .year_fraction(date, *pillar);
                        if time == 0.0 {
                            return self.zero_rate(date);
                        }
                        Ok(InterestRate::<$scalar>::implied_rate(
                            start_df.div_val(self.discount_factor(*pillar)?),
                            self.rate_definition.day_counter(),
                            self.rate_definition.compounding(),
                            self.rate_definition.frequency(),
                            time,
                        )?
                        .rate())
                    })
                    .collect::<Result<Vec<_>>>()?;
                let mut curve = Self::new(
                    date,
                    dates,
                    rates,
                    self.rate_definition,
                    self.interpolator,
                    self.enable_extrapolation,
                )?;
                curve.pillar_labels = self.pillar_labels.clone();
                Ok(Rc::new(curve))
            }
        }

        impl InterestRatesTermStructure<$scalar> for ZeroRateTermStructure<$scalar> {
            fn reference_date(&self) -> Date {
                self.reference_date
            }

            fn discount_factor(&self, date: Date) -> Result<$scalar> {
                self.discount_factor_from_time(
                    self.rate_definition
                        .day_counter()
                        .year_fraction(self.reference_date, date),
                )
            }

            fn forward_rate(
                &self,
                start_date: Date,
                end_date: Date,
                comp: Compounding,
                freq: Frequency,
            ) -> Result<$scalar> {
                if end_date <= start_date {
                    return Err(QSError::InvalidValueErr(
                        "Forward end date must follow its start".into(),
                    ));
                }
                let compound = self
                    .discount_factor(start_date)?
                    .div_val(self.discount_factor(end_date)?);
                let time = self
                    .rate_definition
                    .day_counter()
                    .year_fraction(start_date, end_date);
                Ok(InterestRate::<$scalar>::implied_rate(
                    compound,
                    self.rate_definition.day_counter(),
                    comp,
                    freq,
                    time,
                )?
                .rate())
            }

            fn nodes(&self) -> Option<Vec<(Date, $scalar)>> {
                self.dates
                    .iter()
                    .map(|date| {
                        self.discount_factor(*date)
                            .map(|discount| (*date, discount))
                    })
                    .collect::<Result<Vec<_>>>()
                    .ok()
            }

            fn day_counter(&self) -> Option<DayCounter> {
                Some(self.rate_definition.day_counter())
            }

            fn discount_factor_from_time(&self, time: f64) -> Result<$scalar> {
                if time == 0.0 {
                    return Ok(<$scalar as Scalar>::one());
                }
                let rate = InterestRate::from_rate_definition(
                    self.zero_rate_from_time(time)?,
                    self.rate_definition,
                );
                let discount =
                    <$scalar as Scalar>::one().div_val(rate.compound_factor_from_yf(time));
                if !discount.value().is_finite() || discount.value() <= 0.0 {
                    return Err(QSError::InvalidValueErr(
                        "Zero curve discount factor must be finite and positive".into(),
                    ));
                }
                Ok(discount)
            }

            fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<$scalar> {
                if end < start {
                    return Err(QSError::InvalidValueErr(
                        "Forward end time must not precede its start".into(),
                    ));
                }
                let end = if (end - start).abs() < 1e-14 {
                    start + 1e-8
                } else {
                    end
                };
                Ok(self
                    .discount_factor_from_time(start)?
                    .div_val(self.discount_factor_from_time(end)?)
                    .sub_val(<$scalar as Scalar>::one())
                    .div_val(<$scalar as Scalar>::scalar(end - start)))
            }
        }
    };
}

impl_zero_curve!(f64);
impl_zero_curve!(DualFwd);

impl ZeroRateTermStructure<f64> {
    /// Converts the zero rates to AD values while preserving conventions and labels.
    #[must_use]
    pub fn to_dual(&self) -> ZeroRateTermStructure<DualFwd> {
        ZeroRateTermStructure {
            reference_date: self.reference_date,
            dates: self.dates.clone(),
            rates: self.rates.iter().copied().map(DualFwd::new).collect(),
            year_fractions: self.year_fractions.clone(),
            rate_definition: self.rate_definition,
            interpolator: self.interpolator,
            enable_extrapolation: self.enable_extrapolation,
            pillar_labels: self.pillar_labels.clone(),
        }
    }
}

impl Pillars<DualFwd> for ZeroRateTermStructure<DualFwd> {
    fn pillars(&self) -> Option<Vec<(String, &DualFwd)>> {
        self.pillar_labels
            .as_ref()
            .map(|labels| labels.iter().cloned().zip(self.rates.iter()).collect())
    }

    fn pillar_labels(&self) -> Option<Vec<String>> {
        self.pillar_labels.clone()
    }

    fn put_pillars_on_tape(&mut self) {
        self.rates.iter_mut().for_each(DualFwd::put_on_tape);
    }
}

impl ADCurveElement for ZeroRateTermStructure<DualFwd> {
    fn clone_curve(&self) -> Result<Box<dyn ADCurveElement>> {
        Ok(Box::new(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ad::tape::Tape;

    fn curve_values<T: Scalar>(
        curve: &dyn InterestRatesTermStructure<T>,
        start: Date,
        end: Date,
    ) -> Result<Vec<T>> {
        Ok(vec![
            curve.discount_factor(end)?,
            curve.discount_factor_from_time(0.25)?,
            curve.forward_rate(start, end, Compounding::Compounded, Frequency::Semiannual)?,
            curve.forward_rate_from_time(0.25, 1.5)?,
        ])
    }

    #[test]
    fn zero_curve_ad_matches_finite_differences() -> Result<()> {
        let reference = Date::new(2026, 1, 1);
        let start = reference.advance(6, TimeUnit::Months);
        let end = reference.advance(18, TimeUnit::Months);
        let dates = vec![reference, reference.advance(1, TimeUnit::Years), end];
        let rates = vec![0.01, 0.025, 0.03];
        let labels = vec!["0D".into(), "1Y".into(), "18M".into()];
        let bump = 1e-6;
        for interpolator in [
            Interpolator::Linear,
            Interpolator::LogLinear,
            Interpolator::CubicSpline,
        ] {
            for compounding in [
                Compounding::Simple,
                Compounding::Compounded,
                Compounding::Continuous,
                Compounding::SimpleThenCompounded,
                Compounding::CompoundedThenSimple,
            ] {
                let definition =
                    RateDefinition::new(DayCounter::Actual365, compounding, Frequency::Semiannual);
                let make_curve = |rates| {
                    ZeroRateTermStructure::new(
                        reference,
                        dates.clone(),
                        rates,
                        definition,
                        interpolator,
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
                assert_eq!(element.pillar_labels(), Some(labels.clone()));
                assert_eq!(
                    element.pillars().map(|pillars| pillars.len()),
                    Some(rates.len())
                );
                let mut outputs = curve_values(element, start, end)?;
                outputs.push(dual.zero_rate(start)?);
                outputs.push(dual.zero_rate_from_time(0.25)?);
                let mut expected = curve_values(&curve, start, end)?;
                expected.push(curve.zero_rate(start)?);
                expected.push(curve.zero_rate_from_time(0.25)?);
                for (output_index, (output, expected)) in outputs.iter().zip(expected).enumerate() {
                    assert!((output.value() - expected).abs() < 1e-12);
                    Tape::reset_adjoints_fwd();
                    let _ = output.backward();
                    for (pillar_index, (_, pillar)) in
                        element.pillars().into_iter().flatten().enumerate()
                    {
                        let mut plus = rates.clone();
                        let mut minus = rates.clone();
                        plus[pillar_index] += bump;
                        minus[pillar_index] -= bump;
                        let evaluate = |rates| -> Result<f64> {
                            let curve = make_curve(rates)?;
                            let mut values = curve_values(&curve, start, end)?;
                            values.push(curve.zero_rate(start)?);
                            values.push(curve.zero_rate_from_time(0.25)?);
                            Ok(values[output_index])
                        };
                        let finite_difference = (evaluate(plus)? - evaluate(minus)?) / (2.0 * bump);
                        let adjoint = pillar.adjoint()?;
                        assert!((adjoint - finite_difference).abs() < 1e-7 * (1.0 + finite_difference.abs()),
                            "{interpolator:?}/{compounding:?}, output {output_index}, pillar {pillar_index}: AD {adjoint} vs FD {finite_difference}");
                    }
                }
                assert_eq!(element.nodes().map(|nodes| nodes.len()), Some(dates.len()));
                Tape::stop_recording_fwd();
                Tape::rewind_to_init_fwd();
            }
        }
        Ok(())
    }

    #[test]
    fn zero_curve_ad_roll_preserves_pillar_risk() -> Result<()> {
        let reference = Date::new(2026, 1, 1);
        let dates = vec![
            reference,
            reference.advance(1, TimeUnit::Years),
            reference.advance(2, TimeUnit::Years),
        ];
        let rates = vec![0.01, 0.02, 0.03];
        let roll = reference.advance(6, TimeUnit::Months);
        let payment = reference.advance(3, TimeUnit::Years);
        let make_curve = |rates| {
            ZeroRateTermStructure::new(
                reference,
                dates.clone(),
                rates,
                RateDefinition::default(),
                Interpolator::Linear,
                true,
            )
        };
        let curve = make_curve(rates.clone())?;
        Tape::rewind_to_init_fwd();
        Tape::start_recording_fwd();
        let dual = curve.to_dual();
        let output = dual.advance_to_date(roll)?.discount_factor(payment)?;
        let _ = output.backward();
        for (index, pillar) in dual.rates().iter().enumerate() {
            let mut plus = rates.clone();
            let mut minus = rates.clone();
            plus[index] += 1e-6;
            minus[index] -= 1e-6;
            let evaluate = |rates| {
                make_curve(rates)?
                    .advance_to_date(roll)?
                    .discount_factor(payment)
            };
            let finite_difference = (evaluate(plus)? - evaluate(minus)?) / 2e-6;
            assert!((pillar.adjoint()? - finite_difference).abs() < 1e-7);
        }
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
        Ok(())
    }

    #[test]
    fn zero_curve_ad_constructor_preserves_existing_links() -> Result<()> {
        let reference = Date::new(2026, 1, 1);
        Tape::rewind_to_init_fwd();
        Tape::start_recording_fwd();
        let input = DualFwd::new(0.01);
        let curve = ZeroRateTermStructure::new(
            reference,
            vec![reference],
            vec![input.mul_val(DualFwd::scalar(2.0))],
            RateDefinition::new(
                DayCounter::Actual365,
                Compounding::Continuous,
                Frequency::Annual,
            ),
            Interpolator::Linear,
            true,
        )?;
        let output = curve.discount_factor_from_time(2.0)?;
        let _ = output.backward();
        assert!((input.adjoint()? + 4.0 * (-0.04_f64).exp()).abs() < 1e-12);
        assert!(curve.with_pillar_labels(vec![]).is_err());
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
        Ok(())
    }

    #[test]
    fn zero_curve_interpolation_forward_and_serde() -> Result<()> {
        let reference = Date::new(2026, 1, 1);
        let curve = ZeroRateTermStructure::new(
            reference,
            vec![
                reference,
                reference + Period::new(1, TimeUnit::Years),
                reference + Period::new(2, TimeUnit::Years),
            ],
            vec![0.01, 0.02, 0.03],
            RateDefinition::default(),
            Interpolator::Linear,
            true,
        )?
        .with_pillar_labels(vec!["0D".into(), "1Y".into(), "2Y".into()])?;
        let start = reference + Period::new(1, TimeUnit::Years);
        let end = reference + Period::new(2, TimeUnit::Years);
        let expected = (curve.discount_factor(start)? / curve.discount_factor(end)? - 1.0)
            / DayCounter::Actual360.year_fraction(start, end);
        assert!(
            (curve.forward_rate(start, end, Compounding::Simple, Frequency::Annual)? - expected)
                .abs()
                < 1e-12
        );
        assert_eq!(curve.discount_factor(reference)?, 1.0);
        assert!(
            (curve.zero_rate_from_time(0.5)? - (0.01 + 0.01 * 0.5 / (365.0 / 360.0))).abs() < 1e-12
        );
        let json = serde_json::to_string(&curve)
            .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        let restored: ZeroRateTermStructure = serde_json::from_str(&json)
            .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        assert_eq!(restored.discount_factor(end)?, curve.discount_factor(end)?);
        assert_eq!(restored.labels(), curve.labels());
        let rolled = curve.advance_to_period(Period::new(1, TimeUnit::Years))?;
        let shifted = end + Period::new(365, TimeUnit::Days);
        assert!(
            (rolled.discount_factor(shifted)?
                - curve.discount_factor(shifted)? / curve.discount_factor(start)?)
            .abs()
                < 1e-12
        );
        Ok(())
    }

    #[test]
    fn zero_curve_tracked_pillars_match_interpolation() -> Result<()> {
        let reference = Date::new(2026, 1, 1);
        let curve = ZeroRateTermStructure::new(
            reference,
            vec![reference, reference.advance(2, TimeUnit::Years)],
            vec![0.002, 0.008],
            RateDefinition::default(),
            Interpolator::LogLinear,
            true,
        )?;
        let date = reference.advance(1, TimeUnit::Years);
        Tape::start_recording_fwd();
        let mut dual = curve.to_dual();
        dual.put_pillars_on_tape();
        let rate = dual.zero_rate(date)?;
        let _ = rate.backward();
        assert!((rate.value() - curve.zero_rate(date)?).abs() < 1e-12);
        assert!(dual
            .rates()
            .iter()
            .all(|pillar| pillar.adjoint().is_ok_and(|value| value > 0.0)));
        Tape::stop_recording_fwd();
        let single = ZeroRateTermStructure::new(
            reference,
            vec![reference],
            vec![0.005],
            RateDefinition::default(),
            Interpolator::Linear,
            true,
        )?;
        assert_eq!(single.zero_rate(date)?, 0.005);
        Ok(())
    }

    #[test]
    fn zero_curve_rejects_invalid_data_and_extrapolation() -> Result<()> {
        let reference = Date::new(2026, 1, 1);
        for (dates, rates) in [
            (vec![], vec![]),
            (vec![reference], vec![]),
            (vec![reference, reference], vec![0.01, 0.02]),
            (vec![reference], vec![f64::NAN]),
        ] {
            assert!(ZeroRateTermStructure::new(
                reference,
                dates,
                rates,
                RateDefinition::default(),
                Interpolator::Linear,
                false
            )
            .is_err());
        }
        let curve = ZeroRateTermStructure::new(
            reference,
            vec![reference, reference + Period::new(1, TimeUnit::Years)],
            vec![0.01, 0.02],
            RateDefinition::default(),
            Interpolator::Linear,
            false,
        )?;
        assert!(curve.discount_factor_from_time(2.0).is_err());
        assert!(curve.discount_factor_from_time(-1.0).is_err());
        assert!(curve
            .advance_to_period(Period::new(-1, TimeUnit::Days))
            .is_err());
        Ok(())
    }
}

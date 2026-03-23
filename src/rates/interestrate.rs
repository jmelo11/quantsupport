use serde::{Deserialize, Serialize};

use crate::{
    ad::adreal::{ADReal, Const, FloatExt, IsReal},
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};

use super::compounding::Compounding;

/// Struct that defines a rate.
///
/// # Example
/// ```
/// use quantsupport::prelude::*;
///
/// let rate_definition = RateDefinition::new(DayCounter::Actual360, Compounding::Simple, Frequency::Annual);
/// assert_eq!(rate_definition.compounding(), Compounding::Simple);
/// assert_eq!(rate_definition.frequency(), Frequency::Annual);
/// assert_eq!(rate_definition.day_counter(), DayCounter::Actual360);
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RateDefinition {
    day_counter: DayCounter,
    compounding: Compounding,
    frequency: Frequency,
}

impl RateDefinition {
    /// Creates a new [`RateDefinition`] with the specified day counter, compounding, and frequency.
    #[must_use]
    pub const fn new(
        day_counter: DayCounter,
        compounding: Compounding,
        frequency: Frequency,
    ) -> Self {
        Self {
            day_counter,
            compounding,
            frequency,
        }
    }

    /// Returns the compounding method of this rate definition.
    #[must_use]
    pub const fn compounding(&self) -> Compounding {
        self.compounding
    }

    /// Returns the frequency of this rate definition.
    #[must_use]
    pub const fn frequency(&self) -> Frequency {
        self.frequency
    }

    /// Returns the day counter of this rate definition.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }
}

impl Default for RateDefinition {
    fn default() -> Self {
        Self::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        )
    }
}

/// Struct that defines an interest rate.
///
/// ## Example
/// ```
/// use quantsupport::rates::interestrate::InterestRate;
/// use quantsupport::time::{date::Date, daycounter::DayCounter, enums::Frequency};
/// use quantsupport::rates::compounding::Compounding;
///
/// let rate = InterestRate::new(0.05, Compounding::Simple, Frequency::Annual, DayCounter::Actual360);
/// assert_eq!(rate.rate(), 0.05);
/// assert_eq!(rate.compounding(), Compounding::Simple);
/// assert_eq!(rate.frequency(), Frequency::Annual);
/// assert_eq!(rate.day_counter(), DayCounter::Actual360);
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterestRate<T>
where
    T: IsReal,
{
    rate: T,
    rate_definition: RateDefinition,
}

impl<T> InterestRate<T>
where
    T: IsReal,
{
    /// Creates a new [`InterestRate<T>`] with the specified rate value and rate definition parameters.
    #[must_use]
    pub const fn new(
        rate: T,
        compounding: Compounding,
        frequency: Frequency,
        day_counter: DayCounter,
    ) -> Self {
        Self {
            rate,
            rate_definition: RateDefinition::new(day_counter, compounding, frequency),
        }
    }

    /// Creates a new [`InterestRate<T>`] from a rate value and a [`RateDefinition`].
    #[must_use]
    pub const fn from_rate_definition(rate: T, rate_definition: RateDefinition) -> Self {
        Self {
            rate,
            rate_definition,
        }
    }

    /// Returns the rate value of this interest rate.
    #[must_use]
    pub const fn rate_ref(&self) -> &T {
        &self.rate
    }

    /// Returns the rate definition of this interest rate.
    #[must_use]
    pub const fn rate_mut(&mut self) -> &mut T {
        &mut self.rate
    }

    /// Returns the rate value of this interest rate.
    #[must_use]
    pub const fn rate(&self) -> T {
        self.rate
    }

    /// Returns the rate definition of this interest rate.
    #[must_use]
    pub const fn rate_definition(&self) -> RateDefinition {
        self.rate_definition
    }

    /// Returns the compounding method of this interest rate.
    #[must_use]
    pub const fn compounding(&self) -> Compounding {
        self.rate_definition.compounding()
    }

    /// Returns the frequency of this interest rate.
    #[must_use]
    pub const fn frequency(&self) -> Frequency {
        self.rate_definition.frequency()
    }

    /// Returns the day counter of this interest rate.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.rate_definition.day_counter()
    }
}

impl From<InterestRate<f64>> for InterestRate<ADReal> {
    fn from(value: InterestRate<f64>) -> Self {
        Self::from_rate_definition(ADReal::new(value.rate().value()), value.rate_definition())
    }
}

impl From<InterestRate<ADReal>> for InterestRate<f64> {
    fn from(value: InterestRate<ADReal>) -> Self {
        Self::from_rate_definition(value.rate().value(), value.rate_definition())
    }
}

impl InterestRate<f64> {
    /// Calculates the implied interest rate from a compound factor.
    ///
    /// # Errors
    /// Returns an error if the compound factor or time are invalid for the
    /// requested compounding convention.
    pub fn implied_rate(
        compound: f64,
        result_dc: DayCounter,
        comp: Compounding,
        freq: Frequency,
        t: f64,
    ) -> Result<Self> {
        if compound <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Positive compound factor required".to_string(),
            ));
        }
        let r: f64;
        let f = f64::from(freq as i32);
        if (compound - 1.0).abs() < 1e-12 {
            if t < 0.0 {
                return Err(QSError::InvalidValueErr(
                    "Non-negative time required".to_string(),
                ));
            }
            r = 0.0;
        } else {
            if t <= 0.0 {
                return Err(QSError::InvalidValueErr(
                    "Positive time required".to_string(),
                ));
            }
            match comp {
                Compounding::Simple => r = (compound - 1.0) / t,
                Compounding::Compounded => r = (compound.powf(1.0 / (f * t)) - 1.0) * f,
                Compounding::Continuous => r = (compound).ln() / t,
                Compounding::SimpleThenCompounded => {
                    if t <= 1.0 / f {
                        r = (compound - 1.0) / t;
                    } else {
                        r = (compound.powf(1.0 / (f * t)) - 1.0) * f;
                    }
                }
                Compounding::CompoundedThenSimple => {
                    if t > 1.0 / f {
                        r = (compound - 1.0) / t;
                    } else {
                        r = (compound.powf(1.0 / (f * t)) - 1.0) * f;
                    }
                }
            }
        }
        Ok(Self::new(r, comp, freq, result_dc))
    }

    /// Calculates the compound factor between two dates using the day counter.
    #[must_use]
    pub fn compound_factor(&self, start: Date, end: Date) -> f64 {
        let day_counter = self.day_counter();
        let year_fraction = day_counter.year_fraction(start, end);
        self.compound_factor_from_yf(year_fraction)
    }

    /// Calculates the compound factor from a year fraction.
    #[must_use]
    pub fn compound_factor_from_yf(&self, year_fraction: f64) -> f64 {
        let rate = self.rate();
        let compounding = self.compounding();
        let f = f64::from(self.frequency() as i32);
        match compounding {
            Compounding::Simple => rate.mul_add(year_fraction, 1.0),
            Compounding::Compounded => (1.0 + rate / f).powf(f * year_fraction),
            Compounding::Continuous => (rate * year_fraction).exp(),
            Compounding::SimpleThenCompounded => {
                if year_fraction <= 1.0 / f {
                    rate.mul_add(year_fraction, 1.0)
                } else {
                    (1.0 + rate / f).powf(year_fraction * f)
                }
            }
            Compounding::CompoundedThenSimple => {
                if year_fraction > 1.0 / f {
                    rate.mul_add(year_fraction, 1.0)
                } else {
                    (1.0 + rate / f).powf(year_fraction * f)
                }
            }
        }
    }

    /// Calculates the discount factor between two dates.
    #[must_use]
    pub fn discount_factor(&self, start: Date, end: Date) -> f64 {
        1.0 / self.compound_factor(start, end)
    }

    /// Calculates the forward rate between two dates with specified compounding and frequency.
    #[must_use]
    pub const fn forward_rate(
        &self,
        _start_date: Date,
        _end_date: Date,
        _comp: Compounding,
        _freq: Frequency,
    ) -> f64 {
        self.rate
    }
}

impl InterestRate<ADReal> {
    /// Calculates the implied interest rate from a compound factor.
    ///
    /// # Errors
    /// Returns an error if the compound factor or time are invalid for the
    /// requested compounding convention.
    pub fn implied_rate(
        compound: ADReal,
        result_dc: DayCounter,
        comp: Compounding,
        freq: Frequency,
        t: f64,
    ) -> Result<Self> {
        if compound <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Positive compound factor required".to_string(),
            ));
        }
        // NOTE: Unlike the f64 version we do NOT short-circuit when
        // compound ≈ 1.  Returning `ADReal::zero()` would sever the
        // connection to the AD tape and produce a zero-row Jacobian,
        // which breaks Newton solvers that rely on AD derivatives
        // (e.g. multi-curve bootstrapping).  The normal formulas below
        // evaluate to ≈ 0 with the correct non-zero partial derivatives.
        if t <= 0.0 {
            // The only safe early-out: when time is non-positive AND
            // compound ≈ 1 the rate is mathematically 0.  Otherwise error.
            if (compound.value() - 1.0).abs() < 1e-12 {
                return Ok(Self::new(ADReal::zero(), comp, freq, result_dc));
            }
            return Err(QSError::InvalidValueErr(
                "Positive time required".to_string(),
            ));
        }
        let f = f64::from(freq as i32);
        match comp {
            Compounding::Simple => {
                let r = (compound - 1.0) / t;
                Ok(Self::new(r.into(), comp, freq, result_dc))
            }
            Compounding::Compounded => {
                let r = (compound.pow_expr(Const::one() / (t * f)) - 1.0) * f;
                Ok(Self::new(r.into(), comp, freq, result_dc))
            }
            Compounding::Continuous => {
                let r = (compound).ln() / t;
                Ok(Self::new(r.into(), comp, freq, result_dc))
            }
            Compounding::SimpleThenCompounded => {
                if t <= 1.0 / f {
                    let r = (compound - 1.0) / t;
                    Ok(Self::new(r.into(), comp, freq, result_dc))
                } else {
                    let r = (compound.pow_expr(Const::one() / (t * f)) - 1.0) * f;
                    Ok(Self::new(r.into(), comp, freq, result_dc))
                }
            }
            Compounding::CompoundedThenSimple => {
                if t > 1.0 / f {
                    let r = (compound - 1.0) / t;
                    Ok(Self::new(r.into(), comp, freq, result_dc))
                } else {
                    let r = (compound.pow_expr(Const::one() / (t * f)) - 1.0) * f;
                    Ok(Self::new(r.into(), comp, freq, result_dc))
                }
            }
        }
    }

    /// Calculates the compound factor between two dates using the day counter.
    #[must_use]
    pub fn compound_factor(&self, start: Date, end: Date) -> ADReal {
        let day_counter = self.day_counter();
        let year_fraction = day_counter.year_fraction(start, end); // for now
        self.compound_factor_from_yf(year_fraction)
    }

    /// Calculates the compound factor from a year fraction.
    #[must_use]
    pub fn compound_factor_from_yf(&self, year_fraction: f64) -> ADReal {
        let rate = self.rate();
        let compounding = self.compounding();
        let f = f64::from(self.frequency() as i32);
        match compounding {
            Compounding::Simple => (rate * year_fraction + Const::one()).into(),
            Compounding::Compounded => (Const::one() + rate / f).powf(year_fraction * f).into(),
            Compounding::Continuous => (rate * year_fraction).exp().into(),
            Compounding::SimpleThenCompounded => {
                if year_fraction <= 1.0 / f {
                    (rate * year_fraction + Const::one()).into()
                } else {
                    (Const::one() + rate / f).powf(year_fraction * f).into()
                }
            }
            Compounding::CompoundedThenSimple => {
                if year_fraction > 1.0 / f {
                    (rate * year_fraction + Const::one()).into()
                } else {
                    (Const::one() + rate / f).powf(year_fraction * f).into()
                }
            }
        }
    }

    /// Calculates the discount factor between two dates.
    #[must_use]
    pub fn discount_factor(&self, start: Date, end: Date) -> ADReal {
        (Const::one() / self.compound_factor(start, end)).into()
    }

    /// Calculates the forward rate between two dates with specified compounding and frequency.
    #[must_use]
    pub const fn forward_rate(
        &self,
        _start_date: Date,
        _end_date: Date,
        _comp: Compounding,
        _freq: Frequency,
    ) -> ADReal {
        self.rate
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        rates::{
            compounding::Compounding,
            interestrate::{InterestRate, RateDefinition},
        },
        time::{daycounter::DayCounter, enums::Frequency},
        utils::errors::Result,
    };

    struct InterestRateData {
        rate: f64,
        compounding: Compounding,
        frequency: Frequency,
        time: f64,
        compounding2: Compounding,
        frequency2: Frequency,
        rate2: f64,
        precision: i64,
    }
    #[allow(clippy::too_many_lines)]
    fn test_cases() -> Vec<InterestRateData> {
        let test_cases = vec![
            InterestRateData {
                rate: 0.0800,
                compounding: Compounding::Compounded,
                frequency: Frequency::Quarterly,
                time: 1.00,
                compounding2: Compounding::Continuous,
                frequency2: Frequency::Annual,
                rate2: 0.0792,
                precision: 4,
            },
            InterestRateData {
                rate: 0.1200,
                compounding: Compounding::Continuous,
                frequency: Frequency::Annual,
                time: 1.00,
                compounding2: Compounding::Compounded,
                frequency2: Frequency::Annual,
                rate2: 0.1275,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0800,
                compounding: Compounding::Compounded,
                frequency: Frequency::Quarterly,
                time: 1.00,
                compounding2: Compounding::Compounded,
                frequency2: Frequency::Annual,
                rate2: 0.0824,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0700,
                compounding: Compounding::Compounded,
                frequency: Frequency::Quarterly,
                time: 1.00,
                compounding2: Compounding::Compounded,
                frequency2: Frequency::Semiannual,
                rate2: 0.0706,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0100,
                compounding: Compounding::Compounded,
                frequency: Frequency::Annual,
                time: 1.00,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Annual,
                rate2: 0.0100,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0200,
                compounding: Compounding::Simple,
                frequency: Frequency::Annual,
                time: 1.00,
                compounding2: Compounding::Compounded,
                frequency2: Frequency::Annual,
                rate2: 0.0200,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0300,
                compounding: Compounding::Compounded,
                frequency: Frequency::Semiannual,
                time: 0.50,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Annual,
                rate2: 0.0300,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0400,
                compounding: Compounding::Simple,
                frequency: Frequency::Annual,
                time: 0.50,
                compounding2: Compounding::Compounded,
                frequency2: Frequency::Semiannual,
                rate2: 0.0400,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0500,
                compounding: Compounding::Compounded,
                frequency: Frequency::EveryFourthMonth,
                time: 1.0 / 3.0,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Annual,
                rate2: 0.0500,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0600,
                compounding: Compounding::Simple,
                frequency: Frequency::Annual,
                time: 1.0 / 3.0,
                compounding2: Compounding::Compounded,
                frequency2: Frequency::EveryFourthMonth,
                rate2: 0.0600,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0500,
                compounding: Compounding::Compounded,
                frequency: Frequency::Quarterly,
                time: 0.25,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Annual,
                rate2: 0.0500,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0600,
                compounding: Compounding::Simple,
                frequency: Frequency::Annual,
                time: 0.25,
                compounding2: Compounding::Compounded,
                frequency2: Frequency::Quarterly,
                rate2: 0.0600,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0700,
                compounding: Compounding::Compounded,
                frequency: Frequency::Bimonthly,
                time: 1.0 / 6.0,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Annual,
                rate2: 0.0700,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0800,
                compounding: Compounding::Simple,
                frequency: Frequency::Annual,
                time: 1.0 / 6.0,
                compounding2: Compounding::Compounded,
                frequency2: Frequency::Bimonthly,
                rate2: 0.0800,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0900,
                compounding: Compounding::Compounded,
                frequency: Frequency::Monthly,
                time: 1.0 / 12.0,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Annual,
                rate2: 0.0900,
                precision: 4,
            },
            InterestRateData {
                rate: 0.1000,
                compounding: Compounding::Simple,
                frequency: Frequency::Annual,
                time: 1.0 / 12.0,
                compounding2: Compounding::Compounded,
                frequency2: Frequency::Monthly,
                rate2: 0.1000,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0300,
                compounding: Compounding::SimpleThenCompounded,
                frequency: Frequency::Semiannual,
                time: 0.25,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Annual,
                rate2: 0.0300,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0300,
                compounding: Compounding::SimpleThenCompounded,
                frequency: Frequency::Semiannual,
                time: 0.25,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Semiannual,
                rate2: 0.0300,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0300,
                compounding: Compounding::SimpleThenCompounded,
                frequency: Frequency::Semiannual,
                time: 0.25,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Quarterly,
                rate2: 0.0300,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0300,
                compounding: Compounding::SimpleThenCompounded,
                frequency: Frequency::Semiannual,
                time: 0.50,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Annual,
                rate2: 0.0300,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0300,
                compounding: Compounding::SimpleThenCompounded,
                frequency: Frequency::Semiannual,
                time: 0.50,
                compounding2: Compounding::Simple,
                frequency2: Frequency::Semiannual,
                rate2: 0.0300,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0300,
                compounding: Compounding::SimpleThenCompounded,
                frequency: Frequency::Semiannual,
                time: 0.75,
                compounding2: Compounding::Compounded,
                frequency2: Frequency::Semiannual,
                rate2: 0.0300,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0400,
                compounding: Compounding::Simple,
                frequency: Frequency::Semiannual,
                time: 0.25,
                compounding2: Compounding::SimpleThenCompounded,
                frequency2: Frequency::Quarterly,
                rate2: 0.0400,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0400,
                compounding: Compounding::Simple,
                frequency: Frequency::Semiannual,
                time: 0.25,
                compounding2: Compounding::SimpleThenCompounded,
                frequency2: Frequency::Semiannual,
                rate2: 0.0400,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0400,
                compounding: Compounding::Simple,
                frequency: Frequency::Semiannual,
                time: 0.25,
                compounding2: Compounding::SimpleThenCompounded,
                frequency2: Frequency::Annual,
                rate2: 0.0400,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0400,
                compounding: Compounding::Compounded,
                frequency: Frequency::Quarterly,
                time: 0.50,
                compounding2: Compounding::SimpleThenCompounded,
                frequency2: Frequency::Quarterly,
                rate2: 0.0400,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0400,
                compounding: Compounding::Simple,
                frequency: Frequency::Semiannual,
                time: 0.50,
                compounding2: Compounding::SimpleThenCompounded,
                frequency2: Frequency::Semiannual,
                rate2: 0.0400,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0400,
                compounding: Compounding::Simple,
                frequency: Frequency::Semiannual,
                time: 0.50,
                compounding2: Compounding::SimpleThenCompounded,
                frequency2: Frequency::Annual,
                rate2: 0.0400,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0400,
                compounding: Compounding::Compounded,
                frequency: Frequency::Quarterly,
                time: 0.75,
                compounding2: Compounding::SimpleThenCompounded,
                frequency2: Frequency::Quarterly,
                rate2: 0.0400,
                precision: 4,
            },
            InterestRateData {
                rate: 0.0400,
                compounding: Compounding::Compounded,
                frequency: Frequency::Semiannual,
                time: 0.75,
                compounding2: Compounding::SimpleThenCompounded,
                frequency2: Frequency::Semiannual,
                rate2: 0.0400,
                precision: 4,
            },
        ];
        test_cases
    }
    #[test]
    fn test_all_cases() {
        let test_cases = test_cases();
        for test_case in test_cases {
            let rate: InterestRate<f64> = InterestRate::new(
                test_case.rate,
                test_case.compounding,
                test_case.frequency,
                DayCounter::Actual360,
            );
            let implied_rate: InterestRate<f64> = InterestRate::<f64>::implied_rate(
                rate.compound_factor_from_yf(test_case.time),
                DayCounter::Actual360,
                test_case.compounding2,
                test_case.frequency2,
                test_case.time,
            )
            .unwrap_or_else(|e| {
                panic!(
                    "implied_rate should succeed in test_implied_rate_for_compounding_and_frequency: {e}"
                )
            });
            let precision_i32 = i32::try_from(test_case.precision)
                .unwrap_or_else(|_| panic!("precision should fit in i32"));
            let precision_limit = f64::from(precision_i32);
            assert!((implied_rate.rate() - test_case.rate2).abs() < precision_limit / 100.0);
        }
    }

    #[test]
    fn test_rate_definition_new() {
        let rd = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        );
        assert_eq!(rd.compounding(), Compounding::Simple);
        assert_eq!(rd.frequency(), Frequency::Annual);
        assert_eq!(rd.day_counter(), DayCounter::Actual360);
    }

    #[test]
    fn test_rate_definition_common_definition() {
        let rd = RateDefinition::default();
        assert_eq!(rd.compounding(), Compounding::Simple);
        assert_eq!(rd.frequency(), Frequency::Annual);
        assert_eq!(rd.day_counter(), DayCounter::Actual360);
    }

    #[test]
    fn test_interest_rate_new() {
        let ir = InterestRate::new(
            0.05,
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        assert!((ir.rate() - 0.05).abs() < 1e-12);
        assert_eq!(ir.compounding(), Compounding::Simple);
        assert_eq!(ir.frequency(), Frequency::Annual);
        assert_eq!(ir.day_counter(), DayCounter::Actual360);
    }

    #[test]
    fn test_interest_rate_from_rate_definition() {
        let rd = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        );
        let ir = InterestRate::from_rate_definition(0.05, rd);
        assert!((ir.rate() - 0.05).abs() < 1e-12);
        assert_eq!(ir.compounding(), Compounding::Simple);
        assert_eq!(ir.frequency(), Frequency::Annual);
        assert_eq!(ir.day_counter(), DayCounter::Actual360);
    }

    const EPSILON: f64 = 1e-9; // or any other small value that you choose

    #[test]
    fn test_implied_rate() -> Result<()> {
        // Choose parameters that make sense for your implied_rate function
        // For example:
        let ir = InterestRate::<f64>::implied_rate(
            1.05,
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
            1.0,
        )?;
        let expected_rate = 0.05;
        assert!((ir.rate() - expected_rate).abs() < EPSILON);
        Ok(())
    }

    #[test]
    fn test_implied_rate_panic() {
        let err = InterestRate::<f64>::implied_rate(
            0.0,
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
            1.0,
        );
        assert!(err.is_err());
    }
}

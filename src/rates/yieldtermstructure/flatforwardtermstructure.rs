use crate::{
    ad::adreal::{ADReal, IsReal},
    rates::{
        compounding::Compounding,
        interestrate::{InterestRate, RateDefinition},
        yieldtermstructure::ratestermstructure::RatesTermStructure,
    },
    time::{date::Date, enums::Frequency},
    utils::errors::{AtlasError, Result},
};

/// # `FlatForwardTermStructure`
/// Struct that defines a flat forward term structure.
/// # Example
/// ```
/// use quantsupport::time::date::Date;
/// use quantsupport::rates::yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure;
/// use quantsupport::rates::interestrate::RateDefinition;
/// use quantsupport::rates::yieldtermstructure::ratestermstructure::RatesTermStructure;
///
/// let reference_date = Date::new(2023, 8, 19);
/// let term_structure = FlatForwardTermStructure::new(reference_date, 0.5, RateDefinition::default());
/// assert_eq!(term_structure.reference_date(), reference_date);
/// ```
#[derive(Clone, Copy)]
pub struct FlatForwardTermStructure<T>
where
    T: IsReal,
{
    reference_date: Date,
    rate: InterestRate<T>,
}

impl<T> FlatForwardTermStructure<T>
where
    T: IsReal,
{
    /// Creates a new `FlatForwardTermStructure` with the specified reference date, rate, and rate definition.
    #[must_use]
    pub const fn new(reference_date: Date, rate: T, rate_definition: RateDefinition) -> Self {
        let rate = InterestRate::from_rate_definition(rate, rate_definition);
        Self {
            reference_date,
            rate,
        }
    }

    /// Returns the underlying interest rate.
    #[must_use]
    pub const fn rate(&self) -> InterestRate<T> {
        self.rate
    }

    /// Returns the rate value.
    #[must_use]
    pub const fn value(&self) -> T {
        self.rate.rate()
    }

    /// Returns the rate definition.
    #[must_use]
    pub const fn rate_definition(&self) -> RateDefinition {
        self.rate.rate_definition()
    }
}

impl RatesTermStructure<f64> for FlatForwardTermStructure<f64> {
    fn reference_date(&self) -> Date {
        self.reference_date
    }
    fn discount_factor(&self, date: Date) -> Result<f64> {
        if date < self.reference_date() {
            return Err(AtlasError::InvalidValueErr(format!(
                "Date {date:?} is before reference date {reference_date:?}",
                reference_date = self.reference_date()
            )));
        }
        Ok(self.rate.discount_factor(self.reference_date(), date))
    }
    fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<f64> {
        let comp_factor = self.discount_factor(start_date)? / self.discount_factor(end_date)?;
        let t = self.rate.day_counter().year_fraction(start_date, end_date);
        Ok(
            InterestRate::<f64>::implied_rate(comp_factor, self.rate.day_counter(), comp, freq, t)?
                .rate(),
        )
    }
}

impl RatesTermStructure<ADReal> for FlatForwardTermStructure<ADReal> {
    fn reference_date(&self) -> Date {
        self.reference_date
    }
    fn discount_factor(&self, date: Date) -> Result<ADReal> {
        if date < self.reference_date() {
            return Err(AtlasError::InvalidValueErr(format!(
                "Date {date:?} is before reference date {reference_date:?}",
                reference_date = self.reference_date()
            )));
        }
        Ok(self.rate.discount_factor(self.reference_date(), date))
    }
    fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<ADReal> {
        let comp_factor = self.discount_factor(start_date)? / self.discount_factor(end_date)?;
        let t = self.rate.day_counter().year_fraction(start_date, end_date);
        Ok(InterestRate::<ADReal>::implied_rate(
            comp_factor.into(),
            self.rate.day_counter(),
            comp,
            freq,
            t,
        )?
        .rate())
    }
}

#[cfg(test)]
mod tests {
    use crate::time::{daycounter::DayCounter, enums::TimeUnit, period::Period};

    use super::*;

    #[test]
    fn test_reference_date() {
        let reference_date = Date::new(2023, 8, 19);

        let term_structure =
            FlatForwardTermStructure::new(reference_date, 0.5, RateDefinition::default());
        assert_eq!(term_structure.reference_date(), reference_date);
    }

    #[test]
    fn test_discount() -> Result<()> {
        let reference_date = Date::new(2023, 8, 19);
        let target_date = Date::new(2024, 8, 19);
        let interest_rate = InterestRate::from_rate_definition(0.05, RateDefinition::default());

        let term_structure =
            FlatForwardTermStructure::new(reference_date, 0.05, RateDefinition::default());

        let expected_discount = interest_rate.discount_factor(reference_date, target_date);
        let actual_discount = term_structure.discount_factor(target_date)?;

        assert!((actual_discount - expected_discount).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn test_discount_continuous() -> Result<()> {
        let reference_date = Date::new(2023, 8, 19);
        let target_date = reference_date + Period::new(1, TimeUnit::Years);
        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Continuous,
            Frequency::Semiannual,
        );
        let interest_rate = InterestRate::from_rate_definition(0.05, rate_definition);
        let term_structure = FlatForwardTermStructure::new(reference_date, 0.05, rate_definition);

        let expected_discount = interest_rate.discount_factor(reference_date, target_date);
        let actual_discount = term_structure.discount_factor(target_date)?;

        assert!((actual_discount - expected_discount).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn test_forward_rate() -> Result<()> {
        let reference_date = Date::new(2023, 8, 19);
        let interest_rate = InterestRate::new(
            0.05,
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let start_date = Date::new(2023, 9, 19);
        let end_date = Date::new(2024, 9, 19);
        let comp = Compounding::Simple;
        let freq = Frequency::Annual;

        let term_structure =
            FlatForwardTermStructure::new(reference_date, 0.5, RateDefinition::default());

        let comp_factor = term_structure.discount_factor(start_date)?
            / term_structure.discount_factor(end_date)?;
        let t = interest_rate
            .day_counter()
            .year_fraction(start_date, end_date);

        let expected_forward_rate = InterestRate::<f64>::implied_rate(
            comp_factor,
            interest_rate.day_counter(),
            comp,
            freq,
            t,
        )?
        .rate();
        let actual_forward_rate = term_structure.forward_rate(start_date, end_date, comp, freq)?;

        assert!((actual_forward_rate - expected_forward_rate).abs() < 1e-10);

        Ok(())
    }
}

use crate::{
    ad::{scalar::Scalar, Recorded},
    core::{elements::curveelement::ADCurveElement, pillars::Pillars},
    rates::{
        compounding::Compounding,
        interestrate::{InterestRate, RateDefinition},
        yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};

/// Discount curve generated from one constant quoted interest-rate parameter.
///
/// The rate definition determines discounting; a flat quoted rate is not a
/// constant instantaneous forward under every compounding convention.
///
/// ## Example
/// ```
/// use quantsupport::prelude::*;
///
/// let reference_date = Date::new(2023, 8, 19);
/// let term_structure = FlatForwardTermStructure::new(reference_date, 0.5, RateDefinition::default());
/// assert_eq!(term_structure.reference_date(), reference_date);
/// ```
#[derive(Clone)]
pub struct FlatForwardTermStructure<T>
where
    T: Scalar,
{
    reference_date: Date,
    rate: InterestRate<T>,
    pillar_label: Option<String>,
}

impl<T> FlatForwardTermStructure<T>
where
    T: Scalar,
{
    /// Creates a new [`FlatForwardTermStructure`].
    ///
    /// ## Parameters
    /// * `reference_date`: the reference date for the term structure. All discount factors and forward rates will be calculated relative to this date.
    /// * `rate`: constant decimal quoted rate, interpreted using `rate_definition`.
    /// * `rate_definition`: the definition of the interest rate, including day count convention, compounding method, and frequency.
    #[must_use]
    pub const fn new(reference_date: Date, rate: T, rate_definition: RateDefinition) -> Self {
        let rate = InterestRate::from_rate_definition(rate, rate_definition);
        Self {
            reference_date,
            rate,
            pillar_label: None,
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

    /// Sets the pillar label for the term structure and returns a new instance with the updated label.
    #[must_use]
    pub fn with_pillar_label(mut self, label: String) -> Self {
        self.pillar_label = Some(label);
        self
    }
}

impl Pillars<Recorded> for FlatForwardTermStructure<Recorded> {
    /// Returns the labelled inputs used to report market sensitivities.
    fn pillars(&self) -> Option<Vec<(String, &Recorded)>> {
        self.pillar_label
            .as_ref()
            .map(|label| vec![(label.clone(), self.rate.rate_ref())])
    }

    fn pillar_labels(&self) -> Option<Vec<String>> {
        self.pillar_label.as_ref().map(|label| vec![label.clone()])
    }

    /// Registers current labelled market pillars as independent compiler inputs.
    fn register_pillars(&mut self) {
        self.rate.rate_mut().register_input();
    }
}

impl InterestRatesTermStructure<f64> for FlatForwardTermStructure<f64> {
    fn reference_date(&self) -> Date {
        self.reference_date
    }
    /// Returns the discount factor at a date under the curve conventions.
    fn discount_factor(&self, date: Date) -> Result<f64> {
        if date < self.reference_date() {
            return Err(QSError::InvalidValueErr(format!(
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

    /// Returns the curve nodes in their interpolation order.
    fn nodes(&self) -> Option<Vec<(Date, f64)>> {
        None
    }

    fn day_counter(&self) -> Option<DayCounter> {
        Some(self.rate.day_counter())
    }

    /// Returns the discount factor for a day-count year fraction.
    fn discount_factor_from_time(&self, t: f64) -> Result<f64> {
        if t < 0.0 {
            return Err(QSError::InvalidValueErr("Time must be non-negative".into()));
        }
        Ok(1.0 / self.rate.compound_factor_from_yf(t))
    }

    /// Returns the forward rate between two day-count year fractions.
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<f64> {
        if (end - start).abs() < 1e-14 {
            // Instantaneous forward rate: use numerical limit
            let eps = 1e-8;
            let df_s = self.discount_factor_from_time(start)?;
            let df_e = self.discount_factor_from_time(start + eps)?;
            return Ok((df_s / df_e - 1.0) / eps);
        }
        let df_start = self.discount_factor_from_time(start)?;
        let df_end = self.discount_factor_from_time(end)?;
        let fwd = (df_start / df_end - 1.0) / (end - start);
        Ok(fwd)
    }
}

impl InterestRatesTermStructure<Recorded> for FlatForwardTermStructure<Recorded> {
    fn reference_date(&self) -> Date {
        self.reference_date
    }
    /// Returns the discount factor at a date under the curve conventions.
    fn discount_factor(&self, date: Date) -> Result<Recorded> {
        if date < self.reference_date() {
            return Err(QSError::InvalidValueErr(format!(
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
    ) -> Result<Recorded> {
        let comp_factor = self.discount_factor(start_date)? / self.discount_factor(end_date)?;
        let t = self.rate.day_counter().year_fraction(start_date, end_date);
        Ok(InterestRate::<Recorded>::implied_rate(
            comp_factor,
            self.rate.day_counter(),
            comp,
            freq,
            t,
        )?
        .rate())
    }

    /// Returns the curve nodes in their interpolation order.
    fn nodes(&self) -> Option<Vec<(Date, Recorded)>> {
        None
    }

    fn day_counter(&self) -> Option<DayCounter> {
        Some(self.rate.day_counter())
    }

    /// Returns the discount factor for a day-count year fraction.
    fn discount_factor_from_time(&self, t: f64) -> Result<Recorded> {
        if t < 0.0 {
            return Err(QSError::InvalidValueErr("Time must be non-negative".into()));
        }
        let cf = self.rate.compound_factor_from_yf(t);
        Ok(1.0_f64 / cf)
    }

    /// Returns the forward rate between two day-count year fractions.
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<Recorded> {
        if (end - start).abs() < 1e-14 {
            let eps = 1e-8;
            let df_s = self.discount_factor_from_time(start)?;
            let df_e = self.discount_factor_from_time(start + eps)?;
            let fwd = (df_s / df_e - 1.0) / eps;
            return Ok(fwd);
        }
        let df_start = self.discount_factor_from_time(start)?;
        let df_end = self.discount_factor_from_time(end)?;
        let fwd = (df_start / df_end - 1.0) / (end - start);
        Ok(fwd)
    }
}

impl ADCurveElement for FlatForwardTermStructure<Recorded> {
    fn clone_curve(&self) -> Result<Box<dyn ADCurveElement>> {
        let mut curve = self.clone();
        *curve.rate.rate_mut() = Recorded::constant(self.value().value());
        Ok(Box::new(curve))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{daycounter::DayCounter, enums::TimeUnit, period::Period};

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

    // #[test]
    // fn test_ad_forward_rate() -> Result<()> {
    //     let reference_date = Date::new(2023, 8, 19);

    //     let mut rate = Recorded::from(0.05);
    //     Recording::start();
    //     rate.register_input();
    //     let term_structure =
    //         FlatForwardTermStructure::new(reference_date, rate, RateDefinition::default());
    //     let term_structure_2 = term_structure.clone();

    //     // let df = term_structure.discount_factor(reference_date + Period::new(1, TimeUnit::Years));
    //     let df2 =
    //         term_structure_2.discount_factor(reference_date + Period::new(1, TimeUnit::Years));

    //     Recording::stop();
    //     let rate2 = rate.clone();
    //     // let _ = df?.differentiate()?;
    //     // println!("Rate sensitivity: {:?}", rate.derivative()?);
    //     let _ = df2?.differentiate()?;
    //     println!("Rate sensitivity: {:?}", rate.derivative()?);
    //     Ok(())
    // }
}

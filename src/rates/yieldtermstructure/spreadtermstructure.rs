use crate::{
    ad::{dual::DualFwd, expr::FloatExt, scalar::Scalar},
    core::{elements::curveelement::ADCurveElement, pillars::Pillars},
    math::interpolation::interpolator::{Interpolate, Interpolator},
    rates::{
        compounding::Compounding, interestrate::InterestRate,
        yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};

/// A term structure that represents the continuously-compounded zero-rate
/// spread between two curves.
///
/// Given a *target* curve and a *base* curve the spread at each tenor `t_i` is:
/// ```text
///   s(t_i) = -ln(DF_target(t_i) / DF_base(t_i)) / t_i
/// ```
///
/// The discount factor at an arbitrary time `t` is obtained by interpolating
/// the spread and applying:
/// ```text
///   DF_spread(t) = exp(-s(t) * t)
/// ```
///
/// When built with `T = DualFwd`, the spread values live on the AD tape and
/// sensitivities to the spread can be extracted via the [`Pillars`] trait.
pub struct SpreadTermStructure<T: Scalar> {
    reference_date: Date,
    year_fractions: Vec<f64>,
    spreads: Vec<T>,
    day_counter: DayCounter,
    interpolator: Interpolator,
    pillar_labels: Option<Vec<String>>,
}

impl<T: Scalar> SpreadTermStructure<T> {
    /// Creates a spread term structure from pre-computed values.
    ///
    /// # Errors
    /// Returns an error if `year_fractions` and `spreads` have different lengths
    /// or are empty.
    pub fn new(
        reference_date: Date,
        year_fractions: Vec<f64>,
        spreads: Vec<T>,
        day_counter: DayCounter,
        interpolator: Interpolator,
    ) -> Result<Self> {
        if year_fractions.len() != spreads.len() {
            return Err(QSError::InvalidValueErr(
                "year_fractions and spreads must have the same length".into(),
            ));
        }
        if year_fractions.is_empty() {
            return Err(QSError::InvalidValueErr("spreads cannot be empty".into()));
        }
        Ok(Self {
            reference_date,
            year_fractions,
            spreads,
            day_counter,
            interpolator,
            pillar_labels: None,
        })
    }

    /// Attaches pillar labels so that spreads are exposed through the
    /// [`Pillars`] trait.
    #[must_use]
    pub fn with_pillar_labels(mut self, labels: Vec<String>) -> Self {
        self.pillar_labels = Some(labels);
        self
    }

    /// Returns the underlying spread values.
    #[must_use]
    pub fn spreads(&self) -> &[T] {
        &self.spreads
    }

    /// Returns the reference date of the term structure.
    #[must_use]
    pub const fn ref_date(&self) -> Date {
        self.reference_date
    }
}

impl SpreadTermStructure<f64> {
    /// Builds a spread term structure from two curves and a set of tenor dates.
    ///
    /// The spread at each tenor is extracted as the continuously-compounded
    /// zero-rate difference between `target_curve` and `base_curve`.
    ///
    /// # Errors
    /// Returns an error if any tenor date is before the reference date or if a
    /// discount factor cannot be computed.
    pub fn from_curves(
        target_curve: &dyn InterestRatesTermStructure<f64>,
        base_curve: &dyn InterestRatesTermStructure<f64>,
        tenor_dates: &[Date],
        day_counter: DayCounter,
        interpolator: Interpolator,
    ) -> Result<Self> {
        let reference_date = base_curve.reference_date();
        let mut year_fractions = Vec::with_capacity(tenor_dates.len());
        let mut spreads = Vec::with_capacity(tenor_dates.len());

        for &date in tenor_dates {
            let t = day_counter.year_fraction(reference_date, date);
            if t <= 0.0 {
                return Err(QSError::InvalidValueErr(
                    "Tenor dates must be after the reference date".into(),
                ));
            }
            let df_target = target_curve.discount_factor(date)?;
            let df_base = base_curve.discount_factor(date)?;
            // s(t) = -ln(df_target / df_base) / t
            let spread = -(df_target / df_base).ln() / t;
            year_fractions.push(t);
            spreads.push(spread);
        }

        Ok(Self {
            reference_date,
            year_fractions,
            spreads,
            day_counter,
            interpolator,
            pillar_labels: None,
        })
    }

    /// Converts this f64 spread term structure into a `DualFwd` version,
    /// wrapping every spread value as a new [`DualFwd`].
    #[must_use]
    pub fn to_dual(&self) -> SpreadTermStructure<DualFwd> {
        SpreadTermStructure {
            reference_date: self.reference_date,
            year_fractions: self.year_fractions.clone(),
            spreads: self.spreads.iter().copied().map(DualFwd::new).collect(),
            day_counter: self.day_counter,
            interpolator: self.interpolator,
            pillar_labels: self.pillar_labels.clone(),
        }
    }
}

impl InterestRatesTermStructure<f64> for SpreadTermStructure<f64> {
    fn reference_date(&self) -> Date {
        self.reference_date
    }

    fn discount_factor(&self, date: Date) -> Result<f64> {
        let t = self.day_counter.year_fraction(self.reference_date, date);
        self.discount_factor_from_time(t)
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
        let t = self.day_counter.year_fraction(start_date, end_date);
        Ok(InterestRate::<f64>::implied_rate(comp_factor, self.day_counter, comp, freq, t)?.rate())
    }

    fn nodes(&self) -> Option<Vec<(Date, f64)>> {
        None
    }

    fn day_counter(&self) -> Option<DayCounter> {
        Some(self.day_counter)
    }

    fn discount_factor_from_time(&self, t: f64) -> Result<f64> {
        if t <= 0.0 {
            return Ok(1.0);
        }
        let spread = self
            .interpolator
            .interpolate(t, &self.year_fractions, &self.spreads, true)?;
        Ok((-spread * t).exp())
    }

    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<f64> {
        let df_start = self.discount_factor_from_time(start)?;
        let df_end = self.discount_factor_from_time(end)?;
        let fwd = (df_start / df_end - 1.0) / (end - start);
        Ok(fwd)
    }
}

impl InterestRatesTermStructure<DualFwd> for SpreadTermStructure<DualFwd> {
    fn reference_date(&self) -> Date {
        self.reference_date
    }

    fn discount_factor(&self, date: Date) -> Result<DualFwd> {
        let t = self.day_counter.year_fraction(self.reference_date, date);
        self.discount_factor_from_time(t)
    }

    fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<DualFwd> {
        let df_start = self.discount_factor(start_date)?;
        let df_end = self.discount_factor(end_date)?;
        let comp_factor = df_start / df_end;
        let t = self.day_counter.year_fraction(start_date, end_date);
        Ok(InterestRate::<DualFwd>::implied_rate(
            comp_factor.into(),
            self.day_counter,
            comp,
            freq,
            t,
        )?
        .rate())
    }

    fn nodes(&self) -> Option<Vec<(Date, DualFwd)>> {
        None
    }

    fn day_counter(&self) -> Option<DayCounter> {
        Some(self.day_counter)
    }

    fn discount_factor_from_time(&self, t: f64) -> Result<DualFwd> {
        if t <= 0.0 {
            return Ok(DualFwd::one());
        }
        let yf = DualFwd::new(t);
        let tmp_yfs: Vec<DualFwd> = self
            .year_fractions
            .iter()
            .copied()
            .map(DualFwd::new)
            .collect();
        let spread = self
            .interpolator
            .interpolate(yf, &tmp_yfs, &self.spreads, true)?;
        Ok((DualFwd::new(0.0) - spread * DualFwd::new(t)).exp().into())
    }

    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<DualFwd> {
        let df_start = self.discount_factor_from_time(start)?;
        let df_end = self.discount_factor_from_time(end)?;
        let fwd = (df_start / df_end - DualFwd::one()) / DualFwd::new(end - start);
        Ok(fwd.into())
    }
}

impl Pillars<DualFwd> for SpreadTermStructure<DualFwd> {
    fn pillars(&self) -> Option<Vec<(String, &DualFwd)>> {
        self.pillar_labels
            .as_ref()
            .map(|labels| labels.iter().cloned().zip(self.spreads.iter()).collect())
    }

    fn pillar_labels(&self) -> Option<Vec<String>> {
        self.pillar_labels.clone()
    }

    fn put_pillars_on_tape(&mut self) {
        self.spreads.iter_mut().for_each(DualFwd::put_on_tape);
    }
}

impl ADCurveElement for SpreadTermStructure<DualFwd> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rates::{
        interestrate::RateDefinition,
        yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
    };
    use crate::time::enums::TimeUnit;

    #[test]
    fn spread_round_trip() -> Result<()> {
        let ref_date = Date::new(2024, 1, 1);
        let dc = DayCounter::Actual365;
        let rate_def = RateDefinition::default();

        let target = FlatForwardTermStructure::new(ref_date, 0.05, rate_def);
        let base = FlatForwardTermStructure::new(ref_date, 0.03, rate_def);

        let tenors: Vec<Date> = (1..=10)
            .map(|y| ref_date.advance(y, TimeUnit::Years))
            .collect();

        let spread = SpreadTermStructure::<f64>::from_curves(
            &target,
            &base,
            &tenors,
            dc,
            Interpolator::Linear,
        )?;

        // The spread discount factor times the base discount factor should
        // reproduce the target discount factor.
        for &date in &tenors {
            let df_spread = spread.discount_factor(date)?;
            let df_base = base.discount_factor(date)?;
            let df_target = target.discount_factor(date)?;
            let reconstructed = df_spread * df_base;
            assert!(
                (reconstructed - df_target).abs() < 1e-10,
                "mismatch at {date:?}: {reconstructed} vs {df_target}"
            );
        }
        Ok(())
    }

    #[test]
    fn spread_values_are_positive_for_higher_target_rate() -> Result<()> {
        let ref_date = Date::new(2024, 1, 1);
        let dc = DayCounter::Actual365;
        let rate_def = RateDefinition::default();

        let target = FlatForwardTermStructure::new(ref_date, 0.05, rate_def);
        let base = FlatForwardTermStructure::new(ref_date, 0.03, rate_def);

        let tenors: Vec<Date> = (1..=5)
            .map(|y| ref_date.advance(y, TimeUnit::Years))
            .collect();

        let spread = SpreadTermStructure::<f64>::from_curves(
            &target,
            &base,
            &tenors,
            dc,
            Interpolator::Linear,
        )?;

        // Spread should be approximately 0.02 (5% - 3%). The underlying
        // flat-forward curves use Simple / Actual360 so the continuous spread
        // is not exactly 0.02.
        for s in spread.spreads() {
            assert!(*s > 0.0);
            assert!((*s - 0.02).abs() < 5e-3);
        }
        Ok(())
    }
}

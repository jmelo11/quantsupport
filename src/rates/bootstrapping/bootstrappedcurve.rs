use crate::{
    ad::dual::DualFwd,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::{Interpolate, Interpolator},
    rates::{
        interestrate::{InterestRate, RateDefinition},
        yieldtermstructure::{
            interestratestermstructure::InterestRatesTermStructure,
            zeroratetermstructure::ZeroRateTermStructure,
        },
    },
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

#[derive(Clone)]
enum CurveSource {
    ZeroRate(Box<ZeroRateTermStructure<f64>>),
    Composite(Box<BootstrappedCurve>, Box<BootstrappedCurve>),
}

/// Internally constructed curve representation used during bootstrapping.
#[derive(Clone)]
pub struct BootstrappedCurve {
    market_index: MarketIndex,
    reference_date: Date,
    times: Vec<f64>,
    discount_factors: Vec<f64>,
    day_counter: DayCounter,
    interpolator: Interpolator,
    pillar_values: Option<Vec<DualFwd>>,
    pillar_labels: Option<Vec<String>>,
    output_discount_factors: Option<Vec<DualFwd>>,
    ift_sensitivities: Option<Vec<Vec<f64>>>,
    source: Option<CurveSource>,
}

impl BootstrappedCurve {
    /// Creates a new solved curve from raw data.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        reference_date: Date,
        times: Vec<f64>,
        discount_factors: Vec<f64>,
        day_counter: DayCounter,
        interpolator: Interpolator,
    ) -> Self {
        Self {
            market_index,
            reference_date,
            times,
            discount_factors,
            day_counter,
            interpolator,
            pillar_values: None,
            pillar_labels: None,
            output_discount_factors: None,
            ift_sensitivities: None,
            source: None,
        }
    }

    /// Preserves explicit zero-rate interpolation when the curve is a calibration parent.
    pub(super) fn from_zero_rates(
        market_index: MarketIndex,
        curve: ZeroRateTermStructure<f64>,
    ) -> Result<Self> {
        let reference_date = curve.reference_date();
        let day_counter = curve.rate_definition().day_counter();
        let mut times = vec![0.0];
        times.extend(
            curve
                .dates()
                .iter()
                .map(|date| day_counter.year_fraction(reference_date, *date)),
        );
        times.dedup();
        let discount_factors = times
            .iter()
            .map(|time| curve.discount_factor_from_time(*time))
            .collect::<Result<_>>()?;
        let values = curve
            .rates()
            .iter()
            .map(|rate| DualFwd::new(*rate))
            .collect();
        let labels = curve.labels().unwrap_or_default().to_vec();
        let mut result = Self::new(
            market_index,
            reference_date,
            times,
            discount_factors,
            day_counter,
            curve.interpolator(),
        )
        .with_pillar_values(values)
        .with_pillar_labels(labels);
        result.source = Some(CurveSource::ZeroRate(Box::new(curve)));
        Ok(result)
    }

    /// Preserves multiplicative discounting when a composite is a calibration parent.
    pub(super) fn from_components(
        market_index: MarketIndex,
        base: Self,
        spread: Self,
    ) -> Result<Self> {
        // Query components directly. Caching only the origin avoids requiring
        // their outermost pillars to fall within each other's supported domain.
        let times = vec![0.0];
        let discount_factors = vec![1.0];
        let mut values = base.pillar_values()?.to_vec();
        values.extend_from_slice(spread.pillar_values()?);
        let mut labels = base.pillar_labels().unwrap_or_default().to_vec();
        labels.extend_from_slice(spread.pillar_labels().unwrap_or_default());
        let mut result = Self::new(
            market_index,
            base.reference_date,
            times,
            discount_factors,
            base.day_counter,
            base.interpolator,
        )
        .with_pillar_values(values)
        .with_pillar_labels(labels);
        result.source = Some(CurveSource::Composite(Box::new(base), Box::new(spread)));
        Ok(result)
    }

    /// Whether parent risk should be bumped in source-pillar coordinates.
    pub(super) const fn has_explicit_source(&self) -> bool {
        self.source.is_some()
    }

    /// Bumps one source pillar while retaining its curve's interpolation and composition.
    pub(super) fn bump_pillar(&mut self, index: usize, bump: f64) -> Result<()> {
        let bumped = self.pillar_values()?[index].value() + bump;
        match &mut self.source {
            Some(CurveSource::ZeroRate(curve)) => {
                let mut rates = curve.rates().clone();
                rates[index] += bump;
                **curve = ZeroRateTermStructure::new(
                    curve.reference_date(),
                    curve.dates().clone(),
                    rates,
                    curve.rate_definition(),
                    curve.interpolator(),
                    curve.enable_extrapolation(),
                )?;
            }
            Some(CurveSource::Composite(base, spread)) => {
                let base_count = base.pillar_values()?.len();
                if index < base_count {
                    base.bump_pillar(index, bump)?;
                } else {
                    spread.bump_pillar(index - base_count, bump)?;
                }
            }
            None => {
                let sensitivities = self.ift_sensitivities.as_ref().ok_or_else(|| {
                    QSError::ValueNotSetErr("Parent curve IFT sensitivities".into())
                })?;
                for (factor, row) in self.discount_factors[1..].iter_mut().zip(sensitivities) {
                    *factor = row[index].mul_add(bump, *factor);
                }
            }
        }
        self.pillar_values
            .as_mut()
            .ok_or_else(|| QSError::ValueNotSetErr("Parent curve pillars".into()))?[index] =
            DualFwd::new(bumped);
        if self.source.is_some() {
            self.discount_factors = self
                .times
                .iter()
                .map(|time| self.discount_factor_from_time(*time))
                .collect::<Result<_>>()?;
        }
        Ok(())
    }

    /// Returns the market index of this curve.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the curve valuation date.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Attaches AD-tracked pillar values (typically market quotes).
    #[must_use]
    pub fn with_pillar_values(mut self, pillar_values: Vec<DualFwd>) -> Self {
        self.pillar_values = Some(pillar_values);
        self
    }

    /// Attaches pillar labels (matching `pillar_values` in order).
    #[must_use]
    pub fn with_pillar_labels(mut self, labels: Vec<String>) -> Self {
        self.pillar_labels = Some(labels);
        self
    }

    /// Returns pillar labels, if set.
    #[must_use]
    pub fn pillar_labels(&self) -> Option<&[String]> {
        self.pillar_labels.as_deref()
    }

    /// Attaches AD-tracked discount factors at the pillar dates.
    #[must_use]
    pub fn with_output_discount_factors(mut self, output_discount_factors: Vec<DualFwd>) -> Self {
        self.output_discount_factors = Some(output_discount_factors);
        self
    }

    /// Attaches the IFT sensitivity matrix: `sens[i][j]` = ∂DF(i+1)/∂q(j).
    #[must_use]
    pub fn with_ift_sensitivities(mut self, sensitivities: Vec<Vec<f64>>) -> Self {
        self.ift_sensitivities = Some(sensitivities);
        self
    }

    /// Returns the IFT sensitivity matrix, if available.
    #[must_use]
    pub const fn ift_sensitivities(&self) -> Option<&Vec<Vec<f64>>> {
        self.ift_sensitivities.as_ref()
    }

    /// Returns the AD-tracked pillar values.
    ///
    /// # Errors
    /// Returns an error if the pillar values have not been set, which typically indicates that the curve has not been fully bootstrapped or that there is an issue with the AD linkage during bootstrapping.
    pub fn pillar_values(&self) -> Result<&[DualFwd]> {
        self.pillar_values
            .as_deref()
            .ok_or_else(|| QSError::InvalidValueErr("Pillar values not set".into()))
    }

    /// Returns the raw discount factors.
    #[must_use]
    pub fn discount_factors(&self) -> &[f64] {
        &self.discount_factors
    }

    /// Returns a mutable reference to the raw discount factors.
    #[must_use]
    pub const fn discount_factors_mut(&mut self) -> &mut Vec<f64> {
        &mut self.discount_factors
    }

    /// Returns the discount factor at the given date.
    ///
    /// # Errors
    /// Returns an error if the date is out of bounds for the curve's time grid or if interpolation fails for any reason (e.g., NaN inputs, invalid interpolator state). The error message will indicate the nature of the failure to aid in debugging bootstrapping issues.
    pub fn discount_factor(&self, date: Date) -> Result<f64> {
        let year_fraction = self.day_counter.year_fraction(self.reference_date, date);
        self.discount_factor_from_time(year_fraction)
    }

    fn discount_factor_from_time(&self, time: f64) -> Result<f64> {
        match &self.source {
            Some(CurveSource::ZeroRate(curve)) => curve.discount_factor_from_time(time),
            Some(CurveSource::Composite(base, spread)) => Ok(base
                .discount_factor_from_time(time)?
                * spread.discount_factor_from_time(time)?),
            None => self
                .interpolator
                .interpolate(time, &self.times, &self.discount_factors, true),
        }
    }

    /// Returns the AD-tracked discount factors at the pillar dates.
    ///
    /// # Errors
    /// Returns an error if the output discount factors have not been set, which typically indicates that the curve has not been fully bootstrapped or that there is an issue with the AD linkage during bootstrapping.
    pub fn output_discount_factors(&self) -> Result<&[DualFwd]> {
        self.output_discount_factors
            .as_deref()
            .ok_or_else(|| QSError::InvalidValueErr("Output discount factors not set".into()))
    }

    /// Computes the forward rate between two dates.
    ///
    /// # Errors
    /// Returns an error if the discount factor for either date cannot be computed or if the year fraction between the dates is invalid for the given rate definition.    
    pub fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        rate_definition: RateDefinition,
    ) -> Result<f64> {
        let discount_factor_to_start = self.discount_factor(start_date)?;
        let discount_factor_to_end = self.discount_factor(end_date)?;
        let comp_factor = discount_factor_to_start / discount_factor_to_end;
        let day_counter = rate_definition.day_counter();
        let tenor = day_counter.year_fraction(start_date, end_date);

        Ok(InterestRate::<f64>::implied_rate(
            comp_factor,
            day_counter,
            rate_definition.compounding(),
            rate_definition.frequency(),
            tenor,
        )?
        .rate())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rates::compounding::Compounding, time::enums::Frequency};

    #[test]
    fn forward_rate_uses_coupon_day_count() {
        let start = Date::new(2026, 1, 2);
        let end = Date::new(2027, 1, 2);
        for curve_day_counter in [DayCounter::Actual360, DayCounter::Actual365] {
            let curve = BootstrappedCurve::new(
                MarketIndex::SOFR,
                start,
                vec![0.0, curve_day_counter.year_fraction(start, end)],
                vec![1.0, 0.95],
                curve_day_counter,
                Interpolator::LogLinear,
            );
            let definition = RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Simple,
                Frequency::Annual,
            );
            let rate = curve.forward_rate(start, end, definition).unwrap();
            let growth = rate * definition.day_counter().year_fraction(start, end);
            assert!((growth - (1.0 / 0.95 - 1.0)).abs() < 1e-14);
        }
    }
}

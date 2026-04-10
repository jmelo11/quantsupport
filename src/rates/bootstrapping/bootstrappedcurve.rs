use crate::{
    ad::dual::DualFwd,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::{Interpolate, Interpolator},
    rates::interestrate::{InterestRate, RateDefinition},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

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
        }
    }

    /// Returns the market index of this curve.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
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
        self.interpolator
            .interpolate(year_fraction, &self.times, &self.discount_factors, true)
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
        let tenor = self.day_counter.year_fraction(start_date, end_date);

        Ok(InterestRate::<f64>::implied_rate(
            comp_factor,
            self.day_counter,
            rate_definition.compounding(),
            rate_definition.frequency(),
            tenor,
        )?
        .rate())
    }
}

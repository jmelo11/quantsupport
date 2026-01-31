use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    rates::{
        enums::Compounding,
        interestrate::RateDefinition,
        traits::{HasReferenceDate, YieldProvider},
        yieldtermstructure::traits::YieldTermStructureTrait,
    },
    time::{
        date::Date,
        enums::{Frequency, TimeUnit},
        period::Period,
    },
    utils::errors::{AtlasError, Result},
};

use super::{
    overnightindex::OvernightIndex,
    traits::{
        AdvanceInterestRateIndexInTime, FixingProvider, HasName, HasTenor, HasTermStructure,
        InterestRateIndexTrait, RelinkableTermStructure,
    },
};

/// # `OvernightCompoundedRateIndex`
/// Overnight index, used for overnight rates. Uses a price index (such as ICP) to calculate the overnight rates.
#[derive(Clone)]
pub struct OvernightCompoundedRateIndex {
    fixings_rates: HashMap<Date, f64>,
    overnight_index: OvernightIndex,
}

/// Calculates the overnight index value for a given period.
///
/// # Arguments
/// * `start_date` - The start date of the period
/// * `end_date` - The end date of the period
/// * `index` - The current index value
/// * `rate` - The overnight rate for the period
/// * `rate_definition` - The rate definition containing day counter information
///
/// # Returns
/// The updated index value
#[must_use]
pub fn calculate_overnight_index(
    start_date: Date,
    end_date: Date,
    index: f64,
    rate: f64,
    rate_definition: RateDefinition,
) -> f64 {
    let year_fraction = rate_definition
        .day_counter()
        .year_fraction(start_date, end_date);
    rate.mul_add(year_fraction, 1.0) * index
}

/// Composes a fixing index from overnight fixing rates.
///
/// # Arguments
/// * `fixings_rates` - A map of dates to overnight rates
/// * `rate_definition` - The rate definition containing day counter information
///
/// # Returns
/// A map of dates to computed index values
#[must_use]
pub fn compose_fixing_rate(
    fixings_rates: &HashMap<Date, f64, impl std::hash::BuildHasher>,
    rate_definition: RateDefinition,
) -> HashMap<Date, f64> {
    let mut fixings_rates = fixings_rates.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>();
    fixings_rates.sort_by(|a, b| a.0.cmp(&b.0));

    let mut fixing_index = HashMap::new();

    let mut index = 1000.0;
    fixing_index.insert(fixings_rates[0].0, index);

    for i in 1..fixings_rates.len() {
        let (fixing_date, applied_rate) = fixings_rates[i - 1];
        let current_date = fixings_rates[i].0;
        let new_index = calculate_overnight_index(
            fixing_date,
            current_date,
            index,
            applied_rate,
            rate_definition,
        );
        fixing_index.insert(current_date, new_index);
        index = new_index;
    }
    fixing_index
}

impl OvernightCompoundedRateIndex {
    /// Creates a new `OvernightCompoundedRateIndex` with the given reference date.
    #[must_use]
    pub fn new(reference_date: Date) -> Self {
        Self {
            fixings_rates: HashMap::new(),
            overnight_index: OvernightIndex::new(reference_date),
        }
    }

    /// Sets the name for this index.
    #[must_use]
    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.overnight_index = self.overnight_index.with_name(name);
        self
    }

    /// Returns the rate definition for this index.
    #[must_use]
    pub const fn rate_definition(&self) -> RateDefinition {
        self.overnight_index.rate_definition()
    }

    /// Sets the rate definition for this index.
    #[must_use]
    pub fn with_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
        self.overnight_index = self.overnight_index.with_rate_definition(rate_definition);
        self
    }

    /// Sets the overnight fixing rates for this index.
    #[must_use]
    pub fn with_fixings_rates(mut self, fixings_rates: HashMap<Date, f64>) -> Self {
        let fixing_index = compose_fixing_rate(&fixings_rates, self.rate_definition());
        self.fixings_rates = fixings_rates;
        self.overnight_index = self.overnight_index.with_fixings(fixing_index);
        self
    }

    /// Returns a reference to the fixing rates map.
    #[must_use]
    pub const fn fixings_rates(&self) -> &HashMap<Date, f64> {
        &self.fixings_rates
    }

    /// Sets the yield term structure for this index.
    #[must_use]
    pub fn with_term_structure(mut self, term_structure: Arc<dyn YieldTermStructureTrait>) -> Self {
        self.overnight_index = self.overnight_index.with_term_structure(term_structure);
        self
    }

    /// Calculates the average overnight rate between two dates.
    ///
    /// # Errors
    /// Returns an error if required fixings or rate data are unavailable.
    pub fn average_rate(&self, start_date: Date, end_date: Date) -> Result<f64> {
        self.overnight_index.average_rate(start_date, end_date)
    }
}

impl FixingProvider for OvernightCompoundedRateIndex {
    fn fixing(&self, date: Date) -> Result<f64> {
        self.overnight_index
            .fixings()
            .get(&date)
            .copied()
            .ok_or(AtlasError::NotFoundErr(format!(
                "No fixing for date {date} for index {name:?}",
                name = self.overnight_index.name()
            )))
    }

    fn fixings(&self) -> &HashMap<Date, f64> {
        self.overnight_index.fixings()
    }

    fn add_fixing(&mut self, date: Date, rate: f64) {
        self.overnight_index.add_fixing(date, rate);
    }
}

impl HasReferenceDate for OvernightCompoundedRateIndex {
    fn reference_date(&self) -> Date {
        self.overnight_index.reference_date()
    }
}

impl HasTenor for OvernightCompoundedRateIndex {
    fn tenor(&self) -> Period {
        self.overnight_index.tenor()
    }
}

impl HasName for OvernightCompoundedRateIndex {
    fn name(&self) -> Result<String> {
        self.overnight_index.name()
    }
}

impl YieldProvider for OvernightCompoundedRateIndex {
    fn discount_factor(&self, date: Date) -> Result<f64> {
        self.overnight_index.discount_factor(date)
    }

    fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<f64> {
        self.overnight_index
            .forward_rate(start_date, end_date, comp, freq)
    }
}

impl AdvanceInterestRateIndexInTime for OvernightCompoundedRateIndex {
    fn advance_to_period(&self, period: Period) -> Result<Arc<RwLock<dyn InterestRateIndexTrait>>> {
        self.overnight_index.advance_to_period(period)
    }

    fn advance_to_date(&self, date: Date) -> Result<Arc<RwLock<dyn InterestRateIndexTrait>>> {
        let days = i32::try_from(date - self.reference_date()).map_err(|_| {
            AtlasError::InvalidValueErr("Day count should fit in i32".to_string())
        })?;
        let period = Period::new(days, TimeUnit::Days);
        self.advance_to_period(period)
    }
}

impl HasTermStructure for OvernightCompoundedRateIndex {
    fn term_structure(&self) -> Result<Arc<dyn YieldTermStructureTrait>> {
        self.overnight_index.term_structure()
    }
}

impl RelinkableTermStructure for OvernightCompoundedRateIndex {
    fn link_to(&mut self, term_structure: Arc<dyn YieldTermStructureTrait>) {
        self.overnight_index.link_to(term_structure);
    }
}

impl InterestRateIndexTrait for OvernightCompoundedRateIndex {}

#[cfg(test)]
mod tests {
    use crate::{
        math::interpolation::interpolator::Interpolator,
        rates::yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
    };

    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_new_overnight_index() {
        let date = Date::new(2021, 1, 1);
        let overnight_index = OvernightCompoundedRateIndex::new(date);
        assert!(overnight_index.fixings_rates.is_empty());
    }

    #[test]
    fn test_with_rate_definition() {
        let date = Date::new(2021, 1, 1);
        let overnight_index =
            OvernightCompoundedRateIndex::new(date).with_rate_definition(RateDefinition::default());
        assert_eq!(overnight_index.rate_definition(), RateDefinition::default());
    }

    #[test]
    fn test_with_fixings() {
        let date = Date::new(2021, 1, 1);
        let mut fixings = HashMap::new();
        fixings.insert(Date::new(2021, 1, 1), 0.02);
        let overnight_index =
            OvernightCompoundedRateIndex::new(date).with_fixings_rates(fixings.clone());
        assert_eq!(*overnight_index.fixings_rates(), fixings);
    }

    #[test]
    fn test_average_rate() {
        let date = Date::new(2021, 1, 1);
        let mut fixings = HashMap::new();

        fixings.insert(Date::new(2021, 1, 1), 0.02);
        fixings.insert(Date::new(2021, 1, 2), 0.025);
        fixings.insert(Date::new(2021, 1, 3), 0.03);
        fixings.insert(Date::new(2021, 1, 4), 0.035);
        fixings.insert(Date::new(2021, 1, 5), 0.04);
        fixings.insert(Date::new(2021, 1, 6), 0.045);

        let overnight_index =
            OvernightCompoundedRateIndex::new(date).with_fixings_rates(fixings.clone());
        let average_rate = overnight_index
            .average_rate(Date::new(2021, 1, 2), Date::new(2021, 1, 5))
            .unwrap_or_else(|e| panic!("average_rate should succeed in test_average_rate: {e}"));
        assert!((average_rate - 0.03).abs() < 1e-5);
    }

    #[test]
    fn test_average_rate_disordered() {
        let date = Date::new(2021, 1, 1);
        let mut fixings = HashMap::new();

        fixings.insert(Date::new(2021, 1, 2), 0.025);
        fixings.insert(Date::new(2021, 1, 3), 0.03);
        fixings.insert(Date::new(2021, 1, 5), 0.04);
        fixings.insert(Date::new(2021, 1, 6), 0.045);
        fixings.insert(Date::new(2021, 1, 1), 0.02);
        fixings.insert(Date::new(2021, 1, 4), 0.035);

        let overnight_index =
            OvernightCompoundedRateIndex::new(date).with_fixings_rates(fixings.clone());
        let average_rate = overnight_index
            .average_rate(Date::new(2021, 1, 2), Date::new(2021, 1, 5))
            .unwrap_or_else(|e| {
                panic!("average_rate should succeed in test_average_rate_disordered: {e}")
            });
        assert!((average_rate - 0.03).abs() < 1e-5);
    }

    #[test]
    fn test_fixing() {
        let date = Date::new(2021, 1, 1);
        let mut fixings = HashMap::new();
        fixings.insert(Date::new(2021, 1, 1), 0.02);
        let overnight_index =
            OvernightCompoundedRateIndex::new(date).with_fixings_rates(fixings.clone());

        let fixing = overnight_index
            .fixing(Date::new(2021, 1, 1))
            .unwrap_or_else(|e| panic!("fixing should succeed in test_fixing: {e}"));
        assert!((fixing - 1000.0).abs() < 1e-10);
    }

    #[test]
    fn test_reference_date() {
        let date = Date::new(2021, 1, 1);
        let mut fixings = HashMap::new();
        let ref_date = Date::new(2021, 1, 1);

        fixings.insert(ref_date, 1.5);
        let overnight_index = OvernightCompoundedRateIndex::new(date)
            .with_fixings_rates(fixings.clone())
            .with_term_structure(Arc::new(FlatForwardTermStructure::new(
                ref_date,
                0.2,
                RateDefinition::default(),
            )));

        assert_eq!(overnight_index.reference_date(), ref_date);

        let next_date_2 = Date::new(2021, 1, 3);
        fixings.insert(next_date_2, 1.5);
        let overnight_index = OvernightCompoundedRateIndex::new(next_date_2)
            .with_term_structure(Arc::new(FlatForwardTermStructure::new(
                next_date_2,
                0.2,
                RateDefinition::default(),
            )))
            .with_fixings_rates(fixings.clone());

        assert_eq!(overnight_index.reference_date(), next_date_2);
    }

    #[test]
    fn test_fixing_provider_overnight() {
        let fixing: HashMap<Date, f64> =
            [(Date::new(2023, 6, 2), 2.5), (Date::new(2023, 6, 5), 3.0)]
                .iter()
                .copied()
                .collect();

        let mut overnight_index =
            OvernightCompoundedRateIndex::new(Date::new(2023, 6, 5)).with_fixings_rates(fixing);

        overnight_index.fill_missing_fixings(Interpolator::Linear);

        assert!(
            (overnight_index
                .fixings()
                .get(&Date::new(2023, 6, 3))
                .unwrap_or_else(|| panic!(
                    "fixings map should contain interpolated fixing for 2023-06-03"
                ))
                - 1006.944444)
                .abs()
                < 0.001
        );
    }
}

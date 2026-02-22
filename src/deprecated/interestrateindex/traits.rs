use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, RwLock},
};

use crate::{
    math::interpolation::interpolator::Interpolator,
    rates::{
        traits::{HasReferenceDate, YieldProvider},
        yieldtermstructure::traits::YieldTermStructureTrait,
    },
    time::{date::Date, enums::TimeUnit, period::Period},
    utils::errors::Result,
};

/// # `FixingProvider`
/// Implement this trait for a struct that provides fixing information.
pub trait FixingProvider {
    /// Returns the fixing rate for a given date.
    ///
    /// # Errors
    /// Returns an error if the fixing is unavailable for the requested date.
    fn fixing(&self, date: Date) -> Result<f64>;
    /// Returns a reference to the map of all fixings.
    fn fixings(&self) -> &HashMap<Date, f64>;
    /// Adds a fixing for a given date and rate.
    fn add_fixing(&mut self, date: Date, rate: f64);

    /// Fill missing fixings using interpolation.
    fn fill_missing_fixings(&mut self, interpolator: Interpolator) {
        if !self.fixings().is_empty() {
            let (first_date, last_date) =
                match (self.fixings().keys().min(), self.fixings().keys().max()) {
                    (Some(first), Some(last)) => (*first, *last),
                    _ => return,
                };

            let aux_btreemap = self
                .fixings()
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect::<BTreeMap<Date, f64>>();

            let x: Vec<f64> = aux_btreemap
                .keys()
                .map(|&d| {
                    let days = i32::try_from(d - first_date)
                        .unwrap_or_else(|_| panic!("fixing day count should fit in i32"));
                    f64::from(days)
                })
                .collect::<Vec<f64>>();

            let y = aux_btreemap.values().copied().collect::<Vec<f64>>();

            let mut current_date = first_date;

            while current_date <= last_date {
                if !self.fixings().contains_key(&current_date) {
                    let days = i32::try_from(current_date - first_date)
                        .unwrap_or_else(|_| panic!("fixing day count should fit in i32")); // fix this panic!
                    let days = f64::from(days);
                    let rate = interpolator.interpolate(days, &x, &y, false).unwrap(); // can this fail?
                    self.add_fixing(current_date, rate);
                }
                current_date = current_date + Period::new(1, TimeUnit::Days);
            }
        }
    }
}

// /// # InterestRateIndexClone
// /// Trait for cloning a given object.
// pub trait InterestRateIndexClone {
//     fn clone_box(&self) -> Box<dyn InterestRateIndexTrait>;
// }

// /// # InterestRateIndexClone for T
// impl<T: 'static + InterestRateIndexTrait + Clone> InterestRateIndexClone for T {
//     fn clone_box(&self) -> Box<dyn InterestRateIndexTrait> {
//         Box::new(self.clone())
//     }
// }

// /// # Clone for Box<dyn InterestRateIndexTrait>
// /// Implementation of Clone for Box<dyn InterestRateIndexTrait>.
// impl Clone for Box<dyn InterestRateIndexTrait> {
//     fn clone(&self) -> Self {
//         self.clone_box()
//     }
// }

/// # `AdvanceInterestRateIndexInTime`
/// Trait for advancing in time a given object. Returns a represation of the object
/// as it would be after the given period/time.
pub trait AdvanceInterestRateIndexInTime {
    /// Advances the index to a given period.
    ///
    /// # Errors
    /// Returns an error if the index cannot be advanced.
    fn advance_to_period(&self, period: Period) -> Result<Arc<RwLock<dyn InterestRateIndexTrait>>>;
    /// Advances the index to a given date.
    ///
    /// # Errors
    /// Returns an error if the index cannot be advanced.
    fn advance_to_date(&self, date: Date) -> Result<Arc<RwLock<dyn InterestRateIndexTrait>>>;
}
/// # `HasTenor`
/// Implement this trait for a struct that holds a tenor.
pub trait HasTenor {
    /// Returns the tenor of the interest rate index.
    fn tenor(&self) -> Period;
}

/// # `HasTermStructure`
/// Implement this trait for a struct that holds a term structure.
pub trait HasTermStructure {
    /// Returns the yield term structure associated with the index.
    ///
    /// # Errors
    /// Returns an error if the term structure is unavailable.
    fn term_structure(&self) -> Result<Arc<dyn YieldTermStructureTrait>>;
}

/// # `HasName`
/// Implement this trait for a struct that holds a name.
pub trait HasName {
    /// Returns the name of the interest rate index.
    ///
    /// # Errors
    /// Returns an error if the name is unavailable.
    fn name(&self) -> Result<String>;
}

/// # `RelinkableTermStructure`
/// Allows to link a term structure to another.
pub trait RelinkableTermStructure {
    /// Links the index to a new yield term structure.
    fn link_to(&mut self, term_structure: Arc<dyn YieldTermStructureTrait>);
}

/// # `InterestRateIndexTrait`
/// Implement this trait for a struct that holds interest rate index information.
pub trait InterestRateIndexTrait:
    FixingProvider
    + YieldProvider
    + HasReferenceDate
    + AdvanceInterestRateIndexInTime
    + HasTermStructure
    + RelinkableTermStructure
    + HasTenor
    + HasName
    + Send
    + Sync
{
}

use std::{
    collections::HashMap,
    sync::{Arc, RwLock, RwLockReadGuard},
};

use crate::{
    currencies::enums::Currency,
    time::{date::Date, enums::TimeUnit, period::Period},
    utils::errors::{AtlasError, Result},
};

use super::{
    interestrateindex::traits::InterestRateIndexTrait,
    yieldtermstructure::traits::YieldTermStructureTrait,
};

/// # `IndexStore`
/// A store for interest rate indices.
///
/// ## Parameters
/// * `reference_date` - The reference date of the index store
#[derive(Clone)]
pub struct IndexStore {
    reference_date: Date,
    index_map: HashMap<usize, Arc<RwLock<dyn InterestRateIndexTrait>>>,
    currency_curve: HashMap<Currency, usize>,
}

/// Trait for reading an interest rate index.
pub trait ReadIndex {
    /// Returns a read guard to the interest rate index.
    ///
    /// # Errors
    /// Returns an error if the index lock cannot be acquired for reading.
    fn read_index(&self) -> Result<RwLockReadGuard<'_, dyn InterestRateIndexTrait>>;
}

impl ReadIndex for Arc<RwLock<dyn InterestRateIndexTrait>> {
    fn read_index(&self) -> Result<RwLockReadGuard<'_, dyn InterestRateIndexTrait>> {
        self.read()
            .map_err(|_| AtlasError::InvalidValueErr("Could not read index".to_string()))
    }
}

impl IndexStore {
    /// Creates a new `IndexStore` with the given reference date.
    #[must_use]
    pub fn new(reference_date: Date) -> Self {
        Self {
            reference_date,
            index_map: HashMap::new(),
            currency_curve: HashMap::new(),
        }
    }

    /// Returns the reference date of this index store.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Adds a currency curve mapping to the store.
    pub fn add_currency_curve(&mut self, currency: Currency, fx_curve: usize) {
        self.currency_curve.insert(currency, fx_curve);
    }

    /// Retrieves the curve ID for the given currency.
    ///
    /// # Errors
    /// Returns an error if no curve is mapped to the requested currency.
    pub fn get_currency_curve(&self, currency: Currency) -> Result<usize> {
        self.currency_curve
            .get(&currency)
            .copied()
            .ok_or(AtlasError::NotFoundErr(format!(
                "Currency curve for currency {currency:?}"
            )))
    }

    /// Links a yield term structure to the index with the given ID.
    ///
    /// # Errors
    /// Returns an error if the index cannot be found or its lock cannot be written.
    pub fn link_term_structure(
        &self,
        id: usize,
        term_structure: Arc<dyn YieldTermStructureTrait>,
    ) -> Result<()> {
        self.index_map
            .get(&id)
            .ok_or(AtlasError::NotFoundErr(format!(
                "Index with id {id} not found"
            )))?
            .write()
            .map_err(|_| AtlasError::InvalidValueErr("Could not write index".to_string()))?
            .link_to(term_structure);
        Ok(())
    }

    /// Adds an index to the store with the given ID.
    ///
    /// # Errors
    /// Returns an error if the index reference date does not match or the ID already exists.
    pub fn add_index(
        &mut self,
        id: usize,
        index: Arc<RwLock<dyn InterestRateIndexTrait>>,
    ) -> Result<()> {
        if self.reference_date != index.read_index()?.reference_date() {
            return Err(AtlasError::InvalidValueErr(format!(
                "Index ({name:?}) reference date ({reference_date}) does not match index store reference date ({store_reference_date})",
                name = index.read_index()?.name(),
                reference_date = index.read_index()?.reference_date(),
                store_reference_date = self.reference_date
            )));
        }
        // check if name already exists
        if self.index_map.contains_key(&id) {
            return Err(AtlasError::InvalidValueErr(format!(
                "Index with id {id} already exists"
            )));
        }

        self.index_map.insert(id, index);

        Ok(())
    }

    /// Replaces an existing index in the store with the given ID.
    ///
    /// # Errors
    /// Returns an error if the index reference date does not match or the ID is missing.
    pub fn replace_index(
        &mut self,
        id: usize,
        index: Arc<RwLock<dyn InterestRateIndexTrait>>,
    ) -> Result<()> {
        if self.reference_date != index.read_index()?.reference_date() {
            return Err(AtlasError::InvalidValueErr(format!(
                "Index ({name:?}) reference date ({reference_date}) does not match index store reference date ({store_reference_date})",
                name = index.read_index()?.name(),
                reference_date = index.read_index()?.reference_date(),
                store_reference_date = self.reference_date
            )));
        }
        // check if name already exists
        if !self.index_map.contains_key(&id) {
            return Err(AtlasError::InvalidValueErr(format!(
                "Index with id {id} does not exist"
            )));
        }

        self.index_map.insert(id, index);

        Ok(())
    }

    /// Retrieves an index from the store by its ID.
    ///
    /// # Errors
    /// Returns an error if the index ID is not found.
    pub fn get_index(&self, id: usize) -> Result<Arc<RwLock<dyn InterestRateIndexTrait>>> {
        self.index_map
            .get(&id)
            .cloned()
            .ok_or(AtlasError::NotFoundErr(format!(
                "Index with id {id} not found"
            )))
    }

    /// Retrieves an index from the store by its name.
    ///
    /// # Errors
    /// Returns an error if the index cannot be read or the name is not found.
    pub fn get_index_by_name(&self, name: &str) -> Result<Arc<RwLock<dyn InterestRateIndexTrait>>> {
        for (id, index) in &self.index_map {
            if index.read_index()?.name()? == name {
                return self.get_index(*id);
            }
        }
        Err(AtlasError::NotFoundErr(format!(
            "Index with name {name} not found"
        )))
    }

    /// Returns a vector of all index names in the store.
    ///
    /// # Errors
    /// Returns an error if any index cannot be read to retrieve its name.
    pub fn get_index_names(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        for index in self.index_map.values() {
            names.push(index.read_index()?.name()?);
        }
        Ok(names)
    }

    /// Returns a mapping of index names to their IDs.
    ///
    /// # Errors
    /// Returns an error if any index cannot be read to retrieve its name.
    pub fn get_index_map(&self) -> Result<HashMap<String, usize>> {
        let mut map = HashMap::new();
        for (id, index) in &self.index_map {
            map.insert(index.read_index()?.name()?, *id);
        }
        Ok(map)
    }

    /// Returns all indices stored in this store.
    #[must_use]
    pub fn get_all_indices(&self) -> Vec<Arc<RwLock<dyn InterestRateIndexTrait>>> {
        let mut indices = Vec::new();
        for index in self.index_map.values() {
            indices.push(Arc::clone(index));
        }
        indices
    }

    /// Returns the next available index ID.
    #[must_use]
    pub fn next_available_id(&self) -> usize {
        let keys = self.index_map.keys();
        let mut max = 0;
        for key in keys {
            if *key > max {
                max = *key;
            }
        }
        max + 1
    }

    /// Advances the index store to a new reference date by the given period.
    ///
    /// # Errors
    /// Returns an error if any index cannot be advanced or reinserted.
    pub fn advance_to_period(&self, period: Period) -> Result<Self> {
        let reference_date = self.reference_date + period;
        let mut store = Self::new(reference_date);
        for (id, index) in &self.index_map {
            let new_index = index.read_index()?.advance_to_period(period)?;
            store.add_index(*id, new_index)?;
        }

        for (currency, curve) in &self.currency_curve {
            store.add_currency_curve(*currency, *curve);
        }

        Ok(store)
    }

    /// Advances the index store to a specific date.
    ///
    /// # Errors
    /// Returns an error if the store cannot be advanced to the target date.
    pub fn advance_to_date(&self, date: Date) -> Result<Self> {
        let days = i32::try_from(date - self.reference_date)
            .map_err(|_| AtlasError::InvalidValueErr("Day count should fit in i32".to_string()))?;
        self.advance_to_period(Period::new(days, TimeUnit::Days))
    }

    /// Swaps the index with the given source ID to the given target ID.
    ///
    /// # Errors
    /// Returns an error if the source index is missing.
    pub fn swap_index_by_id(&mut self, from: usize, to: usize) -> Result<()> {
        let index = self
            .index_map
            .remove(&from)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Index with id {from} not found")))?;
        self.index_map.insert(to, index);
        Ok(())
    }

    /// Calculates the currency forecast factor between two currencies at a given date.
    ///
    /// # Errors
    /// Returns an error if required currency curves or indices are missing.
    pub fn currency_forescast_factor(
        &self,
        first_currency: Currency,
        second_currency: Currency,
        date: Date,
    ) -> Result<f64> {
        let first_id = self.get_currency_curve(first_currency)?;
        let second_id = self.get_currency_curve(second_currency)?;

        let first_curve = self.get_index(first_id)?;
        let second_curve = self.get_index(second_id)?;

        let first_df = first_curve.read_index()?.discount_factor(date)?;
        let second_df = second_curve.read_index()?.discount_factor(date)?;

        Ok(second_df / first_df)
    }
}

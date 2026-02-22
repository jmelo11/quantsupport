use core::fmt;
use std::sync::{Arc, RwLock};

use crate::{
    currencies::{
        enums::Currency,
        exchangeratestore::ExchangeRateStore,
        traits::{AdvanceExchangeRateStoreInTime, CurrencyDetails},
    },
    rates::{
        indexstore::{IndexStore, ReadIndex},
        interestrateindex::traits::InterestRateIndexTrait,
        traits::HasReferenceDate,
    },
    time::{date::Date, enums::TimeUnit, period::Period},
    utils::{
        errors::{AtlasError, Result},
        tools,
    },
};

/// # `MarketStore`
/// A store for market data.
///
/// ## Parameters
/// * `reference_date` - The reference date of the market store
/// * `local_currency` - The local currency of the market store
/// * `exchange_rate_store` - The exchange rate store
/// * `index_store` - The index store
#[derive(Clone)]
pub struct MarketStore {
    reference_date: Date,
    local_currency: Currency,
    exchange_rate_store: ExchangeRateStore,
    index_store: IndexStore,
}

impl MarketStore {
    /// Creates a new `MarketStore` with the given reference date and local currency.
    #[must_use]
    pub fn new(reference_date: Date, local_currency: Currency) -> Self {
        Self {
            reference_date,
            local_currency,
            exchange_rate_store: ExchangeRateStore::new(reference_date),
            index_store: IndexStore::new(reference_date),
        }
    }

    /// Returns the local currency of this market store.
    #[must_use]
    pub const fn local_currency(&self) -> Currency {
        self.local_currency
    }

    /// Returns a reference to the exchange rate store.
    #[must_use]
    pub const fn exchange_rate_store(&self) -> &ExchangeRateStore {
        &self.exchange_rate_store
    }

    /// Returns a mutable reference to the exchange rate store.
    pub const fn mut_exchange_rate_store(&mut self) -> &mut ExchangeRateStore {
        &mut self.exchange_rate_store
    }

    /// Returns a reference to the index store.
    #[must_use]
    pub const fn index_store(&self) -> &IndexStore {
        &self.index_store
    }

    /// Returns a mutable reference to the index store.
    pub const fn mut_index_store(&mut self) -> &mut IndexStore {
        &mut self.index_store
    }

    /// Gets the exchange rate between two currencies.
    ///
    /// If no second currency is provided, uses the local currency.
    ///
    /// # Errors
    ///
    /// Returns an error if the exchange rate cannot be retrieved.
    pub fn get_exchange_rate(
        &self,
        first_currency: Currency,
        second_currency: Option<Currency>,
    ) -> Result<f64> {
        let second_currency = second_currency.unwrap_or(self.local_currency);
        self.exchange_rate_store
            .get_exchange_rate(first_currency, second_currency)
    }

    /// Gets an interest rate index by its ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the index cannot be found.
    pub fn get_index(&self, id: usize) -> Result<Arc<RwLock<dyn InterestRateIndexTrait>>> {
        self.index_store.get_index(id)
    }

    /// Advances the market store to a new date by the given period.
    ///
    /// # Errors
    ///
    /// Returns an error if the period is negative or advancing fails.
    pub fn advance_to_period(&self, period: Period) -> Result<Self> {
        if period.length() < 0 {
            return Err(AtlasError::InvalidValueErr(format!(
                "Negative periods are not allowed when advancing market store in time ({period:?})",
            )));
        }
        let new_reference_date = self.reference_date + period;
        let new_exchange_rate_store = self
            .exchange_rate_store
            .advance_to_period(period, &self.index_store)?;
        let new_index_store = self.index_store.advance_to_period(period)?;

        Ok(Self {
            reference_date: new_reference_date,
            local_currency: self.local_currency,
            exchange_rate_store: new_exchange_rate_store,
            index_store: new_index_store,
        })
    }

    /// Advances the market store to a specific date.
    ///
    /// # Errors
    ///
    /// Returns an error if the date is before the reference date or advancing fails.
    pub fn advance_to_date(&self, date: Date) -> Result<Self> {
        if date < self.reference_date {
            return Err(AtlasError::InvalidValueErr(format!(
                "Date {date} is before reference date {reference_date}",
                reference_date = self.reference_date
            )));
        }
        let days = i32::try_from(date - self.reference_date).map_err(|_| {
            AtlasError::InvalidValueErr(format!(
                "Date {date} is too far from reference date {reference_date} to convert to days",
                reference_date = self.reference_date
            ))
        })?;
        let period = Period::new(days, TimeUnit::Days);
        self.advance_to_period(period)
    }
}

impl HasReferenceDate for MarketStore {
    fn reference_date(&self) -> Date {
        self.reference_date
    }
}

// fecha, moneda, qué curvas tiene cargadas, paridades
impl fmt::Display for MarketStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut msg = "=====================================\n".to_string();
        msg.push_str("======= MarketStore features! =======\n");
        msg.push_str("=====================================\n");

        msg.push_str("> Reference Date: ");
        msg.push_str(&self.reference_date.to_string());
        msg.push('\n');
        msg.push_str("-------------------------------------\n");
        msg.push_str("> Currency: ");
        msg.push_str(self.local_currency.code());
        msg.push('\n');
        msg.push_str("-------------------------------------\n");

        let index_store = self.index_store();
        let all_indices = index_store.get_all_indices();

        let mut indices_names: Vec<String> = vec![];
        for indice in all_indices {
            if let Ok(indice) = indice.read_index() {
                if let Ok(name) = indice.name() {
                    indices_names.push(name);
                }
            }
        }
        let indices_map = index_store.get_index_map().unwrap_or_default();

        indices_names = tools::sort_strings_alphabetically(&indices_names);

        msg.push_str("> Indices (");
        msg.push_str(&indices_names.len().to_string());
        msg.push_str("):\n");
        for indice_name in indices_names {
            let indice_idx = indices_map
                .get(&indice_name)
                .map_or_else(|| "Unknown".to_string(), std::string::ToString::to_string);

            msg.push_str(">> ");
            msg.push_str(&indice_idx);
            msg.push_str(" -> ");
            msg.push_str(&indice_name);
            msg.push('\n');
        }

        let exchange_rate_store = self.exchange_rate_store();
        let exchange_rate_map = exchange_rate_store.get_exchange_rate_map();
        msg.push_str("-------------------------------------\n");
        msg.push_str("> Currency pairs (");
        msg.push_str(&exchange_rate_map.len().to_string());
        msg.push_str("):\n");
        for (currencies, value) in &exchange_rate_map {
            msg.push_str(">> ");
            msg.push_str(currencies.0.code());
            msg.push_str(" -> ");
            msg.push_str(currencies.1.code());
            msg.push_str(": ");
            msg.push_str(&value.to_string());
            msg.push('\n');
        }

        msg.push_str("=====================================\n");

        write!(f, "{msg}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_does_not_panic() {
        let reference_date = Date::new(2024, 1, 1);
        let market_store = MarketStore::new(reference_date, Currency::USD);

        let result = std::panic::catch_unwind(|| {
            let _ = format!("{}", market_store);
        });

        assert!(result.is_ok());
    }
}

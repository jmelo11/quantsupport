use crate::{
    time::{date::Date, period::Period},
    utils::errors::Result,
};

use super::exchangeratestore::ExchangeRateStore;

/// # `CurrencyDetails`
/// Trait for currency details
pub trait CurrencyDetails {
    /// Returns the ISO 4217 currency code
    fn code(&self) -> &'static str;
    /// Returns the name of the currency
    fn name(&self) -> &'static str;
    /// Returns the currency symbol
    fn symbol(&self) -> &'static str;
    /// Returns the number of decimal places for the currency
    fn precision(&self) -> u8;
    /// Returns the ISO 4217 numeric code
    fn numeric_code(&self) -> u16;
}

/// # `AdvanceExchangeRateStoreInTime`
/// Trait for advancing an exchange rate store in time using de index store
/// It is necessary for any currency, have free risk curve tabulated in the index store
/// If the currency does not have a free risk curve, method `advance_to_period` and `advance_to_date` will mantain the same fx
pub trait AdvanceExchangeRateStoreInTime {
    /// Advances the exchange rate store to a specific period using the index store
    ///
    /// # Errors
    ///
    /// Returns an error if advancing the exchange rate store in time fails.
    fn advance_to_period(
        &self,
        period: Period,
        index_store: &IndexStore,
    ) -> Result<ExchangeRateStore>;
    /// Advances the exchange rate store to a specific date using the index store
    ///
    /// # Errors
    ///
    /// Returns an error if advancing the exchange rate store in time fails.
    fn advance_to_date(&self, date: Date, index_store: &IndexStore) -> Result<ExchangeRateStore>;
}

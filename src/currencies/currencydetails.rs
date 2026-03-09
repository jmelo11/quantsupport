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

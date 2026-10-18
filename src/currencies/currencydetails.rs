/// Trait for currency details.
///
/// This trait defines the interface for retrieving details about a currency, such as its ISO code, name, symbol,
/// precision, and numeric code. It is implemented by the [`Currency`](crate::currencies::currency::Currency) enum to provide consistent
/// access to these details across the library.
pub trait CurrencyDetails {
    /// Returns the currency identifier, including market identifiers such as CNH.
    fn code(&self) -> &'static str;
    /// Returns the name of the currency
    fn name(&self) -> &'static str;
    /// Returns the currency symbol
    fn symbol(&self) -> &'static str;
    /// Returns decimal-place metadata, without rounding monetary calculations.
    fn precision(&self) -> u8;
    /// Returns the ISO 4217 numeric code
    fn numeric_code(&self) -> u16;
}

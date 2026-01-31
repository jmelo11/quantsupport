/// # `Instrument`
/// The `Instrument` trait catalogs any financial product. Financial product have
/// more charasteristics (i.e. start date, initial spread, strike, etc.) that structs that implement
/// `Instrument` could provide.
pub trait Instrument: Send + Sync {
    /// Provides the unique id that is related to the instrument.
    /// ## Returns
    /// The id of the instrument.
    fn id(&self) -> usize;
    /// Market-associated name of the instrument. For example, it could be the name of the stock, CUSIP of a bond, among others.
    /// ## Returns
    /// The name of the instrument.
    fn identifier(&self) -> &str;
}

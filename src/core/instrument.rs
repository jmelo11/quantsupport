use crate::{core::pricingdata::PricingDataContext, utils::errors::Result};

/// # `Instrument`
///
/// The `Instrument` trait catalogs any financial product. Financial product have
/// more charasteristics (i.e. start date, initial spread, strike, etc.) that structs that implement
/// `Instrument` could provide.
pub trait Instrument: Send + Sync {
    /// Market-associated name of the instrument. For example, it could be the name of the stock, CUSIP of a bond, among others.
    fn identifier(&self) -> String;

    /// Checks if the instrument is fully resolved. An instrument is considered resolved when all its required fields to perform pricing are set.
    fn is_resolved(&self) -> bool;

    /// Resolves the instrument by filling in any missing required fields. This may involve fetching data from external sources or performing calculations.
    fn resolve(&mut self, ctx: &PricingDataContext) -> Result<()>;
}

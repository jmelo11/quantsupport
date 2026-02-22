use crate::{core::contextmanager::ContextManager, utils::errors::Result};

/// # `Instrument`
///
/// The `Instrument` trait catalogs any financial product. Financial product have
/// more charasteristics (i.e. start date, initial spread, strike, etc.) that structs that implement
/// `Instrument` could provide.
pub trait Instrument: Send + Sync + Sized {
    /// Market-associated name of the instrument. For example, it could be the name of the stock, CUSIP of a bond, among others.
    fn identifier(&self) -> String;

    /// Resolves the instrument by filling in any missing required fields. This may involve fetching data from external sources or performing calculations.
    ///
    /// ## Errors
    /// Returns an error if the instrument cannot be resolved due to missing data or other issues.
    fn resolve(&self, ctx: &ContextManager) -> Result<Self>;
}

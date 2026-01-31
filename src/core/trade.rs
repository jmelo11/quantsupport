use crate::core::instrument::Instrument;

/// # `Trade`
pub trait Trade<I: Instrument>: Send + Sync {
    /// Returns the id of the trait. Not to be confused with the instrument id.
    fn id(&self) -> usize;
    /// Returns the associated instrument of the trade.
    fn instrument(&self) -> I;
}

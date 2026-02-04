use crate::core::instrument::Instrument;

/// # `Trade`
pub trait Trade<I: Instrument>: Send + Sync {
    /// Returns the associated instrument of the trade.
    fn instrument(&self) -> I;
}

use crate::core::instrument::Instrument;

/// # `Trade`
///
/// Represent a trade over a particular instrument.
pub trait Trade<I: Instrument>: Send + Sync {
    /// Returns the associated instrument of the trade.
    fn instrument(&self) -> I;
}

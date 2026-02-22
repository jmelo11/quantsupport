use crate::{core::instrument::Instrument, time::date::Date};

/// # `Trade`
///
/// Represent a trade over a particular instrument.
pub trait Trade<I: Instrument>: Send + Sync {
    /// Returns the associated instrument of the trade.
    fn instrument(&self) -> I;
    /// Date of execution of the trade.
    fn trade_date(&self) -> Date;
}

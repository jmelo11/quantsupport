use crate::{core::instrument::Instrument, time::date::Date};

/// A [`Side`] representing the direction of the cashflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Paying or shorting a position.
    PayShort,
    /// Recieve or being long a position.
    LongRecieve,
}

impl Side {
    /// Returns the sign associated with the side, where `PayShort` corresponds to a positive sign and `LongRecieve` corresponds to a negative sign.
    #[must_use]
    pub const fn sign(&self) -> f64 {
        match self {
            Self::PayShort => 1.0,
            Self::LongRecieve => -1.0,
        }
    }
}

/// A [`Trade<I>`] represent a position taken on a particular instrument.
pub trait Trade<I: Instrument>: Send + Sync {
    /// Returns the associated instrument of the trade.
    fn instrument(&self) -> &I;

    /// Date of execution of the trade.
    fn trade_date(&self) -> Date;

    /// Side associated with the trade.
    fn side(&self) -> Side;
}

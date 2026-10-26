use crate::{core::instrument::Instrument, time::date::Date};
use crate::{
    core::resolution::{ResolutionContext, Resolve},
    utils::errors::Result,
};
use std::sync::Arc;

type TradeResolver<T> = dyn Fn(&ResolutionContext<'_>) -> Result<T> + Send + Sync;

/// A trade whose instrument or position dates are resolved when it is evaluated.
///
/// Store a maker and a position constructor with [`Self::from_maker`]. A native
/// [`Evaluator`](crate::core::evaluator::Evaluator) resolves this trade against
/// the supplied market before selecting the ordinary concrete trade's pricer.
/// Each evaluation creates a fresh contract; neither this object nor its maker
/// is changed. Retain a resolved trade when contractual terms must stay fixed
/// between valuations or scenarios.
pub struct UnresolvedTrade<T> {
    resolver: Arc<TradeResolver<T>>,
}

impl<T> Clone for UnresolvedTrade<T> {
    fn clone(&self) -> Self {
        Self {
            resolver: Arc::clone(&self.resolver),
        }
    }
}

impl<T: 'static> UnresolvedTrade<T> {
    /// Stores a context-dependent constructor for a concrete trade.
    #[must_use]
    pub fn new<F>(resolver: F) -> Self
    where
        F: Fn(&ResolutionContext<'_>) -> Result<T> + Send + Sync + 'static,
    {
        Self {
            resolver: Arc::new(resolver),
        }
    }

    /// Retains a maker and supplies its resolved instrument to a position constructor.
    /// The constructor can use the context reference date for execution or settlement.
    #[must_use]
    pub fn from_maker<M, F>(maker: M, position: F) -> Self
    where
        M: Resolve + Send + Sync + 'static,
        F: Fn(M::Resolved, &ResolutionContext<'_>) -> Result<T> + Send + Sync + 'static,
    {
        Self::new(move |context| position(maker.resolve(context)?, context))
    }
}

impl<T> Resolve for UnresolvedTrade<T> {
    type Resolved = T;

    fn resolve(&self, context: &ResolutionContext<'_>) -> Result<T> {
        (self.resolver)(context)
    }
}

/// A [`Side`] representing the direction of the cashflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Paying or shorting a position.
    PayShort,
    /// Receive or being long a position.
    LongReceive,
}

impl Side {
    /// Returns the sign associated with the side: `LongReceive` (incoming cashflows) is positive,
    /// `PayShort` (outgoing cashflows) is negative.
    #[must_use]
    pub const fn sign(&self) -> f64 {
        match self {
            Self::PayShort => -1.0,
            Self::LongReceive => 1.0,
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

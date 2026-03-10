use crate::{
    core::{
        evaluationresults::EvaluationResults,
        marketdatahandling::marketdata::{MarketDataProvider, MarketDataRequest},
        request::Request,
    },
    utils::errors::QSError,
};

/// The [`Pricer`] trait should be implemented by any instrument pricing methodology. Implementers
/// must also implement [`Send`] and [`Sync`].
pub trait Pricer: Send + Sync {
    /// The associated instrument to be priced.
    type Item;
    /// The discount policy type supported by this pricer.
    type Policy: ?Sized + Send + Sync;
    ///
    /// Evaluates the instrument over a [`Request`] given a [`ContextManager`](crate::core::contextmanager::ContextManager).
    ///
    /// ## Arguments
    /// * `trade`: the associated instrument that this pricer is capable of handeling.
    /// * `requests`: a slice containing the different [`Request`] that being required to resolve.
    /// * `ctx`: an implementation of [`MarketDataProvider`] that can be used to resolve market data requests and access constructed market data elements during the evaluation process.
    ///
    /// ## Returns
    /// Returns [`EvaluationResults`] if the evaluation succeded.
    ///
    /// ## Errors
    /// Returns an [`QSError`] if the evaluation fails.
    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults, QSError>;

    /// Returns a [`MarketDataRequest`] containing the market data elements required to evaluate the instrument.
    ///
    /// ## Arguments
    /// * `trade`: the associated instrument that this pricer is capable of handeling.
    ///
    /// ## Returns
    /// Returns a [`MarketDataRequest`] if the pricer requires market data to evaluate the instrument, otherwise returns `None`.
    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest>;

    /// Attaches a [`DiscountPolicy`](crate::core::collateral::DiscountPolicy) that overrides default discounting and
    /// can define both discount-curve and pricing-currency resolution.
    fn set_discount_policy(&mut self, _policy: Box<Self::Policy>);

    /// Returns the currently active [`DiscountPolicy`](crate::core::collateral::DiscountPolicy), if any.
    fn discount_policy(&self) -> Option<&Self::Policy>;
}

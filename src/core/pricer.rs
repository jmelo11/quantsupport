use crate::{
    core::{
        evaluationresults::EvaluationResults,
        marketdatahandling::marketdata::{MarketDataProvider, MarketDataRequest},
        request::Request,
    },
    utils::errors::AtlasError,
};

/// The [`Pricer`] trait should be implemented by any instrument pricing methodology. Implementers
/// must also implement [`Send`] and [`Sync`].
pub trait Pricer: Send + Sync {
    /// The associated instrument to be priced.
    type Item;
    /// The discount policy type supported by this pricer.
    type Policy: ?Sized + Send + Sync;
    ///
    /// Evaluates the instrument over a [`Request`] given a [`ContextManager`].
    ///
    /// ## Arguments
    /// * `trade`: the associated instrument that this pricer is capable of handeling.
    /// * `requests`: a slice containing the different [`Request`] that being required to resolve.
    /// * `ctx`: a [`ContextManager`].
    ///
    /// ## Returns
    /// Returns [`EvaluationResults`] if the evaluation succeded.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the evaluation fails.
    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults, AtlasError>;

    /// Returns a [`MarketDataRequest`] containing the market data elements required to evaluate the instrument.
    ///
    /// ## Arguments
    /// * `trade`: the associated instrument that this pricer is capable of handeling.
    ///
    /// ## Returns
    /// Returns a [`MarketDataRequest`] if the pricer requires market data to evaluate the instrument, otherwise returns `None`.
    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest>;

    /// Attaches a [`DiscountPolicy`] that overrides default discounting and
    /// can define both discount-curve and pricing-currency resolution.
    fn set_discount_policy(&mut self, _policy: Box<Self::Policy>) {}

    /// Returns the currently active [`DiscountPolicy`], if any.
    fn discount_policy(&self) -> Option<&Self::Policy> {
        None
    }
}

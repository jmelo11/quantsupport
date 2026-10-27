use std::any::Any;

use crate::{
    core::{
        evaluationresults::EvaluationResults,
        marketdatahandling::marketdata::{MarketDataProvider, MarketDataRequest},
        request::Request,
    },
    utils::errors::{QSError, Result},
};

/// Valuation method for an associated position type.
pub trait Pricer: Send + Sync {
    /// Position type accepted by this pricer.
    type Item;
    /// The discount policy type supported by this pricer.
    type Policy: ?Sized + Send + Sync;

    /// Evaluates a position for the requested result measures using a market provider.
    ///
    /// ## Arguments
    /// * `trade`: the position type accepted by this pricer.
    /// * `requests`: result measures, such as value or sensitivities. Support is
    ///   specific to the implementation; these are not market-data requests.
    /// * `ctx`: provider of the evaluation date and required market inputs.
    ///
    /// ## Returns
    /// Returns [`EvaluationResults`]. Check its optional fields rather than
    /// interpreting an absent measure as zero.
    ///
    /// ## Errors
    /// Propagates invalid-contract, missing-market-data, calibration, and
    /// numerical errors encountered by the implementation. Unsupported result
    /// requests may be rejected or left absent; consult the concrete pricer.
    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults>;

    /// Returns a [`MarketDataRequest`] containing the market data elements required to evaluate the instrument.
    ///
    /// ## Arguments
    /// * `trade`: the position whose dependencies are required.
    ///
    /// ## Returns
    /// Returns a [`MarketDataRequest`] if the pricer requires market data to evaluate the instrument, otherwise returns `None`.
    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest>;

    /// Attaches a [`DiscountPolicy`](crate::core::collateral::DiscountPolicy) that overrides discount-curve selection.
    fn set_discount_policy(&mut self, _policy: Box<Self::Policy>);

    /// Returns the currently active [`DiscountPolicy`](crate::core::collateral::DiscountPolicy), if any.
    fn discount_policy(&self) -> Option<&Self::Policy>;
}

/// Type-erased valuation interface for runtime trade dispatch.
pub trait ErasedPricer: Send + Sync {
    /// Whether this pricer accepts the supplied concrete trade.
    fn accepts_trade(&self, _trade: &dyn Any) -> bool {
        false
    }
    /// Evaluates a type-erased trade for the requested measures.
    ///
    /// # Errors
    /// Returns an error if the trade type is unsupported or evaluation fails.
    ///
    /// ## Arguments
    /// * `trade`: position to downcast to the concrete pricer's item type.
    /// * `requests`: result measures to compute, not market-data requests.
    /// * `ctx`: provider of the evaluation date and required market data.
    ///
    /// ## Returns
    /// Returns [`EvaluationResults`] with the measures produced by the pricer.
    fn evaluate_erased(
        &self,
        trade: &dyn Any,
        requests: &[Request],
        ctx: &dyn MarketDataProvider,
    ) -> Result<EvaluationResults>;
}

/// Blanket implementation: every [`Pricer`] automatically supports type-erased
/// evaluation.
///
/// The incoming `&dyn Any` trade is downcast to the pricer's associated
/// [`Pricer::Item`] and dispatched to [`Pricer::evaluate`]. This means any new
/// pricer only needs to implement [`Pricer`] to be registrable with the
/// [`Evaluator`](crate::core::evaluator::Evaluator).
impl<P> ErasedPricer for P
where
    P: Pricer,
    P::Item: Any,
{
    fn accepts_trade(&self, trade: &dyn Any) -> bool {
        trade.is::<P::Item>()
    }

    fn evaluate_erased(
        &self,
        trade: &dyn Any,
        requests: &[Request],
        ctx: &dyn MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let typed_trade = trade.downcast_ref::<P::Item>().ok_or_else(|| {
            QSError::InvalidValueErr(format!(
                "Pricer expected a trade of type `{}`, but received a value with type id {:?}",
                std::any::type_name::<P::Item>(),
                trade.type_id()
            ))
        })?;
        self.evaluate(typed_trade, requests, &ctx)
    }
}

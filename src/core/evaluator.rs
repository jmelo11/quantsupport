use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::utils::errors::QSError;
use crate::{
    core::{
        evaluationresults::EvaluationResults, pricer::ErasedPricer, pricingcontext::PricingContext,
        request::Request,
    },
    utils::errors::Result,
};

/// Dispatches pricing requests to registered [`ErasedPricer`] implementations.
#[derive(Default)]
pub struct Evaluator {
    // Models should be passed somewhere...
    pricers: HashMap<TypeId, Box<dyn ErasedPricer>>,
}

impl Evaluator {
    /// Creates a new [`Evaluator`] with the specified models and pricers.
    #[must_use]
    pub fn new(pricers: HashMap<TypeId, Box<dyn ErasedPricer>>) -> Self {
        Self { pricers }
    }

    /// Evaluates the given trade using the registered models and pricers, returning the evaluation results.
    ///
    /// # Errors
    /// Returns an error if no pricer is registered for the trade type or if evaluation fails.
    pub fn evaluate(
        &self,
        trade: &dyn Any,
        requests: &[Request],
        context: &PricingContext,
    ) -> Result<EvaluationResults> {
        let trade_type_id = trade.type_id();
        self.pricers.get(&trade_type_id).map_or_else(
            || {
                Err(QSError::NotFoundErr(format!(
                    "No pricer registered for trade type: {trade_type_id:?}"
                )))
            },
            |pricer| pricer.evaluate_erased(trade, requests, context),
        )
    }
}

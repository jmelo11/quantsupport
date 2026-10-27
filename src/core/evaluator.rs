use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::utils::errors::QSError;
use crate::{
    core::{
        evaluationresults::EvaluationResults, marketdatahandling::marketdata::MarketDataProvider,
        pricer::ErasedPricer, request::Request,
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
    /// Creates a dispatcher keyed by each accepted position's [`TypeId`].
    ///
    /// Register the position type, not the instrument or pricer type.
    /// Models and market inputs are supplied separately through the provider.
    #[must_use]
    pub fn new(pricers: HashMap<TypeId, Box<dyn ErasedPricer>>) -> Self {
        Self { pricers }
    }

    /// Dispatches a position to its registered pricer using the supplied market.
    ///
    /// # Errors
    /// Returns an error if no pricer is registered for the trade type or if evaluation fails.
    pub fn evaluate(
        &self,
        trade: &dyn Any,
        requests: &[Request],
        context: &dyn MarketDataProvider,
    ) -> Result<EvaluationResults> {
        Request::validate(requests)?;
        let trade_type_id = trade.type_id();
        self.pricers
            .get(&trade_type_id)
            .or_else(|| {
                self.pricers
                    .values()
                    .find(|pricer| pricer.accepts_trade(trade))
            })
            .map_or_else(
                || {
                    Err(QSError::NotFoundErr(format!(
                        "No pricer registered for trade type: {trade_type_id:?}"
                    )))
                },
                |pricer| pricer.evaluate_erased(trade, requests, context),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::collections::HashMap;

    use super::Evaluator;
    use crate::{
        core::{
            evaluationresults::EvaluationResults,
            marketdatahandling::marketdata::{MarketDataProvider, MarketDataRequest},
            pricer::{ErasedPricer, Pricer},
            pricingcontext::PricingContext,
            request::Request,
        },
        utils::errors::Result,
    };

    struct DummyTrade;
    struct UnregisteredTrade;

    struct DummyPricer;

    impl Pricer for DummyPricer {
        type Item = DummyTrade;
        type Policy = ();

        fn evaluate(
            &self,
            _trade: &DummyTrade,
            _requests: &[Request],
            ctx: &impl MarketDataProvider,
        ) -> Result<EvaluationResults> {
            Ok(EvaluationResults::new(ctx.evaluation_date(), "dummy".to_string()).with_price(42.0))
        }

        fn market_data_request(&self, _trade: &DummyTrade) -> Option<MarketDataRequest> {
            None
        }

        fn set_discount_policy(&mut self, _policy: Box<Self::Policy>) {}

        fn discount_policy(&self) -> Option<&Self::Policy> {
            None
        }
    }

    #[test]
    fn dispatches_to_registered_pricer_by_trade_type() -> Result<()> {
        let mut pricers: HashMap<TypeId, Box<dyn ErasedPricer>> = HashMap::new();
        pricers.insert(TypeId::of::<DummyTrade>(), Box::new(DummyPricer));
        let evaluator = Evaluator::new(pricers);

        let ctx = PricingContext::new();
        let results = evaluator.evaluate(&DummyTrade, &[Request::Value], &ctx)?;
        assert_eq!(results.price(), Some(42.0));
        Ok(())
    }

    #[test]
    fn unregistered_trade_type_is_an_error() {
        let evaluator = Evaluator::new(HashMap::new());
        let ctx = PricingContext::new();
        let result = evaluator.evaluate(&UnregisteredTrade, &[Request::Value], &ctx);
        assert!(
            result.is_err(),
            "expected error for unregistered trade type"
        );
    }

    #[test]
    fn mismatched_registration_surfaces_downcast_error() {
        // Register the pricer under the wrong trade type id: dispatch succeeds
        // but the blanket ErasedPricer downcast must fail with a clear error.
        let mut pricers: HashMap<TypeId, Box<dyn ErasedPricer>> = HashMap::new();
        pricers.insert(TypeId::of::<UnregisteredTrade>(), Box::new(DummyPricer));
        let evaluator = Evaluator::new(pricers);

        let ctx = PricingContext::new();
        let result = evaluator.evaluate(&UnregisteredTrade, &[Request::Value], &ctx);
        let err = match result {
            Ok(_) => panic!("expected downcast failure"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("DummyTrade"),
            "error should name the expected trade type, got: {err}"
        );
    }
}

#[cfg(test)]
mod resolution_tests {
    use crate::{
        core::makeinstrument::test_support::{loan, market, reference},
        prelude::*,
    };

    #[test]
    fn explicitly_built_trade_is_valued_without_rebuilding_contract_terms() -> Result<()> {
        use crate::core::{evaluator::Evaluator, pricer::ErasedPricer};
        use std::{any::TypeId, collections::HashMap};
        let mut pricer = LoanPricer::new();
        pricer.set_discount_policy(Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_discount_curve_index(Currency::EUR, MarketIndex::ESTR),
        ));
        let mut registrations: HashMap<TypeId, Box<dyn ErasedPricer>> = HashMap::new();
        registrations.insert(TypeId::of::<LoanTrade<DualFwd>>(), Box::new(pricer));
        let evaluator = Evaluator::new(registrations);
        let low = market(reference(), 0.02, FixingStore::default());
        let high = market(reference(), 0.08, FixingStore::default());
        let concrete = LoanTrade::new(loan()?.build_using(&low)?, reference(), Side::LongReceive);
        let principal = concrete.instrument().notional();
        let first = evaluator.evaluate(&concrete, &[Request::Value], &low)?;
        let changed = evaluator.evaluate(&concrete, &[Request::Value], &high)?;
        let repeated = evaluator.evaluate(&concrete, &[Request::Value], &low)?;
        assert_ne!(first.price(), changed.price());
        assert_eq!(first.price(), repeated.price());
        assert_eq!(concrete.instrument().notional(), principal);
        Ok(())
    }
}

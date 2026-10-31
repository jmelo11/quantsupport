//! Portfolio valuation with incremental numeric aggregation.

use crate::{
    core::{
        cashflowaggregation::{CashflowAggregator, GrossCashflowAggregator},
        evaluationresults::EvaluationResults,
        makeportfolioaccumulator::MakePortfolioAccumulator,
        marketdatahandling::marketdata::{MarketDataProvider, MarketDataRequest},
        portfolio::{Portfolio, PortfolioAccumulator},
        pricer::Pricer,
        request::Request,
    },
    utils::errors::Result,
};

/// Prices portfolio members and aggregates their completed numeric results.
///
/// The member pricer owns market-data requests, discounting, currency conversion
/// and differentiation. Completed numeric results are accumulated in trade order,
/// releasing each member result before pricing the next member.
///
/// Supports value, sensitivities, cashflows and accrued amount. Unsupported requests are ignored;
/// a measure omitted by any member remains absent from the portfolio result.
/// Monetary outputs share the member pricer's base currency; cashflows retain
/// their original currencies and coupon terms.
///
/// # Example
/// ```
/// use quantsupport::prelude::*;
///
/// let member_pricer = DiscountedCashflowPricer::<
///     FixedRateDeposit<DualFwd>, FixedRateDepositTrade<DualFwd>,
/// >::new();
/// let portfolio_pricer = PortfolioPricer::new(member_pricer);
/// // portfolio_pricer.evaluate(&portfolio, &[Request::Value], &context)?;
/// ```
pub struct PortfolioPricer<P, F = fn() -> GrossCashflowAggregator> {
    member_pricer: P,
    cashflow_aggregator: F,
}

impl<P> PortfolioPricer<P> {
    /// Creates a portfolio pricer using the supplied member pricer and gross cashflow aggregation.
    #[must_use]
    pub fn new(member_pricer: P) -> Self {
        Self {
            member_pricer,
            cashflow_aggregator: GrossCashflowAggregator::default,
        }
    }
}

impl<P, F> PortfolioPricer<P, F> {
    /// Selects a factory that creates a fresh cashflow aggregation strategy for each evaluation.
    #[must_use]
    pub fn with_cashflow_aggregator_factory<G>(self, factory: G) -> PortfolioPricer<P, G> {
        PortfolioPricer {
            member_pricer: self.member_pricer,
            cashflow_aggregator: factory,
        }
    }
}

impl<P, F, A> Pricer for PortfolioPricer<P, F>
where
    P: Pricer,
    F: Fn() -> A + Send + Sync,
    A: CashflowAggregator,
{
    type Item = Portfolio<P::Item>;
    type Policy = P::Policy;

    fn evaluate(
        &self,
        portfolio: &Self::Item,
        requests: &[Request],
        context: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        Request::validate(requests)?;
        let mut accumulator = MakePortfolioAccumulator::default()
            .with_requests(requests)
            .with_cashflow_aggregator((self.cashflow_aggregator)())
            .build()?;
        let requests = accumulator.requests().to_vec();
        for (index, trade) in portfolio.trades().iter().enumerate() {
            let result = self.evaluate_member(trade, &requests, context, index)?;
            accumulate_member_result(&mut accumulator, &result, index)?;
        }
        accumulator.into_evaluation_results(portfolio.identifier().to_owned())
    }

    fn market_data_request(&self, _portfolio: &Self::Item) -> Option<MarketDataRequest> {
        // Members request their own data during valuation; dependencies are not combined.
        None
    }

    fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
        self.member_pricer.set_discount_policy(policy);
    }

    fn discount_policy(&self) -> Option<&Self::Policy> {
        self.member_pricer.discount_policy()
    }
}

impl<P, F> PortfolioPricer<P, F>
where
    P: Pricer,
    F: Send + Sync,
{
    /// Prices one member and adds its portfolio position to any valuation error.
    fn evaluate_member(
        &self,
        trade: &P::Item,
        requests: &[Request],
        context: &impl MarketDataProvider,
        index: usize,
    ) -> Result<EvaluationResults> {
        self.member_pricer
            .evaluate(trade, requests, context)
            .map_err(|error| error.context(format!("pricing portfolio member {}", index + 1)))
    }
}

/// Adds one completed result and identifies the member if numerical aggregation fails.
fn accumulate_member_result<A: CashflowAggregator>(
    accumulator: &mut PortfolioAccumulator<A>,
    result: &EvaluationResults,
    index: usize,
) -> Result<()> {
    accumulator
        .accumulate_trade_result(result)
        .map_err(|error| {
            error.context(format!(
                "aggregating portfolio member {} ({})",
                index + 1,
                result.identifier()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{evaluator::Evaluator, pricer::ErasedPricer, pricingcontext::PricingContext};
    use std::{any::TypeId, collections::HashMap};

    struct ConstantPricer;

    impl Pricer for ConstantPricer {
        type Item = ();
        type Policy = ();

        fn evaluate(
            &self,
            _trade: &(),
            requests: &[Request],
            context: &impl MarketDataProvider,
        ) -> Result<EvaluationResults> {
            let mut result = EvaluationResults::new(context.evaluation_date(), "member".into());
            if requests.contains(&Request::Value) {
                result = result.with_price(42.0);
            }
            Ok(result)
        }
        fn market_data_request(&self, _trade: &()) -> Option<MarketDataRequest> {
            None
        }
        fn set_discount_policy(&mut self, _policy: Box<()>) {}
        fn discount_policy(&self) -> Option<&()> {
            None
        }
    }

    #[test]
    fn dispatches_portfolios_through_the_standard_evaluator() -> Result<()> {
        let portfolio = Portfolio::new("TEST-PORTFOLIO".into(), vec![(), (), ()])?;
        let pricers = HashMap::from([(
            TypeId::of::<Portfolio<()>>(),
            Box::new(PortfolioPricer::new(ConstantPricer)) as Box<dyn ErasedPricer>,
        )]);
        let evaluator = Evaluator::new(pricers);
        let result = evaluator.evaluate(&portfolio, &[Request::Value], &PricingContext::new())?;
        assert_eq!(result.price(), Some(126.0));
        assert_eq!(result.identifier(), "TEST-PORTFOLIO");
        Ok(())
    }

    #[test]
    fn repeated_evaluations_and_empty_requests_do_not_reuse_accumulated_results() -> Result<()> {
        let portfolio = Portfolio::new("TEST-PORTFOLIO".into(), vec![(), ()])?;
        let pricer = PortfolioPricer::new(ConstantPricer);
        let context = PricingContext::new();
        for _ in 0..2 {
            let result =
                pricer.evaluate(&portfolio, &[Request::Value, Request::Value], &context)?;
            assert_eq!(result.price(), Some(84.0));
        }
        assert!(pricer
            .evaluate(&portfolio, &[], &context)?
            .price()
            .is_none());
        Ok(())
    }
}

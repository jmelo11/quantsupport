//! Deterministic pricing of fixed-rate bond trades.
use super::bondanalytics::{
    bond_market_data_request, contractual_market_request, discounted_value, evaluate_bond,
    impl_bond_pricer, BondPricingEngine, BondState, BondTerms, BondValuation,
};
use crate::instruments::fixedincome::fixedratebond::FixedRateBondTrade;
use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        collateral::DiscountPolicy,
        evaluationresults::EvaluationResults,
        marketdatahandling::marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        pricer::Pricer,
        request::{HandleCashflows, Request},
    },
    indices::marketindex::MarketIndex,
    utils::errors::Result,
};
/// Deterministic bond valuation using the trade's price or Z-spread anchor.
#[derive(Default)]
pub struct BondPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}
impl BondPricer {
    /// Creates a reusable pricer; quote and settlement terms belong to each trade.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
impl BondPricingEngine for BondPricer {
    type Model<T: Scalar> = ();
    fn policy(&self) -> Option<&dyn DiscountPolicy> {
        self.discount_policy.as_deref()
    }
    fn prepare(
        &self,
        trade: &impl BondTerms,
        index: &MarketIndex,
        context: &impl MarketDataProvider,
    ) -> Result<(MarketData, ())> {
        Ok((
            context.handle_request(&contractual_market_request(
                trade,
                index,
                context.evaluation_date(),
            ))?,
            (),
        ))
    }
    fn ad_model(&self, (): &(), _: &MarketData) -> Result<()> {
        Ok(())
    }
    fn value<T: Scalar>(&self, (): &(), valuation: &BondValuation<'_, T>, spread: T) -> Result<T> {
        discounted_value(valuation, spread)
    }
}
impl<T: BondTerms> HandleCashflows<T, BondState> for BondPricer {}
impl_bond_pricer!(BondPricer, FixedRateBondTrade<DualFwd>);

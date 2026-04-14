use crate::{
    ad::dual::DualFwd,
    core::{
        evaluationresults::EvaluationResults,
        pricer::{ErasedPricer, Pricer},
        pricingcontext::PricingContext,
        request::Request,
    },
    instruments::{
        fixedincome::{
            fixedratebond::{FixedRateBond, FixedRateBondTrade},
            fixedratedeposit::{FixedRateDeposit, FixedRateDepositTrade},
        },
        rates::swap::{Swap, SwapTrade},
    },
    pricers::cashflows::discountedcashflowpricer::DiscountedCashflowPricer,
    utils::errors::{QSError, Result},
};

macro_rules! impl_erased_pricer {
    ($pricer_type:ty, $trade_type:ty) => {
        impl ErasedPricer for DiscountedCashflowPricer<$pricer_type, $trade_type> {
            fn evaluate_erased(
                &self,
                trade: &dyn std::any::Any,
                requests: &[Request],
                ctx: &PricingContext,
            ) -> Result<EvaluationResults> {
                if let Some(typed_trade) = trade.downcast_ref::<$trade_type>() {
                    self.evaluate(typed_trade, requests, ctx)
                } else {
                    Err(QSError::InvalidValueErr(format!(
                        "Expected trade of type {}, but got {:?}",
                        stringify!($trade_type),
                        trade.type_id()
                    )))
                }
            }
        }
    };
}

impl_erased_pricer!(FixedRateDeposit<DualFwd>, FixedRateDepositTrade<DualFwd>);
impl_erased_pricer!(FixedRateBond<DualFwd>, FixedRateBondTrade<DualFwd>);
impl_erased_pricer!(Swap<DualFwd>, SwapTrade<DualFwd>);



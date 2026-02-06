use crate::{
    core::{
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        pricer::Pricer,
        pricingcontext::PricingContext,
        pricingrequest::{HandlePrice, HandleSensitivities, PricingRequest},
        trade::Trade,
    },
    instruments::fixedincome::deposit::DepositTrade,
    utils::errors::{AtlasError, Result},
};

/// # `DiscountDepositPricer`
///
/// Implementation of pricer for deposit instruments.
pub struct DiscountDepositPricer;

/// # `DepositPriceEvaluationState`
///
/// Holds state information for deposit price evaluation.
pub struct DepositPriceEvaluationState {}

impl HandlePrice<DepositTrade, DepositPriceEvaluationState> for DiscountDepositPricer {
    fn handle_price(
        &self,
        trade: &DepositTrade,
        ctx: &PricingContext,
        state: &mut DepositPriceEvaluationState,
    ) -> Result<f64> {
        match trade.instrument().is_resolved() {
            false => Err(AtlasError::InstrumentResolutionErr(
                "Deposit instrument is not resolved".into(),
            )),
            true => {
                let final_payment = trade.instrument().final_payment().ok_or(
                    AtlasError::InstrumentResolutionErr("Deposit final payment is not set".into()),
                )?;

                let cashflow = trade.notional() * final_payment / trade.instrument().units();
                let discount = 1.0;
                Ok(cashflow * discount)
            }
        }
    }
}

impl HandleSensitivities<DepositTrade, DepositPriceEvaluationState> for DiscountDepositPricer {
    fn handle_sensitivities(
        &self,
        trade: &DepositTrade,
        ctx: &PricingContext,
        state: &mut DepositPriceEvaluationState,
    ) -> Result<SensitivityMap> {
        todo!("Implement deposit sensitivities handling logic");
    }
}

impl Pricer for DiscountDepositPricer {
    type Item = DepositTrade;

    fn evaluate(
        &self,
        trade: &DepositTrade,
        requests: &[PricingRequest],
        ctx: &PricingContext,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let depo = trade.instrument();
        let identifier = depo.identifier();
        
        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = DepositPriceEvaluationState {};

        for request in requests {
            match request {
                PricingRequest::Price => {
                    let price = self.handle_price(trade, ctx, &mut state)?;
                    results = results.with_price(price);
                }

                PricingRequest::Sensitivities => {
                    let sensitivities = self.handle_sensitivities(trade, ctx, &mut state)?;
                    results = results.with_sensitivities(sensitivities);
                }
                _ => {}
            }
        }

        Ok(results)
    }
}


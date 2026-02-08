use crate::{
    ad::adreal::{ADReal, IsReal},
    core::{
        contextmanager::ContextManager,
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        pricer::Pricer,
        request::{HandleSensitivities, HandleValue, Request},
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
#[derive(Default)]
struct DepositPriceEvaluationState {
    /// Price placeholder for perfomance reasons.
    pub price: Option<ADReal>,
}

impl HandleValue<DepositTrade, DepositPriceEvaluationState> for DiscountDepositPricer {
    fn handle_value(
        &self,
        trade: &DepositTrade,
        ctx: &ContextManager,
        state: &mut DepositPriceEvaluationState,
    ) -> Result<f64> {
        let deposit = trade.instrument();
        let amount = deposit.final_payment().ok_or(AtlasError::ValueNotSetErr(
            "Deposit does not have final payment amount. Is the deposit resolved?.".into(),
        ))?;
        // let assets = ctx.get_assets(deposit.market_index());
        let df = ADReal::one();
        let price = (df * amount).into();
        state.price = Some(price);
        Ok(price.value())
    }
}

impl HandleSensitivities<DepositTrade, DepositPriceEvaluationState> for DiscountDepositPricer {
    fn handle_sensitivities(
        &self,
        trade: &DepositTrade,
        ctx: &ContextManager,
        state: &mut DepositPriceEvaluationState,
    ) -> Result<SensitivityMap> {
        match state.price {
            Some(price) => {
                let _ = price.backward()?;
                let sensitivities = SensitivityMap::new();
                Ok(sensitivities)
            }
            None => Err(AtlasError::ValueNotSetErr("Pricing not requested".into())),
        }
    }
}

impl Pricer for DiscountDepositPricer {
    type Item = DepositTrade;

    fn evaluate(
        &self,
        trade: &DepositTrade,
        requests: &[Request],
        ctx: &ContextManager,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let depo = trade.instrument();
        let identifier = depo.identifier();

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = DepositPriceEvaluationState::default();

        for request in requests {
            match request {
                Request::Value => {
                    let price = self.handle_value(trade, ctx, &mut state)?;
                    results = results.with_price(price);
                }

                Request::Sensitivities => {
                    let sensitivities = self.handle_sensitivities(trade, ctx, &mut state)?;
                    results = results.with_sensitivities(sensitivities);
                }
                _ => {}
            }
        }

        Ok(results)
    }
}

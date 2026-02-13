use crate::{
    core::{
        contextmanager::ContextManager,
        evaluationresults::EvaluationResults,
        instrument::Instrument,
        pricer::Pricer,
        request::{HandleValue, Request},
        trade::Trade,
    },
    instruments::equity::equityeurooption::EquityEuroOptionTrade,
    pricers::pricers::BlackClosedFormPricer,
    utils::errors::Result,
};

#[derive(Default)]
struct EquityOptionState;

impl HandleValue<EquityEuroOptionTrade, EquityOptionState> for BlackClosedFormPricer {
    fn handle_value(
        &self,
        trade: &EquityEuroOptionTrade,
        ctx: &ContextManager,
        state: &mut EquityOptionState,
    ) -> Result<f64> {
        Ok(1.0)
    }
}

impl Pricer for BlackClosedFormPricer {
    type Item = EquityEuroOptionTrade;
    fn evaluate(
        &self,
        trade: &EquityEuroOptionTrade,
        requests: &[Request],
        ctx: &ContextManager,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let option = trade.instrument();
        let identifier = option.identifier();

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = EquityOptionState::default();

        for request in requests {
            match request {
                Request::Value => {
                    let price = self.handle_value(trade, ctx, &mut state)?;
                    results = results.with_price(price);
                }
                _ => {}
            }
        }

        Ok(results)
    }
}

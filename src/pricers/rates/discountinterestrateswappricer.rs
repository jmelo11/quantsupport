use crate::{
    ad::adreal::{ADReal, IsReal},
    core::{
        assets::AssetType,
        contextmanager::ContextManager,
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        pricer::Pricer,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::rates::swap::{InterestRateSwapTrade, SwapDirection},
    rates::interest_rate_curve::InterestRateCurveAsset,
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    utils::errors::{AtlasError, Result},
};

/// # `DiscountInterestRateSwapPricer`
///
/// Discounting pricer for interest rate swaps.
pub struct DiscountInterestRateSwapPricer;

/// Swap pricing state.
#[derive(Default)]
struct SwapPriceEvaluationState {
    price: Option<ADReal>,
}

impl HandleValue<InterestRateSwapTrade, SwapPriceEvaluationState>
    for DiscountInterestRateSwapPricer
{
    fn handle_value(
        &self,
        trade: &InterestRateSwapTrade,
        ctx: &ContextManager,
        state: &mut SwapPriceEvaluationState,
    ) -> Result<f64> {
        Ok(1.0)
    }
}

impl HandleSensitivities<InterestRateSwapTrade, SwapPriceEvaluationState>
    for DiscountInterestRateSwapPricer
{
    fn handle_sensitivities(
        &self,
        _trade: &InterestRateSwapTrade,
        _ctx: &ContextManager,
        state: &mut SwapPriceEvaluationState,
    ) -> Result<SensitivityMap> {
        match state.price {
            Some(price) => {
                let _ = price.backward()?;
                Ok(SensitivityMap::new())
            }
            None => Err(AtlasError::ValueNotSetErr("Pricing not requested".into())),
        }
    }
}

impl Pricer for DiscountInterestRateSwapPricer {
    type Item = InterestRateSwapTrade;

    fn evaluate(
        &self,
        trade: &InterestRateSwapTrade,
        requests: &[Request],
        ctx: &ContextManager,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let swap = trade.instrument();
        let identifier = swap.identifier();

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = SwapPriceEvaluationState::default();

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

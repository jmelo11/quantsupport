// use crate::{
//     ad::adreal::ADReal,
//     core::{
//         evaluationresults::{EvaluationResults, SensitivityMap},
//         instrument::Instrument,
//         pricer::Pricer,
//         request::{HandleSensitivities, HandleValue, Request},
//         trade::Trade,
//     },
//     instruments::rates::swap::InterestRateSwapTrade,
//     utils::errors::{AtlasError, Result},
// };

// /// # `DiscountInterestRateSwapPricer`
// ///
// /// Discounting pricer for interest rate swaps.
// pub struct DiscountInterestRateSwapPricer;

// /// Swap pricing state.
// #[derive(Default)]
// struct SwapPriceEvaluationState {
//     price: Option<ADReal>,
// }

// impl HandleValue<InterestRateSwapTrade, SwapPriceEvaluationState>
//     for DiscountInterestRateSwapPricer
// {
//     fn handle_value(
//         &self,
//         _: &InterestRateSwapTrade,
//         _: &mut SwapPriceEvaluationState,
//     ) -> Result<f64> {
//         Ok(1.0)
//     }
// }

// impl HandleSensitivities<InterestRateSwapTrade, SwapPriceEvaluationState>
//     for DiscountInterestRateSwapPricer
// {
//     fn handle_sensitivities(
//         &self,
//         _: &InterestRateSwapTrade,
//         state: &mut SwapPriceEvaluationState,
//     ) -> Result<SensitivityMap> {
//         match state.price {
//             Some(price) => {
//                 let () = price.backward()?;
//                 Ok(SensitivityMap::default())
//             }
//             None => Err(AtlasError::ValueNotSetErr("Pricing not requested".into())),
//         }
//     }
// }

// impl Pricer for DiscountInterestRateSwapPricer {
//     type Item = InterestRateSwapTrade;

//     fn evaluate(
//         &self,
//         trade: &InterestRateSwapTrade,
//         requests: &[Request],
//         ctx: &impl MarketDataProvider,
//     ) -> Result<EvaluationResults> {
//         let eval_date = ctx.evaluation_date();
//         let swap = trade.instrument();
//         let identifier = swap.identifier();

//         let mut results = EvaluationResults::new(eval_date, identifier);
//         let mut state = SwapPriceEvaluationState::default();

//         for request in requests {
//             match request {
//                 Request::Value => {
//                     let price = self.handle_value(trade, &mut state)?;
//                     results = results.with_price(price);
//                 }
//                 Request::Sensitivities => {
//                     let sensitivities = self.handle_sensitivities(trade, &mut state)?;
//                     results = results.with_sensitivities(sensitivities);
//                 }
//                 _ => {}
//             }
//         }

//         Ok(results)
//     }

//     fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest> {
//         let discount_curve = DerivedElementRequest::DiscountCurve {
//             market_index: trade.instrument().market_index(),
//         };
//         Some(MarketDataRequest::default().with_element_requests(vec![discount_curve]))
//     }
// }

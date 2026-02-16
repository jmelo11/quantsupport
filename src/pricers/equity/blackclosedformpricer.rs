// use crate::{
//     ad::{
//         adreal::{ADReal, FloatExt, IsReal},
//         tape::Tape,
//     },
//     core::{
//         evaluationresults::{EvaluationResults, SensitivityMap},
//         instrument::Instrument,
//         marketdataprovider::{
//             DerivedElementRequest, FixingRequest, MarketDataProvider, MarketDataRequest,
//             MarketDataResponse, VolNodeKey,
//         },
//         pricer::Pricer,
//         request::{HandleSensitivities, HandleValue, Request},
//         trade::Trade,
//     },
//     instruments::equity::equityeurooption::{EquityEuroOptionTrade, EuroOptionType},
//     math::probability::norm_cdf::norm_cdf,
//     pricers::pricers::BlackClosedFormPricer,
//     time::daycounter::DayCounter,
//     utils::errors::{AtlasError, Result},
// };

// #[derive(Default)]
// struct EquityOptionState {
//     value: Option<ADReal>,
//     md_response: Option<MarketDataResponse>,
// }

// impl HandleValue<EquityEuroOptionTrade, EquityOptionState> for BlackClosedFormPricer {
//     fn handle_value(
//         &self,
//         trade: &EquityEuroOptionTrade,
//         state: &mut EquityOptionState,
//     ) -> Result<f64> {
//         let option = trade.instrument();
//         let index = option.market_index().clone();
//         let tau = DayCounter::Actual365
//             .year_fraction(trade.trade_date(), option.expiry_date())
//             .max(0.0);

//         let md_response = state
//             .md_response
//             .as_mut()
//             .ok_or(AtlasError::ValueNotSetErr(
//                 "Market data response not loaded".into(),
//             ))?;

//         let fixing_key = (index.clone(), trade.trade_date());
//         let spot_entry =
//             md_response
//                 .fixings
//                 .get_mut(&fixing_key)
//                 .ok_or(AtlasError::NotFoundErr(
//                     "Missing spot fixing for option index/trade date".into(),
//                 ))?;

//         let vol_key = VolNodeKey::new(index.clone(), option.expiry_date(), option.strike());
//         let vol_entry = md_response
//             .vol_nodes
//             .get_mut(&vol_key)
//             .ok_or(AtlasError::NotFoundErr(
//                 "Missing volatility node for option expiry/strike".into(),
//             ))?;

//         let discount_element = md_response
//             .discount_curves
//             .get_mut(&index)
//             .ok_or(AtlasError::NotFoundErr("Missing discount curve".into()))?;
//         let dividend_element = md_response
//             .dividend_curves
//             .get_mut(&index)
//             .ok_or(AtlasError::NotFoundErr("Missing dividend curve".into()))?;

//         Tape::start_recording();
//         for (_, pillar) in &mut discount_element.curve.pillars().unwrap() {
//             pillar.put_on_tape();
//         }
//         for (_, pillar) in &mut dividend_element.curve.pillars().unwrap() {
//             pillar.put_on_tape();
//         }
//         spot_entry.put_on_tape();
//         vol_entry.put_on_tape();
//         let spot = *spot_entry;
//         let vol = *vol_entry;

//         let strike: ADReal = option.strike().into();
//         let sqrt_tau = tau.sqrt();
//         let vol_sqrt_tau: ADReal = (vol * sqrt_tau).into();

//         let df_r = discount_element
//             .curve
//             .discount_factor(option.expiry_date())?;
//         let df_q = dividend_element
//             .curve
//             .discount_factor(option.expiry_date())?;

//         let fwd: ADReal = (spot * df_q / df_r).into();
//         let d1: ADReal = (((fwd / strike).ln() + vol * vol * 0.5 * tau) / vol_sqrt_tau).into();
//         let d2: ADReal = (d1 - vol_sqrt_tau).into();

//         let nd1 = norm_cdf(d1.value());
//         let nd2 = norm_cdf(d2.value());
//         let nmd1 = norm_cdf(-d1.value());
//         let nmd2 = norm_cdf(-d2.value());

//         let undiscounted: ADReal = match option.option_type() {
//             EuroOptionType::Call => (fwd * nd1 - strike * nd2).into(),
//             EuroOptionType::Put => (strike * nmd2 - fwd * nmd1).into(),
//         };

//         let value: ADReal = (df_r * undiscounted * trade.notional()).into();
//         state.value = Some(value);
//         Tape::stop_recording();

//         Ok(value.value())
//     }
// }

// impl HandleSensitivities<EquityEuroOptionTrade, EquityOptionState> for BlackClosedFormPricer {
//     fn handle_sensitivities(
//         &self,
//         trade: &EquityEuroOptionTrade,
//         state: &mut EquityOptionState,
//     ) -> Result<SensitivityMap> {
//         let value = if let Some(value) = state.value {
//             value
//         } else {
//             let _ = self.handle_value(trade, state)?;
//             state
//                 .value
//                 .ok_or(AtlasError::ValueNotSetErr("Pricing not requested".into()))?
//         };

//         let md_response = state
//             .md_response
//             .as_ref()
//             .ok_or(AtlasError::ValueNotSetErr(
//                 "Market data response not loaded".into(),
//             ))?;

//         value.backward()?;
//         let option = trade.instrument();
//         let index = option.market_index();

//         let mut ids = Vec::new();
//         let mut exposures = Vec::new();

//         if let Some(spot) = md_response
//             .fixings
//             .get(&(index.clone(), trade.trade_date()))
//         {
//             ids.push("spot".to_string());
//             exposures.push(spot.adjoint()?);
//         }

//         let vol_key = VolNodeKey::new(index.clone(), option.expiry_date(), option.strike());
//         if let Some(vol) = md_response.vol_nodes.get(&vol_key) {
//             ids.push("volatility".to_string());
//             exposures.push(vol.adjoint()?);
//         }

//         if let Some(discount_curve) = md_response.discount_curves.get(index) {
//             for (label, pillar) in &discount_curve.curve.pillars().unwrap() {
//                 ids.push(format!("discount::{label}"));
//                 exposures.push(pillar.adjoint()?);
//             }
//         }

//         if let Some(dividend_curve) = md_response.dividend_curves.get(index) {
//             for (label, pillar) in &dividend_curve.curve.pillars().unwrap() {
//                 ids.push(format!("dividend::{label}"));
//                 exposures.push(pillar.adjoint()?);
//             }
//         }

//         Ok(SensitivityMap::default()
//             .with_instrument_keys(ids)
//             .with_exposure(exposures))
//     }
// }

// impl Pricer for BlackClosedFormPricer {
//     type Item = EquityEuroOptionTrade;
//     fn evaluate(
//         &self,
//         trade: &EquityEuroOptionTrade,
//         requests: &[Request],
//         ctx: &impl MarketDataProvider,
//     ) -> Result<EvaluationResults> {
//         let eval_date = ctx.evaluation_date();
//         let option = trade.instrument();
//         let identifier = option.identifier();

//         let md_request = self
//             .market_data_request(trade)
//             .ok_or(AtlasError::InvalidValueErr(
//                 "Missing market data request".into(),
//             ))?;

//         let mut results = EvaluationResults::new(eval_date, identifier);
//         let mut state = EquityOptionState {
//             value: None,
//             md_response: Some(ctx.handle_request(&md_request)?),
//         };

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
//         let option = trade.instrument();
//         let index = option.market_index().clone();
//         Some(
//             MarketDataRequest::default()
//                 .with_element_requests(vec![
//                     DerivedElementRequest::DiscountCurve {
//                         market_index: index.clone(),
//                     },
//                     DerivedElementRequest::DividendCurve {
//                         market_index: index.clone(),
//                     },
//                     DerivedElementRequest::VolatilitySurface {
//                         market_index: index.clone(),
//                     },
//                     DerivedElementRequest::VolNode {
//                         market_index: index.clone(),
//                         date: option.expiry_date(),
//                         axis: option.strike(),
//                     },
//                 ])
//                 .with_fixing_requests(vec![FixingRequest::new(index, trade.trade_date())]),
//         )
//     }
// }

// #[cfg(test)]
// mod tests {
//     use std::sync::Arc;

//     use crate::{
//         ad::adreal::ADReal,
//         core::{
//             inmemorymarketdataprovider::InMemoryMarketDataProvider,
//             marketdataprovider::{DiscountCurveElement, DividendCurveElement},
//             pricer::Pricer,
//             request::Request,
//         },
//         currencies::currency::Currency,
//         indices::marketindex::MarketIndex,
//         instruments::equity::equityeurooption::{
//             EquityEuroOption, EquityEuroOptionTrade, EuroOptionType,
//         },
//         pricers::pricers::BlackClosedFormPricer,
//         rates::{
//             interestrate::RateDefinition,
//             yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
//         },
//         time::date::Date,
//     };

//     #[test]
//     fn option_black_value_and_sensitivities() {
//         let eval = Date::new(2025, 1, 1);
//         let expiry = Date::new(2025, 7, 1);
//         let index = MarketIndex::Equity("SPX".to_string());

//         let option = EquityEuroOption::new(
//             index.clone(),
//             expiry,
//             100.0,
//             EuroOptionType::Call,
//             "OPT1".to_string(),
//         );
//         let trade = EquityEuroOptionTrade::new(option, 1.0, eval);

//         let disc = Arc::new(FlatForwardTermStructure::<ADReal>::new(
//             eval,
//             ADReal::from(0.03),
//             RateDefinition::default(),
//         ));
//         let div = Arc::new(FlatForwardTermStructure::<ADReal>::new(
//             eval,
//             ADReal::from(0.01),
//             RateDefinition::default(),
//         ));

//         let md = InMemoryMarketDataProvider::new(eval)
//             .with_discount_curve(DiscountCurveElement {
//                 market_index: index.clone(),
//                 currency: Currency::USD,
//                 curve: disc,
//             })
//             .with_dividend_curve(DividendCurveElement {
//                 market_index: index.clone(),
//                 currency: Currency::USD,
//                 curve: div,
//             })
//             .with_fixing(index.clone(), eval, ADReal::from(102.0))
//             .with_vol_node(index.clone(), expiry, 100.0, ADReal::from(0.2));

//         let pricer = BlackClosedFormPricer;
//         let results = pricer
//             .evaluate(&trade, &[Request::Value, Request::Sensitivities], &md)
//             .expect("option pricing works");

//         assert!(results.price().is_some());
//         let sens = results.sensitivities().expect("sensitivities present");
//         assert!(!sens.instrument_keys().is_empty());
//         assert_eq!(sens.instrument_keys().len(), sens.exposure().len());
//     }
// }

use crate::{
    ad::{
        adreal::{ADReal, IsReal},
        tape::Tape,
    },
    core::{
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        marketdatarequest::{
            derivedelementrequest::{
                DerivedElementRequest, MarketDataProvider, MarketDataRequest, MarketDataResponse,
            },
            fixingrequest::FixingRequest,
            volatilityelements::{VolatilityAxis, VolatilityNodeKey},
        },
        pricer::Pricer,
        request::{HandleSensitivities, HandleValue, PricerState, Request},
        trade::Trade,
    },
    instruments::equity::equityeurooption::{EquityEuroOptionTrade, EuroOptionType},
    pricers::generalpricers::BlackClosedFormPricer,
    time::daycounter::DayCounter,
    utils::errors::{AtlasError, Result},
};

#[derive(Default)]
struct EquityOptionState {
    value: Option<ADReal>,
    spot: Option<ADReal>,
    md_response: Option<MarketDataResponse>,
}

impl PricerState for EquityOptionState {
    fn get_market_data_reponse(&self) -> Option<&MarketDataResponse> {
        self.md_response.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketDataResponse> {
        self.md_response.as_mut()
    }
}

impl HandleValue<EquityEuroOptionTrade, EquityOptionState> for BlackClosedFormPricer {
    fn handle_value(
        &self,
        trade: &EquityEuroOptionTrade,
        state: &mut EquityOptionState,
    ) -> Result<f64> {
        let option = trade.instrument();
        let index = option.market_index().clone();
        let tau = DayCounter::Actual365
            .year_fraction(trade.trade_date(), option.expiry_date())
            .max(0.0);

        let spot = state.get_fixing(&index, trade.trade_date())?;

        let md_response = state
            .md_response
            .as_mut()
            .ok_or(AtlasError::ValueNotSetErr(
                "Market data response not loaded".into(),
            ))?;

        let volatility_key = VolatilityNodeKey::new(
            index.clone(),
            option.expiry_date(),
            VolatilityAxis::strike(option.strike()),
        );
        if !md_response.volatility_nodes().contains_key(&volatility_key) {
            return Err(AtlasError::NotFoundErr(
                "Missing volatility node for option expiry/strike".into(),
            ));
        }

        Tape::start_recording();
        md_response
            .discount_curves_mut()
            .get_mut(&index)
            .ok_or(AtlasError::NotFoundErr("Missing discount curve".into()))?
            .curve_mut()
            .put_pillars_on_tape();
        md_response
            .dividend_curves_mut()
            .get_mut(&index)
            .ok_or(AtlasError::NotFoundErr("Missing dividend curve".into()))?
            .curve_mut()
            .put_pillars_on_tape();
        let mut spot_ad = ADReal::new(spot);
        spot_ad.put_on_tape();
        state.spot = Some(spot_ad);
        let volatility = {
            let volatility_node = md_response
                .volatility_nodes_mut()
                .get_mut(&volatility_key)
                .ok_or(AtlasError::NotFoundErr(
                    "Missing volatility node for option expiry/strike".into(),
                ))?;
            volatility_node.value_mut().put_on_tape();
            volatility_node.value()
        };

        let strike: ADReal = option.strike().into();
        let df_r = md_response
            .discount_curves()
            .get(&index)
            .ok_or(AtlasError::NotFoundErr("Missing discount curve".into()))?
            .curve()
            .discount_factor(option.expiry_date())?;
        let df_q = md_response
            .dividend_curves()
            .get(&index)
            .ok_or(AtlasError::NotFoundErr("Missing dividend curve".into()))?
            .curve()
            .discount_factor(option.expiry_date())?;
        let fwd: ADReal = (spot_ad * df_q / df_r).into();

        let undiscounted = BlackClosedFormPricer::black_forward_price(
            fwd,
            strike,
            volatility,
            tau,
            matches!(option.option_type(), EuroOptionType::Call),
        );

        let value: ADReal = (df_r * undiscounted * trade.notional()).into();
        state.value = Some(value);
        Tape::stop_recording();

        Ok(value.value())
    }
}

impl HandleSensitivities<EquityEuroOptionTrade, EquityOptionState> for BlackClosedFormPricer {
    fn handle_sensitivities(
        &self,
        trade: &EquityEuroOptionTrade,
        state: &mut EquityOptionState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(value) = state.value {
            value
        } else {
            let _ = self.handle_value(trade, state)?;
            state
                .value
                .ok_or(AtlasError::ValueNotSetErr("Pricing not requested".into()))?
        };

        let md_response = state
            .md_response
            .as_ref()
            .ok_or(AtlasError::ValueNotSetErr(
                "Market data response not loaded".into(),
            ))?;

        value.backward()?;
        let option = trade.instrument();
        let index = option.market_index();

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        if md_response
            .fixings()
            .contains_key(&(index.clone(), trade.trade_date()))
        {
            ids.push("spot".to_string());
            exposures.push(
                state
                    .spot
                    .ok_or(AtlasError::ValueNotSetErr(
                        "Spot not recorded on state".into(),
                    ))?
                    .adjoint()?,
            );
        }

        let volatility_key = VolatilityNodeKey::new(
            index.clone(),
            option.expiry_date(),
            VolatilityAxis::strike(option.strike()),
        );
        if let Some(volatility) = md_response.volatility_nodes().get(&volatility_key) {
            ids.push("volatility".to_string());
            exposures.push(volatility.value().adjoint()?);
        }

        if let Some(discount_curve) = md_response.discount_curves().get(index) {
            for (label, pillar) in discount_curve.curve().pillars().unwrap_or_default() {
                ids.push(format!("discount::{label}"));
                exposures.push(pillar.adjoint()?);
            }
        }

        if let Some(dividend_curve) = md_response.dividend_curves().get(index) {
            for (label, pillar) in dividend_curve.curve().pillars().unwrap_or_default() {
                ids.push(format!("dividend::{label}"));
                exposures.push(pillar.adjoint()?);
            }
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures))
    }
}

impl Pricer for BlackClosedFormPricer {
    type Item = EquityEuroOptionTrade;
    fn evaluate(
        &self,
        trade: &EquityEuroOptionTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let option = trade.instrument();
        let identifier = option.identifier();

        let md_request = self
            .market_data_request(trade)
            .ok_or(AtlasError::InvalidValueErr(
                "Missing market data request".into(),
            ))?;

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = EquityOptionState {
            value: None,
            spot: None,
            md_response: Some(ctx.handle_request(&md_request)?),
        };

        for request in requests {
            match request {
                Request::Value => {
                    let price = self.handle_value(trade, &mut state)?;
                    results = results.with_price(price);
                }
                Request::Sensitivities => {
                    let sensitivities = self.handle_sensitivities(trade, &mut state)?;
                    results = results.with_sensitivities(sensitivities);
                }
                _ => {}
            }
        }

        Ok(results)
    }

    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest> {
        let option = trade.instrument();
        let index = option.market_index().clone();
        Some(
            MarketDataRequest::default()
                .with_element_requests(vec![
                    DerivedElementRequest::DiscountCurve {
                        market_index: index.clone(),
                    },
                    DerivedElementRequest::DividendCurve {
                        market_index: index.clone(),
                    },
                    DerivedElementRequest::VolatilitySurface {
                        market_index: index.clone(),
                    },
                    DerivedElementRequest::VolNode {
                        market_index: index.clone(),
                        date: option.expiry_date(),
                        axis: VolatilityAxis::strike(option.strike()),
                    },
                ])
                .with_fixing_requests(vec![FixingRequest::new(index, trade.trade_date())]),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        ad::adreal::{ADReal, IsReal},
        core::{
            marketdatarequest::{
                curveelement::{DiscountCurveElement, DividendCurveElement},
                volatilityelements::{VolatilityAxis, VolatilityNodeKey, VolatilitySurfaceElement},
            },
            pricer::Pricer,
            request::Request,
            marketdataelementowner::MarketDataElementOwner,
        },
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        instruments::equity::equityeurooption::{
            EquityEuroOption, EquityEuroOptionTrade, EuroOptionType,
        },
        pricers::generalpricers::BlackClosedFormPricer,
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::date::Date,
    };

    #[test]
    fn option_black_value_and_sensitivities() {
        let eval = Date::new(2025, 1, 1);
        let expiry = Date::new(2025, 7, 1);
        let index = MarketIndex::Equity("SPX".to_string());

        let option = EquityEuroOption::new(
            index.clone(),
            expiry,
            100.0,
            EuroOptionType::Call,
            "OPT1".to_string(),
        );
        let trade = EquityEuroOptionTrade::new(option, 1.0, eval);

        let disc = Box::new(FlatForwardTermStructure::<ADReal>::new(
            eval,
            ADReal::from(0.03),
            RateDefinition::default(),
        ));
        let div = Box::new(FlatForwardTermStructure::<ADReal>::new(
            eval,
            ADReal::from(0.01),
            RateDefinition::default(),
        ));

        let mut nodes = HashMap::new();
        nodes.insert(
            VolatilityNodeKey::new(index.clone(), expiry, VolatilityAxis::strike(100.0)),
            ADReal::from(0.2),
        );

        let md = MarketDataElementOwner::new(eval)
            .with_discount_curve(DiscountCurveElement::new(
                index.clone(),
                Currency::USD,
                disc,
            ))
            .with_dividend_curve(DividendCurveElement::new(index.clone(), Currency::USD, div))
            .with_fixing(index.clone(), eval, 102.0)
            .with_vol_surface(VolatilitySurfaceElement::new(index.clone(), nodes));

        let pricer = BlackClosedFormPricer;
        let results = pricer
            .evaluate(&trade, &[Request::Value, Request::Sensitivities], &md)
            .expect("option pricing works");

        assert!(results.price().is_some());
        let sens = results.sensitivities().expect("sensitivities present");
        println!("Sensitivities: {:#?}", sens);
        assert!(!sens.instrument_keys().is_empty());
        assert_eq!(sens.instrument_keys().len(), sens.exposure().len());
        let spot_pos = sens
            .instrument_keys()
            .iter()
            .position(|key| key == "spot")
            .expect("spot sensitivity present");
        assert!(sens.exposure()[spot_pos].abs() > 1e-12);
    }

    #[test]
    fn vol_surface_returns_interpolated_node_with_source_keys() {
        let index = MarketIndex::Equity("SPX".to_string());
        let d0 = Date::new(2025, 6, 1);
        let d1 = Date::new(2025, 8, 1);
        let mut nodes = HashMap::new();
        nodes.insert(VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::strike(90.0)), ADReal::from(0.24));
        nodes.insert(VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::strike(110.0)), ADReal::from(0.20));
        nodes.insert(VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::strike(90.0)), ADReal::from(0.22));
        nodes.insert(VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::strike(110.0)), ADReal::from(0.18));

        let surface = VolatilitySurfaceElement::new(index.clone(), nodes);
        let node = surface.node(Date::new(2025, 7, 1), VolatilityAxis::strike(100.0)).expect("interpolated node");

        assert!(node.value().value() > 0.19 && node.value().value() < 0.23);
        assert_eq!(node.interpolation_keys().len(), 4);
    }

    #[test]
    fn vol_surface_supports_delta_and_moneyness_axes() {
        let index = MarketIndex::Equity("SPX".to_string());
        let d0 = Date::new(2025, 6, 1);
        let d1 = Date::new(2025, 8, 1);

        let mut delta_nodes = HashMap::new();
        delta_nodes.insert(VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::delta(0.25)), ADReal::from(0.26));
        delta_nodes.insert(VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::delta(0.50)), ADReal::from(0.22));
        delta_nodes.insert(VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::delta(0.25)), ADReal::from(0.24));
        delta_nodes.insert(VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::delta(0.50)), ADReal::from(0.20));
        let delta_surface = VolatilitySurfaceElement::new(index.clone(), delta_nodes);
        let delta_node = delta_surface.node(Date::new(2025, 7, 1), VolatilityAxis::delta(0.40)).expect("delta interpolation");
        assert!(delta_node.value().value() > 0.21 && delta_node.value().value() < 0.24);

        let mut moneyness_nodes = HashMap::new();
        moneyness_nodes.insert(VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::log_moneyness(-0.1)), ADReal::from(0.24));
        moneyness_nodes.insert(VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::log_moneyness(0.1)), ADReal::from(0.20));
        moneyness_nodes.insert(VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::log_moneyness(-0.1)), ADReal::from(0.22));
        moneyness_nodes.insert(VolatilityNodeKey::new(index, d1, VolatilityAxis::log_moneyness(0.1)), ADReal::from(0.18));
        let moneyness_surface = VolatilitySurfaceElement::new(MarketIndex::Equity("SPX".to_string()), moneyness_nodes);
        let moneyness_node = moneyness_surface.node(Date::new(2025, 7, 1), VolatilityAxis::log_moneyness(0.0)).expect("moneyness interpolation");
        assert!(moneyness_node.value().value() > 0.19 && moneyness_node.value().value() < 0.23);
    }
}

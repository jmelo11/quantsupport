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

fn standard_normal_pdf(x: f64) -> f64 {
    (-(x * x) * 0.5).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

/// # `EquityOptionState`
///
/// State struct for storing intermediate values during the pricing of an equity option, including the option value, spot price, and market data response.
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

        // move to the instrument level
        let tau = DayCounter::Actual365
            .year_fraction(trade.trade_date(), option.expiry_date())
            .max(0.0);

        let spot = state.get_fixing(&index, trade.trade_date())?;

        let volatility_key = VolatilityNodeKey::new(
            index.clone(),
            option.expiry_date(),
            VolatilityAxis::strike(option.strike()),
        );

        // this should interpolate...
        if !state
            .get_market_data_reponse()
            .ok_or(AtlasError::ValueNotSetErr(
                "Market data response not loaded".into(),
            ))?
            .volatility_nodes()
            .contains_key(&volatility_key)
        {
            return Err(AtlasError::NotFoundErr(
                "Missing volatility node for option expiry/strike".into(),
            ));
        }

        Tape::start_recording();
        state
            .get_discount_curve_element_mut(&index)?
            .borrow_mut()
            .curve_mut()
            .put_pillars_on_tape();
        state
            .get_dividend_curve_element_mut(&index)?
            .borrow_mut()
            .curve_mut()
            .put_pillars_on_tape();
        let mut spot_ad = ADReal::new(spot);
        spot_ad.put_on_tape();
        state.spot = Some(spot_ad);

        // this should be simplified and moved to the PriceState impl - we shouldn't have to do this manually in the pricer logic
        let volatility_keys = state
            .get_market_data_reponse()
            .ok_or(AtlasError::ValueNotSetErr(
                "Market data response not loaded".into(),
            ))?
            .volatility_nodes()
            .get(&volatility_key)
            .ok_or(AtlasError::NotFoundErr(
                "Missing volatility node for option expiry/strike".into(),
            ))?
            .interpolation_keys()
            .to_vec();

        let volatility_surface = state
            .get_market_data_reponse()
            .ok_or(AtlasError::ValueNotSetErr(
                "Market data response not loaded".into(),
            ))?
            .volatility_surface(&index)
            .ok_or(AtlasError::NotFoundErr("Missing volatility surface".into()))?
            .clone();

        {
            let mut surface = volatility_surface.borrow_mut();
            for key in &volatility_keys {
                if let Some(node) = surface.nodes_mut().get_mut(key) {
                    node.put_on_tape();
                }
            }
        }

        let repriced_volatility_node = volatility_surface
            .borrow()
            .node(
                option.expiry_date(),
                VolatilityAxis::strike(option.strike()),
            )
            .ok_or(AtlasError::NotFoundErr(
                "Missing volatility node for option expiry/strike".into(),
            ))?;

        state
            .get_market_data_reponse_mut()
            .ok_or(AtlasError::ValueNotSetErr(
                "Market data response not loaded".into(),
            ))?
            .volatility_nodes_mut()
            .insert(volatility_key.clone(), repriced_volatility_node.clone());

        let volatility = state
            .get_market_data_reponse()
            .ok_or(AtlasError::ValueNotSetErr(
                "Market data response not loaded".into(),
            ))?
            .volatility_nodes()
            .get(&volatility_key)
            .ok_or(AtlasError::NotFoundErr(
                "Missing volatility node for option expiry/strike".into(),
            ))?
            .value();

        // this should be just f64
        let strike: ADReal = option.strike().into();

        // borrowing should be handled by the PriceState impl - we shouldn't have to do this manually in the pricer logic
        let df_r = state
            .get_discount_curve_element(&index)?
            .borrow()
            .curve()
            .discount_factor(option.expiry_date())?;
        let df_q = state
            .get_dividend_curve_element(&index)?
            .borrow()
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

        // the mark is not being set on the value during pricing
        value.backward_to_mark()?;
        let option = trade.instrument();
        let index = option.market_index();

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        // this should be the index name or identifier, not "spot"
        ids.push("spot".to_string());
        exposures.push(
            state
                .spot
                .ok_or(AtlasError::ValueNotSetErr(
                    "Spot not recorded on state".into(),
                ))?
                .adjoint()?,
        );

        // this was already done while pricing, we should just be able to read off from the state
        let volatility_key = VolatilityNodeKey::new(
            index.clone(),
            option.expiry_date(),
            VolatilityAxis::strike(option.strike()),
        );
        let volatility = state
            .get_market_data_reponse()
            .ok_or(AtlasError::ValueNotSetErr(
                "Market data response not loaded".into(),
            ))?
            .volatility_nodes()
            .get(&volatility_key)
            .ok_or(AtlasError::NotFoundErr(
                "Missing volatility node for option expiry/strike".into(),
            ))?;
        
        // all this is wrong, we are using AD, we must compute sensitivities using the adjoints, not anything else.
        let tau = DayCounter::Actual365
            .year_fraction(trade.trade_date(), option.expiry_date())
            .max(0.0);
        let spot = state.get_fixing(index, trade.trade_date())?;
        let df_r = state
            .get_discount_curve_element(index)?
            .borrow()
            .curve()
            .discount_factor(option.expiry_date())?
            .value();
        let df_q = state
            .get_dividend_curve_element(index)?
            .borrow()
            .curve()
            .discount_factor(option.expiry_date())?
            .value();
        let fwd = spot * df_q / df_r;
        let vol = volatility.value().value();
        let sqrt_tau = tau.sqrt();
        let vega = if tau <= 0.0 || vol <= 0.0 {
            0.0
        } else {
            let d1 = ((fwd / option.strike()).ln() + 0.5 * vol * vol * tau) / (vol * sqrt_tau);
            df_r * trade.notional() * fwd * standard_normal_pdf(d1) * sqrt_tau
        };

        let mut sensitivity_keys = volatility.interpolation_keys().to_vec();
        if sensitivity_keys.is_empty() {
            sensitivity_keys.push(volatility_key.clone());
        }

        let mut sensitivity_labels = volatility.interpolation_labels().to_vec();
        if sensitivity_labels.is_empty() {
            return Err(AtlasError::NotFoundErr(
                "Missing quote identifiers for volatility node sensitivities".into(),
            ));
        }

        if sensitivity_labels.len() != sensitivity_keys.len() {
            return Err(AtlasError::InvalidValueErr(
                "Volatility interpolation keys and labels length mismatch".into(),
            ));
        }

        let eps = 1e-5;
        let surface = state
            .get_market_data_reponse_mut()
            .ok_or(AtlasError::ValueNotSetErr(
                "Market data response not loaded".into(),
            ))?
            .volatility_surface(index)
            .ok_or(AtlasError::NotFoundErr("Missing volatility surface".into()))?
            .clone();

        for (key, label) in sensitivity_keys
            .into_iter()
            .zip(sensitivity_labels.drain(..))
        {
            let (up, down) = {
                let mut vol_surface = surface.borrow_mut();
                let base = vol_surface
                    .nodes()
                    .get(&key)
                    .ok_or(AtlasError::NotFoundErr(
                        "Missing underlying volatility surface node".into(),
                    ))?
                    .value();

                vol_surface
                    .nodes_mut()
                    .insert(key.clone(), ADReal::from(base + eps));
                let up = vol_surface
                    .node(
                        option.expiry_date(),
                        VolatilityAxis::strike(option.strike()),
                    )
                    .ok_or(AtlasError::NotFoundErr(
                        "Missing volatility node for option expiry/strike".into(),
                    ))?
                    .value()
                    .value();

                vol_surface
                    .nodes_mut()
                    .insert(key.clone(), ADReal::from(base - eps));
                let down = vol_surface
                    .node(
                        option.expiry_date(),
                        VolatilityAxis::strike(option.strike()),
                    )
                    .ok_or(AtlasError::NotFoundErr(
                        "Missing volatility node for option expiry/strike".into(),
                    ))?
                    .value()
                    .value();

                vol_surface
                    .nodes_mut()
                    .insert(key.clone(), ADReal::from(base));
                (up, down)
            };

            let dvol_dnode = (up - down) / (2.0 * eps);
            ids.push(label);
            exposures.push(vega * dvol_dnode);
        }

        for (label, pillar) in state
            .get_discount_curve_element(index)?
            .borrow()
            .curve()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(format!("discount::{label}"));
            exposures.push(pillar.adjoint()?);
        }

        for (label, pillar) in state
            .get_dividend_curve_element(index)?
            .borrow()
            .curve()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(format!("dividend::{label}"));
            exposures.push(pillar.adjoint()?);
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
        ad::adreal::ADReal,
        core::{
            marketdataelementowner::MarketDataElementOwner,
            marketdatarequest::{
                curveelement::{DiscountCurveElement, DividendCurveElement},
                volatilityelements::{VolatilityAxis, VolatilityNodeKey, VolatilitySurfaceElement},
            },
            pricer::Pricer,
            request::Request,
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
        let mut labels = HashMap::new();
        let vol_key = VolatilityNodeKey::new(index.clone(), expiry, VolatilityAxis::strike(100.0));
        nodes.insert(vol_key.clone(), ADReal::from(0.2));
        labels.insert(vol_key, "VOL_SPX_20250701_K100".to_string());

        let md = MarketDataElementOwner::new(eval)
            .with_discount_curve(DiscountCurveElement::new(
                index.clone(),
                Currency::USD,
                disc,
            ))
            .with_dividend_curve(DividendCurveElement::new(index.clone(), Currency::USD, div))
            .with_fixing(index.clone(), eval, 102.0)
            .with_vol_surface(VolatilitySurfaceElement::with_labels(
                index.clone(),
                nodes,
                labels,
            ));

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

        let vol_pos = sens
            .instrument_keys()
            .iter()
            .position(|key| key == "VOL_SPX_20250701_K100")
            .expect("vol sensitivity present");
        assert!(sens.exposure()[vol_pos].abs() > 1e-12);
    }
}

use crate::{
    ad::{
        adreal::{ADReal, IsReal},
        tape::Tape,
    },
    core::{
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            fixingrequest::FixingRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::equity::equityeurooption::{EquityEuroOptionTrade, EuroOptionType},
    pricers::generalpricers::BlackClosedFormPricer,
    utils::errors::{AtlasError, Result},
};

/// # `EquityOptionState`
///
/// State struct for storing intermediate values during the pricing of an equity option, including the option value, spot price, and market data response.
#[derive(Default)]
struct EquityOptionState {
    value: Option<ADReal>,
    spot: Option<ADReal>,
    market_data: Option<MarketData>,
}

impl PricerState for EquityOptionState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl HandleValue<EquityEuroOptionTrade, EquityOptionState> for BlackClosedFormPricer {
    fn handle_value(
        &self,
        trade: &EquityEuroOptionTrade,
        state: &mut EquityOptionState,
    ) -> Result<f64> {
        let option = trade.instrument();
        let expiry = option.expiry_date();
        let index = option.market_index().clone();

        // move to the instrument level
        let tau = option
            .day_counter()
            .year_fraction(trade.trade_date(), option.expiry_date());

        Tape::start_recording();
        Tape::set_mark();

        // get and put the spot in the tape
        let spot = state.get_fixing(&index, trade.trade_date())?;
        let mut spot_ad = ADReal::new(spot);
        spot_ad.put_on_tape();
        state.spot = Some(spot_ad);

        state.put_pillars_on_tape()?;

        let strike = option.strike();
        let vol = state
            .get_volatility_surface_element(&index)?
            .surface()
            .volatility_from_date(expiry, strike)?;

        // this should discount the underyling currency curve
        let df_r = state
            .get_discount_curve_element(&index)?
            .curve()
            .discount_factor(option.expiry_date())?;

        let df_q = if let Ok(curve) = state.get_dividend_curve_element(&index) {
            curve.curve().discount_factor(option.expiry_date())?
        } else {
            ADReal::one()
        };

        let fwd: ADReal = (spot_ad * df_q / df_r).into();

        let undiscounted = BlackClosedFormPricer::black_forward_price(
            fwd,
            strike,
            vol,
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
            state.value.ok_or(AtlasError::UnexpectedErr(
                "State does not contain price, altough it was requested.".into(),
            ))?
        };

        // the mark is not being set on the value during pricing
        value.backward_to_mark()?;
        let option = trade.instrument();
        let index = option.market_index();

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        ids.push(index.to_string());
        exposures.push(
            state
                .spot
                .ok_or(AtlasError::UnexpectedErr(
                    "Spot not recorded on state".into(),
                ))?
                .adjoint()?,
        );

        for (label, pillar) in state
            .get_volatility_surface_element(index)?
            .surface()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?);
        }

        for (label, pillar) in state
            .get_discount_curve_element(index)?
            .curve()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?);
        }

        if let Ok(curve) = state.get_dividend_curve_element(index) {
            for (label, pillar) in curve.curve().pillars().unwrap_or_default() {
                ids.push(label);
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
            market_data: Some(ctx.handle_request(&md_request)?),
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
                .with_constructed_elements_request(vec![
                    ConstructedElementRequest::DiscountCurve {
                        market_index: index.clone(),
                    },
                    ConstructedElementRequest::DividendCurve {
                        market_index: index.clone(),
                    },
                    ConstructedElementRequest::VolatilitySurface {
                        market_index: index.clone(),
                    },
                ])
                .with_fixings_request(vec![FixingRequest::new(index, trade.trade_date())]),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        collections::{BTreeMap, HashMap},
        rc::Rc,
    };

    use crate::{
        ad::adreal::{ADReal, IsReal},
        core::{
            elements::{
                curveelement::{DiscountCurveElement, DividendCurveElement},
                volatilitysurfaceelement::VolatilitySurfaceElement,
            },
            marketdatahandling::{
                constructedelementstore::ConstructedElementStore,
                marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
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
            yieldtermstructure::{
                flatforwardtermstructure::FlatForwardTermStructure,
                interestratestermstructure::InterestRatesTermStructure,
            },
        },
        time::{date::Date, enums::TimeUnit, period::Period},
        utils::errors::Result,
        volatility::{
            interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
            volatilityindexing::F64Key,
        },
    };

    struct MockMarketDataProvider {
        evaluation_date: Date,
        market_data: MarketData,
    }

    impl MarketDataProvider for MockMarketDataProvider {
        fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
            Ok(MarketData::new(
                self.market_data.fixings().clone(),
                self.market_data.constructed_elements().clone(),
            ))
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    fn exposure_for_key(instrument_keys: &[String], exposures: &[f64], key: &str) -> Option<f64> {
        instrument_keys
            .iter()
            .zip(exposures.iter().copied())
            .find(|(instrument_key, _)| instrument_key.as_str() == key)
            .map(|(_, exposure)| exposure)
    }

    fn norm_cdf_black_approx(x: f64) -> f64 {
        let l = x.abs();
        let k = 1.0 / (1.0 + l * 0.231_641_9);
        let poly = ((((k * 1.330_274_429 - 1.821_255_978) * k + 1.781_477_937) * k
            - 0.356_563_782)
            * k
            + 0.319_381_530)
            * k;
        let pdf = (-0.5 * l * l).exp() * 0.398_942_280_401_432_7;
        let w = 1.0 - pdf * poly;
        if x < 0.0 { 1.0 - w } else { w }
    }

    #[test]
    fn equity_option_sensitivities_match_closed_form_delta_and_vega() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let market_index = MarketIndex::Equity("SPX".to_string());
        let six_month_days = expiry_date - trade_date;

        let spot = 100.0;
        let strike = 90.0;
        let notional = 3.0;
        let risk_free_rate = 0.03;
        let dividend_rate = 0.01;

        let option = EquityEuroOption::new(
            market_index.clone(),
            expiry_date,
            strike,
            EuroOptionType::Call,
            "SPX_CALL_90".to_string(),
        );
        let trade = EquityEuroOptionTrade::new(option.clone(), notional, trade_date);

        let discount_curve = FlatForwardTermStructure::new(
            trade_date,
            ADReal::from(risk_free_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("discount_rate".to_string());
        let dividend_curve = FlatForwardTermStructure::new(
            trade_date,
            ADReal::from(dividend_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("dividend_rate".to_string());

        let mut surface_points = BTreeMap::new();
        surface_points.insert(
            Period::new(six_month_days as i32, TimeUnit::Days),
            BTreeMap::from([
                (F64Key::new(90.0), ADReal::from(0.22)),
                (F64Key::new(110.0), ADReal::from(0.24)),
            ]),
        );
        surface_points.insert(
            Period::new(six_month_days as i32 + 365, TimeUnit::Days),
            BTreeMap::from([
                (F64Key::new(90.0), ADReal::from(0.25)),
                (F64Key::new(110.0), ADReal::from(0.27)),
            ]),
        );
        let labels = vec![
            "vol_6m_90".to_string(),
            "vol_6m_110".to_string(),
            "vol_12m_90".to_string(),
            "vol_12m_110".to_string(),
        ];

        let vol_surface = InterpolatedVolatilitySurface::new(
            trade_date,
            market_index.clone(),
            surface_points,
        )
        .with_labels(&labels);

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            market_index.clone(),
            DiscountCurveElement::new(
                market_index.clone(),
                Currency::USD,
                Rc::new(RefCell::new(discount_curve.clone())),
            ),
        );
        constructed_elements.dividend_curves_mut().insert(
            market_index.clone(),
            DividendCurveElement::new(
                market_index.clone(),
                Currency::USD,
                Rc::new(RefCell::new(dividend_curve.clone())),
            ),
        );
        constructed_elements.volatility_surfaces_mut().insert(
            market_index.clone(),
            VolatilitySurfaceElement::new(market_index.clone(), Rc::new(RefCell::new(vol_surface))),
        );

        let fixings = HashMap::from([(
            market_index.clone(),
            BTreeMap::from([(trade_date, spot)]),
        )]);
        let market_data = MarketData::new(fixings, constructed_elements);
        let provider = MockMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackClosedFormPricer;
        let results = pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)?;
        let sensitivities = if let Some(sensitivities) = results.sensitivities() {
            sensitivities
        } else {
            return Err(crate::utils::errors::AtlasError::UnexpectedErr(
                "Missing sensitivities in pricing result".to_string(),
            ));
        };

        let tau = option.day_counter().year_fraction(trade_date, expiry_date);
        let df_r = discount_curve.discount_factor(expiry_date)?.value();
        let df_q = dividend_curve.discount_factor(expiry_date)?.value();
        let vol = 0.22;

        let fwd = spot * df_q / df_r;
        let vol_sqrt_tau = vol * tau.sqrt();
        let d1 = ((fwd / strike).ln() + 0.5 * vol * vol * tau) / vol_sqrt_tau;

        let closed_form_delta = notional * df_q * norm_cdf_black_approx(d1);
        let closed_form_vega =
            notional * spot * df_q * (1.0 / (2.0 * std::f64::consts::PI).sqrt()) * (-0.5 * d1 * d1).exp()
                * tau.sqrt();

        let ad_delta = exposure_for_key(
            sensitivities.instrument_keys(),
            sensitivities.exposure(),
            "SPX",
        )
        .ok_or(crate::utils::errors::AtlasError::NotFoundErr(
            "Spot sensitivity not found".to_string(),
        ))?;

        let ad_vega = exposure_for_key(
            sensitivities.instrument_keys(),
            sensitivities.exposure(),
            "vol_6m_90",
        )
        .ok_or(crate::utils::errors::AtlasError::NotFoundErr(
            "Vol sensitivity not found".to_string(),
        ))?;

        assert!((ad_delta - closed_form_delta).abs() < 1e-5);
        assert!((ad_vega - closed_form_vega).abs() < 1e-3);

        Ok(())
    }
}

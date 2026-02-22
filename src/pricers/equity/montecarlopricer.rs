use crate::{
    ad::{
        adreal::{exp, max, ADReal, IsReal},
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
    models::{ModelKey, ModelParameters},
    pricers::generalpricers::BlackMonteCarloPricer,
    utils::errors::{AtlasError, Result},
};

/// # `MonteCarloState`
///
/// State struct for storing intermediate values during Monte Carlo pricing of an equity option.
#[derive(Default)]
struct MonteCarloState {
    value: Option<ADReal>,
    spot: Option<ADReal>,
    market_data: Option<MarketData>,
}

impl PricerState for MonteCarloState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl HandleValue<EquityEuroOptionTrade, MonteCarloState> for BlackMonteCarloPricer {
    fn handle_value(
        &self,
        trade: &EquityEuroOptionTrade,
        state: &mut MonteCarloState,
    ) -> Result<f64> {
        let option = trade.instrument();
        let expiry = option.expiry_date();
        let index = option.market_index().clone();

        let tau = option
            .day_counter()
            .year_fraction(trade.trade_date(), expiry);

        let simulation = state.get_simulation_element(&index)?;
        if simulation.draws().is_empty() {
            return Err(AtlasError::InvalidValueErr(
                "Simulation element contains no paths".into(),
            ));
        }
        let draws: Vec<f64> = simulation.draws().to_vec();
        let n_paths = draws.len();

        Tape::start_recording();
        Tape::set_mark();

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

        let df_r = state
            .get_discount_curve_element(&index)?
            .curve()
            .discount_factor(expiry)?;

        let df_q = if let Ok(div_curve) = state.get_dividend_curve_element(&index) {
            div_curve.curve().discount_factor(expiry)?
        } else {
            ADReal::one()
        };

        let fwd: ADReal = (spot_ad * df_q / df_r).into();
        let drift: ADReal = (vol * vol * (-0.5) * tau).into();
        let diffusion_scale: ADReal = (vol * tau.sqrt()).into();
        let strike_ad = ADReal::new(strike);

        let mut payoff_sum = ADReal::new(0.0);
        for &z in &draws {
            let terminal: ADReal = (fwd * exp(drift + diffusion_scale * z)).into();
            let payoff: ADReal = match option.option_type() {
                EuroOptionType::Call => max(terminal - strike_ad, ADReal::zero()).into(),
                EuroOptionType::Put => max(strike_ad - terminal, ADReal::zero()).into(),
            };
            payoff_sum = (payoff_sum + payoff).into();
        }

        #[allow(clippy::cast_precision_loss)]
        let value: ADReal = (df_r * payoff_sum * (trade.notional() / n_paths as f64)).into();
        state.value = Some(value);
        Tape::stop_recording();
        Ok(value.value())
    }
}

impl HandleSensitivities<EquityEuroOptionTrade, MonteCarloState> for BlackMonteCarloPricer {
    fn handle_sensitivities(
        &self,
        trade: &EquityEuroOptionTrade,
        state: &mut MonteCarloState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(value) = state.value {
            value
        } else {
            let _ = self.handle_value(trade, state)?;
            state.value.ok_or_else(|| {
                AtlasError::UnexpectedErr(
                    "State does not contain price, although it was requested.".into(),
                )
            })?
        };

        value.backward_to_mark()?;
        let option = trade.instrument();
        let index = option.market_index();

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        ids.push(index.to_string());
        exposures.push(
            state
                .spot
                .ok_or_else(|| AtlasError::UnexpectedErr("Spot not recorded on state".into()))?
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

        if let Ok(div_curve) = state.get_dividend_curve_element(index) {
            for (label, pillar) in div_curve.curve().pillars().unwrap_or_default() {
                ids.push(label);
                exposures.push(pillar.adjoint()?);
            }
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures))
    }
}

impl Pricer for BlackMonteCarloPricer {
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
            .ok_or_else(|| AtlasError::InvalidValueErr("Missing market data request".into()))?;

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = MonteCarloState {
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
                    ConstructedElementRequest::Simulation {
                        market_index: index.clone(),
                    },
                ])
                .with_fixings_request(vec![FixingRequest::new(index, trade.trade_date())])
                .with_model(ModelKey::Gbm, ModelParameters::Gbm(self.model_parameters)),
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
        core::{
            elements::{
                curveelement::{DiscountCurveElement, DividendCurveElement},
                simulationelement::SimulationElement,
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
        models::{GbmModelParameters, ModelKey, ModelParameters},
        pricers::generalpricers::BlackMonteCarloPricer,
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::{
                flatforwardtermstructure::FlatForwardTermStructure,
                interestratestermstructure::InterestRatesTermStructure,
            },
        },
        time::{date::Date, enums::TimeUnit, period::Period},
        utils::errors::{AtlasError, Result},
        volatility::{
            interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
            volatilityindexing::F64Key,
        },
    };

    use crate::ad::adreal::{ADReal, IsReal};

    struct SimpleMarketDataProvider {
        evaluation_date: Date,
        market_data: MarketData,
    }

    impl MarketDataProvider for SimpleMarketDataProvider {
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

    /// Builds shared market data (curves, vol surface, fixings, simulation) for a test option.
    fn setup_market_data(
        trade_date: Date,
        expiry_date: Date,
        market_index: &MarketIndex,
        spot: f64,
        risk_free_rate: f64,
        dividend_rate: f64,
        vol: f64,
        model_params: &GbmModelParameters,
    ) -> Result<MarketData> {
        let days = expiry_date - trade_date;

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
        let smile = BTreeMap::from([
            (F64Key::new(80.0), ADReal::from(vol)),
            (F64Key::new(90.0), ADReal::from(vol)),
            (F64Key::new(100.0), ADReal::from(vol)),
            (F64Key::new(110.0), ADReal::from(vol)),
            (F64Key::new(120.0), ADReal::from(vol)),
        ]);
        surface_points.insert(Period::new(days as i32, TimeUnit::Days), smile.clone());
        // Second tenor needed for bilinear interpolation grid
        surface_points.insert(Period::new(days as i32 + 365, TimeUnit::Days), smile);
        let labels = vec![
            "vol_near_80".to_string(),
            "vol_near_90".to_string(),
            "vol_near_100".to_string(),
            "vol_near_110".to_string(),
            "vol_near_120".to_string(),
            "vol_far_80".to_string(),
            "vol_far_90".to_string(),
            "vol_far_100".to_string(),
            "vol_far_110".to_string(),
            "vol_far_120".to_string(),
        ];
        let vol_surface = Rc::new(RefCell::new(
            InterpolatedVolatilitySurface::new(trade_date, market_index.clone(), surface_points)
                .with_labels(&labels),
        ));

        let draws = model_params.generate_draws();
        let simulation = SimulationElement::new(market_index.clone(), draws);

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            market_index.clone(),
            DiscountCurveElement::new(
                market_index.clone(),
                Currency::USD,
                Rc::new(RefCell::new(discount_curve)),
            ),
        );
        constructed_elements.dividend_curves_mut().insert(
            market_index.clone(),
            DividendCurveElement::new(
                market_index.clone(),
                Currency::USD,
                Rc::new(RefCell::new(dividend_curve)),
            ),
        );
        constructed_elements.volatility_surfaces_mut().insert(
            market_index.clone(),
            VolatilitySurfaceElement::new(market_index.clone(), vol_surface),
        );
        constructed_elements
            .simulations_mut()
            .insert(market_index.clone(), simulation);

        let fixings =
            HashMap::from([(market_index.clone(), BTreeMap::from([(trade_date, spot)]))]);
        Ok(MarketData::new(fixings, constructed_elements))
    }

    #[test]
    fn mc_pricer_call_price_is_positive() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let market_index = MarketIndex::Equity("SPX".to_string());

        let model_params = GbmModelParameters::new(10_000, 42);
        let market_data = setup_market_data(
            trade_date,
            expiry_date,
            &market_index,
            100.0,
            0.03,
            0.01,
            0.20,
            &model_params,
        )?;

        let option = EquityEuroOption::new(
            market_index,
            expiry_date,
            100.0,
            EuroOptionType::Call,
            "SPX_CALL_100".to_string(),
        );
        let trade = EquityEuroOptionTrade::new(option, 1.0, trade_date);
        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackMonteCarloPricer::new(model_params);
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results.price().ok_or(AtlasError::UnexpectedErr(
            "Missing price in MC pricing result".to_string(),
        ))?;

        assert!(price > 0.0, "MC call price should be positive, got {price}");
        Ok(())
    }

    #[test]
    fn mc_pricer_sensitivities_are_non_empty() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let market_index = MarketIndex::Equity("SPX".to_string());

        let model_params = GbmModelParameters::new(10_000, 7);
        let market_data = setup_market_data(
            trade_date,
            expiry_date,
            &market_index,
            100.0,
            0.03,
            0.01,
            0.20,
            &model_params,
        )?;

        let option = EquityEuroOption::new(
            market_index.clone(),
            expiry_date,
            90.0,
            EuroOptionType::Put,
            "SPX_PUT_90".to_string(),
        );
        let trade = EquityEuroOptionTrade::new(option, 2.0, trade_date);
        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackMonteCarloPricer::new(model_params);
        let results =
            pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)?;

        let sensitivities = results.sensitivities().ok_or(AtlasError::UnexpectedErr(
            "Missing sensitivities in MC pricing result".to_string(),
        ))?;

        assert!(
            !sensitivities.instrument_keys().is_empty(),
            "MC pricer should produce sensitivities"
        );
        assert_eq!(
            sensitivities.instrument_keys().len(),
            sensitivities.exposure().len(),
            "Keys and exposures must have equal length"
        );

        // Delta (spot sensitivity) should be present and negative for a put
        let delta = sensitivities
            .instrument_keys()
            .iter()
            .zip(sensitivities.exposure().iter())
            .find(|(k, _)| k.as_str() == "SPX")
            .map(|(_, v)| *v)
            .ok_or(AtlasError::NotFoundErr(
                "Spot sensitivity not found".to_string(),
            ))?;
        assert!(delta < 0.0, "Put delta should be negative, got {delta}");

        Ok(())
    }

    #[test]
    fn mc_price_converges_to_black_scholes_atm() -> Result<()> {
        use crate::{math::probability::norm_cdf::norm_cdf, time::daycounter::DayCounter};

        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let market_index = MarketIndex::Equity("SPX".to_string());

        let spot = 100.0_f64;
        let strike = 100.0_f64;
        let risk_free_rate = 0.05_f64;
        let dividend_rate = 0.02_f64;
        let vol = 0.20_f64;

        // Use the same day counter as EquityEuroOption (Actual360 default)
        let tau = DayCounter::Actual360.year_fraction(trade_date, expiry_date);

        // Closed-form Black-Scholes price
        let discount_curve = FlatForwardTermStructure::new(
            trade_date,
            ADReal::from(risk_free_rate),
            RateDefinition::default(),
        );
        let dividend_curve = FlatForwardTermStructure::new(
            trade_date,
            ADReal::from(dividend_rate),
            RateDefinition::default(),
        );
        let df_r = discount_curve.discount_factor(expiry_date)?.value();
        let df_q = dividend_curve.discount_factor(expiry_date)?.value();
        let fwd = spot * df_q / df_r;
        let d1 = ((fwd / strike).ln() + 0.5 * vol * vol * tau) / (vol * tau.sqrt());
        let d2 = d1 - vol * tau.sqrt();
        let bs_price = df_r
            * (fwd * norm_cdf(ADReal::new(d1)).value()
                - strike * norm_cdf(ADReal::new(d2)).value());

        // MC price with many paths
        let model_params = GbmModelParameters::new(200_000, 99);
        let market_data = setup_market_data(
            trade_date,
            expiry_date,
            &market_index,
            spot,
            risk_free_rate,
            dividend_rate,
            vol,
            &model_params,
        )?;

        let option = EquityEuroOption::new(
            market_index,
            expiry_date,
            strike,
            EuroOptionType::Call,
            "SPX_ATM_CALL".to_string(),
        );
        let trade = EquityEuroOptionTrade::new(option, 1.0, trade_date);
        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackMonteCarloPricer::new(model_params);
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let mc_price = results
            .price()
            .ok_or(AtlasError::UnexpectedErr("No MC price".to_string()))?;

        println!("BS price: {bs_price:.4}, MC price: {mc_price:.4}");
        // With 200k paths the MC price should be within 1% of Black-Scholes
        assert!(
            (mc_price - bs_price).abs() / bs_price < 0.01,
            "MC price {mc_price:.4} should be within 1% of BS price {bs_price:.4}"
        );

        Ok(())
    }
}

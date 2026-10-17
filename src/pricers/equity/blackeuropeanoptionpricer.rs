use crate::{
    ad::{dual::DualFwd, tape::Tape},
    core::{
        collateral::DiscountPolicy,
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
    instruments::equity::equityeuropeanoption::{EquityEuropeanOptionTrade, EuroOptionType},
    models::brownianmotion::BrownianMotion,
    utils::errors::{QSError, Result},
};

/// State struct for storing intermediate values during the pricing of an equity option, including the option value, spot price, and market data response.
#[derive(Default)]
struct EquityOptionState {
    value: Option<DualFwd>,
    spot: Option<DualFwd>,
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

/// A pricer for European equity options using the Black-Scholes model.
///
/// It calculates the option price and sensitivities based on the spot price,
/// volatility surface, discount curve, and dividend curve obtained from the market data.
/// Spot is observed on the trade date, and option time runs from trade date
/// to expiry. A missing dividend curve means zero dividends. The surface key
/// is `strike.resolve(0.0)`, whereas the payoff strike is resolved using the
/// computed forward; these differ for ATM/relative strikes.
///
/// When a [`DiscountPolicy`] is set, the pricer uses the CSA discount curve
/// for payment discounting instead of the instrument's `market_index` curve.
///
pub struct BlackEuropeanOptionPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl BlackEuropeanOptionPricer {
    /// Creates a new [`BlackEuropeanOptionPricer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            discount_policy: None,
        }
    }
}

impl Default for BlackEuropeanOptionPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl HandleValue<EquityEuropeanOptionTrade, EquityOptionState> for BlackEuropeanOptionPricer {
    fn handle_value(
        &self,
        trade: &EquityEuropeanOptionTrade,
        state: &mut EquityOptionState,
    ) -> Result<f64> {
        let option = trade.instrument();
        let expiry = option.expiry_date();
        let index = option.market_index().clone();
        let discount_index = if let Some(policy) = &self.discount_policy {
            policy.accept(option)?
        } else {
            index.clone()
        };

        // move to the instrument level
        let tau = option
            .day_counter()
            .year_fraction(trade.trade_date(), option.expiry_date());

        Tape::start_recording_fwd();
        Tape::set_mark_fwd();

        // get and put the spot in the tape
        let spot = state.get_fixing(&index, trade.trade_date())?;
        let mut spot_ad = DualFwd::new(spot);
        spot_ad.put_on_tape();
        state.spot = Some(spot_ad);

        state.put_pillars_on_tape()?;

        let strike = option.strike();
        let vol = state
            .get_volatility_surface_element(&index)?
            .surface()
            .volatility_from_date(expiry, strike.resolve(0.0))?;

        // this should discount the underyling currency curve
        let df_r = state
            .get_discount_curve_element(&discount_index)?
            .curve()
            .discount_factor(option.expiry_date())?;

        let df_q = if let Ok(curve) = state.get_dividend_curve_element(&index) {
            curve.curve().discount_factor(option.expiry_date())?
        } else {
            DualFwd::one()
        };

        let fwd: DualFwd = (spot_ad * df_q / df_r).into();

        let undiscounted = BrownianMotion::<DualFwd>::closed_form_price(
            fwd,
            strike.resolve(fwd.value()),
            vol,
            tau,
            matches!(option.option_type(), EuroOptionType::Call),
        )?;

        let value: DualFwd = (df_r * undiscounted * trade.notional()).into();
        state.value = Some(value);
        Tape::stop_recording_fwd();
        Ok(value.value())
    }
}

impl HandleSensitivities<EquityEuropeanOptionTrade, EquityOptionState>
    for BlackEuropeanOptionPricer
{
    fn handle_sensitivities(
        &self,
        trade: &EquityEuropeanOptionTrade,
        state: &mut EquityOptionState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(value) = state.value {
            value
        } else {
            let _ = self.handle_value(trade, state)?;
            state.value.ok_or_else(|| {
                QSError::UnexpectedErr(
                    "State does not contain price, altough it was requested.".into(),
                )
            })?
        };

        // the mark is not being set on the value during pricing
        value.backward_to_mark()?;
        let option = trade.instrument();
        let index = option.market_index();
        let policy_discount_index = if let Some(policy) = &self.discount_policy {
            Some(policy.accept(option)?)
        } else {
            None
        };
        let discount_index = policy_discount_index.as_ref().unwrap_or(index);

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        ids.push(index.to_string());
        exposures.push(
            state
                .spot
                .ok_or_else(|| QSError::UnexpectedErr("Spot not recorded on state".into()))?
                .adjoint()?
                .value(),
        );

        for (label, pillar) in state
            .get_volatility_surface_element(index)?
            .surface()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?.value());
        }

        for (label, pillar) in state
            .get_discount_curve_element(discount_index)?
            .curve()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?.value());
        }

        if let Ok(curve) = state.get_dividend_curve_element(index) {
            for (label, pillar) in curve.curve().pillars().unwrap_or_default() {
                ids.push(label);
                exposures.push(pillar.adjoint()?.value());
            }
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures)
            .aggregate())
    }
}

impl Pricer for BlackEuropeanOptionPricer {
    type Item = EquityEuropeanOptionTrade;
    type Policy = dyn DiscountPolicy;
    fn evaluate(
        &self,
        trade: &EquityEuropeanOptionTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let option = trade.instrument();
        let identifier = option.identifier();

        let md_request = self
            .market_data_request(trade)
            .ok_or_else(|| QSError::InvalidValueErr("Missing market data request".into()))?;

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
        let mut elements = vec![
            ConstructedElementRequest::DiscountCurve {
                market_index: index.clone(),
            },
            ConstructedElementRequest::DividendCurve {
                market_index: index.clone(),
            },
            ConstructedElementRequest::VolatilitySurface {
                market_index: index.clone(),
            },
        ];
        let fixings = vec![FixingRequest::new(index.clone(), trade.trade_date())];

        if let Some(policy) = &self.discount_policy {
            let collateral_index = policy.accept(option).ok()?;
            if collateral_index != index {
                elements.push(ConstructedElementRequest::DiscountCurve {
                    market_index: collateral_index,
                });
            }
        }

        let request = MarketDataRequest::default()
            .with_constructed_elements_request(elements)
            .with_fixings_request(fixings);
        Some(request)
    }

    fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
        self.discount_policy = Some(policy);
    }

    fn discount_policy(&self) -> Option<&Self::Policy> {
        self.discount_policy.as_deref()
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
        ad::dual::DualFwd,
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
            trade::Side,
        },
        indices::marketindex::MarketIndex,
        instruments::equity::equityeuropeanoption::{
            EquityEuropeanOption, EquityEuropeanOptionTrade, EuroOptionType,
        },
        math::probability::norm_cdf::norm_cdf,
        pricers::equity::blackeuropeanoptionpricer::BlackEuropeanOptionPricer,
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::{
                flatforwardtermstructure::FlatForwardTermStructure,
                interestratestermstructure::InterestRatesTermStructure,
            },
        },
        time::{date::Date, enums::TimeUnit, period::Period},
        utils::errors::{QSError, Result},
        volatility::{
            interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
            volatilityindexing::{F64Key, SmileType, Strike, VolatilityType},
            volatilitysurface::VolatilitySurface,
        },
    };

    fn exposure_for_key(instrument_keys: &[String], exposures: &[f64], key: &str) -> Option<f64> {
        instrument_keys
            .iter()
            .zip(exposures.iter().copied())
            .find(|(instrument_key, _)| instrument_key.as_str() == key)
            .map(|(_, exposure)| exposure)
    }

    /// Helper function to set up market data for equity option testing
    fn setup_markup_for_equity_option_test(
        trade_date: Date,
        expiry_date: Date,
        market_index: &MarketIndex,
        spot: f64,
        risk_free_rate: f64,
        dividend_rate: f64,
    ) -> Result<(
        MarketData,
        FlatForwardTermStructure<DualFwd>,
        FlatForwardTermStructure<DualFwd>,
        Rc<RefCell<InterpolatedVolatilitySurface<DualFwd>>>,
    )> {
        let six_month_days = expiry_date - trade_date;

        let discount_curve = FlatForwardTermStructure::new(
            trade_date,
            DualFwd::from(risk_free_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("discount_rate".to_string());

        let dividend_curve = FlatForwardTermStructure::new(
            trade_date,
            DualFwd::from(dividend_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("dividend_rate".to_string());

        let mut surface_points = BTreeMap::new();
        surface_points.insert(
            Period::new(six_month_days as i32, TimeUnit::Days),
            BTreeMap::from([
                (F64Key::new(70.0), DualFwd::from(0.28)),
                (F64Key::new(80.0), DualFwd::from(0.24)),
                (F64Key::new(90.0), DualFwd::from(0.22)),
                (F64Key::new(100.0), DualFwd::from(0.23)),
                (F64Key::new(110.0), DualFwd::from(0.24)),
                (F64Key::new(120.0), DualFwd::from(0.26)),
                (F64Key::new(130.0), DualFwd::from(0.28)),
            ]),
        );
        surface_points.insert(
            Period::new(six_month_days as i32 + 365, TimeUnit::Days),
            BTreeMap::from([
                (F64Key::new(70.0), DualFwd::from(0.30)),
                (F64Key::new(80.0), DualFwd::from(0.26)),
                (F64Key::new(90.0), DualFwd::from(0.25)),
                (F64Key::new(100.0), DualFwd::from(0.25)),
                (F64Key::new(110.0), DualFwd::from(0.27)),
                (F64Key::new(120.0), DualFwd::from(0.29)),
                (F64Key::new(130.0), DualFwd::from(0.30)),
            ]),
        );
        let labels = vec![
            "vol_6m_70".to_string(),
            "vol_6m_80".to_string(),
            "vol_6m_90".to_string(),
            "vol_6m_100".to_string(),
            "vol_6m_110".to_string(),
            "vol_6m_120".to_string(),
            "vol_6m_130".to_string(),
            "vol_12m_70".to_string(),
            "vol_12m_80".to_string(),
            "vol_12m_90".to_string(),
            "vol_12m_100".to_string(),
            "vol_12m_110".to_string(),
            "vol_12m_120".to_string(),
            "vol_12m_130".to_string(),
        ];

        let vol_surface = Rc::new(RefCell::new(
            InterpolatedVolatilitySurface::new(
                trade_date,
                market_index.clone(),
                surface_points,
                VolatilityType::Black,
                SmileType::Strike,
            )
            .with_labels(&labels),
        ));

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            market_index.clone(),
            DiscountCurveElement::new(
                market_index.clone(),
                Rc::new(RefCell::new(discount_curve.clone())),
            ),
        );
        constructed_elements.dividend_curves_mut().insert(
            market_index.clone(),
            DividendCurveElement::new(
                market_index.clone(),
                Rc::new(RefCell::new(dividend_curve.clone())),
            ),
        );
        constructed_elements.volatility_surfaces_mut().insert(
            market_index.clone(),
            VolatilitySurfaceElement::new(market_index.clone(), vol_surface.clone()),
        );

        let fixings = HashMap::from([(market_index.clone(), BTreeMap::from([(trade_date, spot)]))]);
        let market_data = MarketData::new(fixings, constructed_elements);

        Ok((market_data, discount_curve, dividend_curve, vol_surface))
    }

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

    #[test]
    fn equity_option_sensitivities_match_closed_form_delta_and_vega() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let market_index = MarketIndex::Equity("SPX".to_string());

        let spot = 100.0;
        let strike = 90.0;
        let notional = 3.0;
        let risk_free_rate = 0.03;
        let dividend_rate = 0.01;

        // Set up market data (curves, surfaces, fixings)
        let (market_data, discount_curve, dividend_curve, vol_surface) =
            setup_markup_for_equity_option_test(
                trade_date,
                expiry_date,
                &market_index,
                spot,
                risk_free_rate,
                dividend_rate,
            )?;

        // Create the trade
        let option = EquityEuropeanOption::new(
            market_index,
            expiry_date,
            Strike::Absolute(strike),
            EuroOptionType::Call,
            "SPX_CALL_90".to_string(),
        );
        let trade =
            EquityEuropeanOptionTrade::new(option.clone(), notional, trade_date, Side::LongReceive);

        // Price using the pricer
        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackEuropeanOptionPricer::new();
        let results =
            pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)?;
        let sensitivities = results.sensitivities().ok_or_else(|| {
            QSError::UnexpectedErr("Missing sensitivities in pricing result".to_string())
        })?;

        // Compute closed-form delta and vega for comparison
        let tau = option.day_counter().year_fraction(trade_date, expiry_date);
        let df_r = discount_curve.discount_factor(expiry_date)?.value();
        let df_q = dividend_curve.discount_factor(expiry_date)?.value();
        let vol = vol_surface
            .borrow()
            .volatility_from_date(expiry_date, strike)?
            .value();

        let fwd = spot * df_q / df_r;
        let vol_sqrt_tau = vol * tau.sqrt();
        let d1 = (0.5 * vol * vol).mul_add(tau, (fwd / strike).ln()) / vol_sqrt_tau;

        let closed_form_delta = notional * df_q * norm_cdf(d1);
        let closed_form_vega = notional
            * spot
            * df_q
            * (1.0 / (2.0 * std::f64::consts::PI).sqrt())
            * (-0.5 * d1 * d1).exp()
            * tau.sqrt();

        // Verify AD sensitivities match closed-form
        let ad_delta = exposure_for_key(
            sensitivities.instrument_keys(),
            sensitivities.exposure(),
            "EQ:SPX",
        )
        .ok_or_else(|| QSError::NotFoundErr("Spot sensitivity not found".to_string()))?;

        let ad_vega = exposure_for_key(
            sensitivities.instrument_keys(),
            sensitivities.exposure(),
            "vol_6m_90",
        )
        .ok_or_else(|| QSError::NotFoundErr("Vol sensitivity not found".to_string()))?;

        println!("Closed-form delta: {closed_form_delta}, AD delta: {ad_delta}");
        assert!((ad_delta - closed_form_delta).abs() < 1e-5);
        assert!((ad_vega - closed_form_vega).abs() < 1e-3);

        Ok(())
    }

    #[test]
    fn equity_option_pricing_works_with_rayon_parallelism() -> Result<()> {
        use rayon::prelude::*;

        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let market_index = MarketIndex::Equity("SPX".to_string());

        let spot = 100.0;
        let notional = 1.0;
        let risk_free_rate = 0.03;
        let dividend_rate = 0.01;

        // Create multiple options with different strikes
        let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];

        // Price all options in parallel using rayon
        // Each thread generates its own copy of market data to avoid Rc<RefCell<>> thread safety issues
        let results: Vec<_> = strikes
            .par_iter()
            .map(|&strike| {
                // Each thread generates its own market data
                let (market_data, _discount_curve, _dividend_curve, _vol_surface) =
                    setup_markup_for_equity_option_test(
                        trade_date,
                        expiry_date,
                        &market_index,
                        spot,
                        risk_free_rate,
                        dividend_rate,
                    )
                    .unwrap_or_else(|_| panic!("Failed to set up market data for strike {strike}"));

                let option = EquityEuropeanOption::new(
                    market_index.clone(),
                    expiry_date,
                    Strike::Absolute(strike),
                    EuroOptionType::Call,
                    format!("SPX_CALL_{}", strike as i32),
                );
                let trade =
                    EquityEuropeanOptionTrade::new(option, notional, trade_date, Side::LongReceive);

                let provider = SimpleMarketDataProvider {
                    evaluation_date: trade_date,
                    market_data,
                };

                let pricer = BlackEuropeanOptionPricer::new();
                let eval_results =
                    pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider);

                (strike, eval_results)
            })
            .collect();

        // Verify all pricing results are valid
        for (strike, result) in results {
            let eval_results = result?;
            let price = eval_results.price().ok_or_else(|| {
                QSError::UnexpectedErr(format!("Missing price for strike {strike}"))
            })?;
            let sensitivities = eval_results.sensitivities().ok_or_else(|| {
                QSError::UnexpectedErr(format!("Missing sensitivities for strike {strike}"))
            })?;

            // Verify price is positive for call option
            assert!(price > 0.0, "Price should be positive for strike {strike}");

            // Verify we have sensitivities
            assert!(
                !sensitivities.instrument_keys().is_empty(),
                "Should have sensitivities for strike {strike}"
            );

            // Verify spot sensitivity exists (delta)
            let delta = exposure_for_key(
                sensitivities.instrument_keys(),
                sensitivities.exposure(),
                "EQ:SPX",
            );
            assert!(
                delta.is_some(),
                "Should have spot sensitivity for strike {strike}"
            );

            // Verify the delta is between 0 and 1 for call options
            let delta_val = delta.unwrap();
            assert!(
                delta_val > 0.0 && delta_val < 1.0,
                "Delta should be between 0 and 1 for call option with strike {strike}"
            );
        }

        Ok(())
    }

    /// Prices a unit-notional option, returning the price and, optionally, the AD delta.
    fn price_unit_option(
        trade_date: Date,
        expiry_date: Date,
        spot: f64,
        strike: f64,
        option_type: EuroOptionType,
        risk_free_rate: f64,
        dividend_rate: f64,
        with_delta: bool,
    ) -> Result<(f64, Option<f64>)> {
        let market_index = MarketIndex::Equity("SPX".to_string());
        let (market_data, _, _, _) = setup_markup_for_equity_option_test(
            trade_date,
            expiry_date,
            &market_index,
            spot,
            risk_free_rate,
            dividend_rate,
        )?;

        let option = EquityEuropeanOption::new(
            market_index,
            expiry_date,
            Strike::Absolute(strike),
            option_type,
            format!("SPX_{strike}"),
        );
        let trade = EquityEuropeanOptionTrade::new(option, 1.0, trade_date, Side::LongReceive);
        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let requests: &[Request] = if with_delta {
            &[Request::Value, Request::Sensitivities]
        } else {
            &[Request::Value]
        };

        let pricer = BlackEuropeanOptionPricer::new();
        let results = pricer.evaluate(&trade, requests, &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;
        let delta = if with_delta {
            let sensitivities = results
                .sensitivities()
                .ok_or_else(|| QSError::UnexpectedErr("Missing sensitivities".into()))?;
            exposure_for_key(
                sensitivities.instrument_keys(),
                sensitivities.exposure(),
                "EQ:SPX",
            )
        } else {
            None
        };
        Ok((price, delta))
    }

    /// Boundary test: for every strike on the ladder, the call price must
    /// respect the no-arbitrage bounds
    /// `max(0, df_q * S - df_r * K) <= C <= df_q * S`.
    #[test]
    fn call_price_respects_no_arbitrage_bounds_across_strike_ladder() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let market_index = MarketIndex::Equity("SPX".to_string());
        let spot = 100.0;
        let risk_free_rate = 0.03;
        let dividend_rate = 0.01;

        let (_, discount_curve, dividend_curve, _) = setup_markup_for_equity_option_test(
            trade_date,
            expiry_date,
            &market_index,
            spot,
            risk_free_rate,
            dividend_rate,
        )?;
        let df_r = discount_curve.discount_factor(expiry_date)?.value();
        let df_q = dividend_curve.discount_factor(expiry_date)?.value();

        for strike in [75.0, 85.0, 95.0, 105.0, 115.0, 125.0] {
            let (price, _) = price_unit_option(
                trade_date,
                expiry_date,
                spot,
                strike,
                EuroOptionType::Call,
                risk_free_rate,
                dividend_rate,
                false,
            )?;

            let lower = (df_q * spot - df_r * strike).max(0.0);
            let upper = df_q * spot;
            assert!(
                price >= lower - 1e-9,
                "Call price {price} below intrinsic lower bound {lower} for strike {strike}"
            );
            assert!(
                price <= upper + 1e-9,
                "Call price {price} above upper bound {upper} for strike {strike}"
            );
        }
        Ok(())
    }

    /// Boundary test: put-call parity `C - P = df_q * S - df_r * K` must hold
    /// exactly (same vol is used for both legs) across the strike ladder.
    #[test]
    fn put_call_parity_holds_across_strike_ladder() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let market_index = MarketIndex::Equity("SPX".to_string());
        let spot = 100.0;
        let risk_free_rate = 0.03;
        let dividend_rate = 0.01;

        let (_, discount_curve, dividend_curve, _) = setup_markup_for_equity_option_test(
            trade_date,
            expiry_date,
            &market_index,
            spot,
            risk_free_rate,
            dividend_rate,
        )?;
        let df_r = discount_curve.discount_factor(expiry_date)?.value();
        let df_q = dividend_curve.discount_factor(expiry_date)?.value();

        for strike in [75.0, 85.0, 95.0, 105.0, 115.0, 125.0] {
            let (call, _) = price_unit_option(
                trade_date,
                expiry_date,
                spot,
                strike,
                EuroOptionType::Call,
                risk_free_rate,
                dividend_rate,
                false,
            )?;
            let (put, _) = price_unit_option(
                trade_date,
                expiry_date,
                spot,
                strike,
                EuroOptionType::Put,
                risk_free_rate,
                dividend_rate,
                false,
            )?;

            let parity = df_q * spot - df_r * strike;
            assert!(
                (call - put - parity).abs() < 1e-8,
                "Put-call parity violated at strike {strike}: C - P = {}, expected {parity}",
                call - put
            );
        }
        Ok(())
    }

    /// Boundary test: a one-day option must converge to its discounted
    /// intrinsic value (deep ITM) and to zero (deep OTM).
    #[test]
    fn short_dated_option_converges_to_intrinsic_value() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(1, TimeUnit::Days);
        let market_index = MarketIndex::Equity("SPX".to_string());
        let spot = 100.0;
        let risk_free_rate = 0.03;
        let dividend_rate = 0.01;

        let (_, discount_curve, dividend_curve, _) = setup_markup_for_equity_option_test(
            trade_date,
            expiry_date,
            &market_index,
            spot,
            risk_free_rate,
            dividend_rate,
        )?;
        let df_r = discount_curve.discount_factor(expiry_date)?.value();
        let df_q = dividend_curve.discount_factor(expiry_date)?.value();

        // Deep ITM call: price ~ discounted intrinsic.
        let (itm_price, _) = price_unit_option(
            trade_date,
            expiry_date,
            spot,
            80.0,
            EuroOptionType::Call,
            risk_free_rate,
            dividend_rate,
            false,
        )?;
        let intrinsic = df_q * spot - df_r * 80.0;
        assert!(
            (itm_price - intrinsic).abs() < 1e-6,
            "Short-dated ITM call {itm_price} should equal discounted intrinsic {intrinsic}"
        );

        // Deep OTM call: price ~ 0.
        let (otm_price, _) = price_unit_option(
            trade_date,
            expiry_date,
            spot,
            120.0,
            EuroOptionType::Call,
            risk_free_rate,
            dividend_rate,
            false,
        )?;
        assert!(
            otm_price.abs() < 1e-8,
            "Short-dated OTM call should be worthless, got {otm_price}"
        );
        Ok(())
    }

    /// Ladder test: across the strike ladder, the AD spot delta must match a
    /// central finite-difference bump-and-reprice of the spot fixing.
    #[test]
    fn delta_ladder_matches_finite_difference() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let spot = 100.0;
        let risk_free_rate = 0.03;
        let dividend_rate = 0.01;
        let bump = 1e-2;

        for option_type in [EuroOptionType::Call, EuroOptionType::Put] {
            let type_label = match option_type {
                EuroOptionType::Call => "call",
                EuroOptionType::Put => "put",
            };
            for strike in [80.0, 90.0, 100.0, 110.0, 120.0] {
                let (_, ad_delta) = price_unit_option(
                    trade_date,
                    expiry_date,
                    spot,
                    strike,
                    option_type.clone(),
                    risk_free_rate,
                    dividend_rate,
                    true,
                )?;
                let ad_delta = ad_delta.ok_or_else(|| {
                    QSError::NotFoundErr(format!("Spot delta missing for strike {strike}"))
                })?;

                let (price_up, _) = price_unit_option(
                    trade_date,
                    expiry_date,
                    spot + bump,
                    strike,
                    option_type.clone(),
                    risk_free_rate,
                    dividend_rate,
                    false,
                )?;
                let (price_down, _) = price_unit_option(
                    trade_date,
                    expiry_date,
                    spot - bump,
                    strike,
                    option_type.clone(),
                    risk_free_rate,
                    dividend_rate,
                    false,
                )?;
                let fd_delta = (price_up - price_down) / (2.0 * bump);

                assert!(
                    (ad_delta - fd_delta).abs() < 1e-6,
                    "AD delta {ad_delta} vs FD delta {fd_delta} mismatch for {type_label} strike {strike}"
                );
            }
        }
        Ok(())
    }
}

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
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::rates::caplet::{CapFloorType, CapletTrade},
    pricers::generalpricers::BlackClosedFormPricer,
    utils::errors::{AtlasError, Result},
};

/// # `BlackCapletPricer`
///
/// Prices a caplet (or floorlet) using the Black (log-normal) model.
///
/// The forward rate for the period is derived from the discount curve:
/// `F = (df(start) / df(end) - 1) / α`
///
/// The Black formula is then applied with the volatility obtained from the
/// volatility surface keyed by the strike rate.
pub struct BlackCapletPricer;

/// State for Black caplet pricing.
#[derive(Default)]
struct BlackCapletState {
    value: Option<ADReal>,
    market_data: Option<MarketData>,
}

impl PricerState for BlackCapletState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl HandleValue<CapletTrade, BlackCapletState> for BlackCapletPricer {
    fn handle_value(&self, trade: &CapletTrade, state: &mut BlackCapletState) -> Result<f64> {
        let caplet = trade.instrument();
        let index = caplet.market_index();
        let start_date = caplet.start_date();
        let end_date = caplet.end_date();
        let payment_date = caplet.payment_date();
        let strike = caplet.strike();
        let alpha = caplet.accrual_factor();

        // Time to expiry: from trade date to fixing date
        let tau = caplet
            .rate_definition()
            .day_counter()
            .year_fraction(trade.trade_date(), start_date);

        Tape::start_recording();
        Tape::set_mark();

        state.put_pillars_on_tape()?;

        let df_start = state
            .get_discount_curve_element(&index)?
            .curve()
            .discount_factor(start_date)?;

        let df_end = state
            .get_discount_curve_element(&index)?
            .curve()
            .discount_factor(end_date)?;

        let df_pay = state
            .get_discount_curve_element(&index)?
            .curve()
            .discount_factor(payment_date)?;

        // Forward rate derived from discount factors (simple compounding)
        let fwd: ADReal = ((df_start / df_end - ADReal::one()) / alpha).into();

        let vol = state
            .get_volatility_surface_element(&index)?
            .surface()
            .volatility_from_date(start_date, strike)?;

        let is_cap = matches!(caplet.option_type(), CapFloorType::Cap);
        let undiscounted =
            BlackClosedFormPricer::black_forward_price(fwd, strike, vol, tau, is_cap);

        let value: ADReal = (df_pay * undiscounted * alpha * trade.notional()).into();
        state.value = Some(value);

        Tape::stop_recording();
        Ok(value.value())
    }
}

impl HandleSensitivities<CapletTrade, BlackCapletState> for BlackCapletPricer {
    fn handle_sensitivities(
        &self,
        trade: &CapletTrade,
        state: &mut BlackCapletState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(v) = state.value {
            v
        } else {
            let _ = self.handle_value(trade, state)?;
            state.value.ok_or_else(|| {
                AtlasError::UnexpectedErr(
                    "State does not contain price after value computation.".into(),
                )
            })?
        };

        value.backward_to_mark()?;

        let caplet = trade.instrument();
        let index = caplet.market_index();

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        // Discount curve sensitivities
        for (label, pillar) in state
            .get_discount_curve_element(&index)?
            .curve()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?);
        }

        // Volatility surface sensitivities
        for (label, pillar) in state
            .get_volatility_surface_element(&index)?
            .surface()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?);
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures))
    }
}

impl Pricer for BlackCapletPricer {
    type Item = CapletTrade;

    fn evaluate(
        &self,
        trade: &CapletTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let caplet = trade.instrument();
        let identifier = caplet.identifier();

        let md_request = self
            .market_data_request(trade)
            .ok_or_else(|| AtlasError::InvalidValueErr("Missing market data request".into()))?;

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = BlackCapletState {
            value: None,
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

    fn market_data_request(&self, trade: &CapletTrade) -> Option<MarketDataRequest> {
        let index = trade.instrument().market_index();
        Some(
            MarketDataRequest::default().with_constructed_elements_request(vec![
                ConstructedElementRequest::DiscountCurve {
                    market_index: index.clone(),
                },
                ConstructedElementRequest::VolatilitySurface {
                    market_index: index,
                },
            ]),
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
                curveelement::DiscountCurveElement,
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
        instruments::rates::caplet::{CapFloorType, Caplet, CapletTrade},
        math::probability::norm_cdf::norm_cdf,
        pricers::rates::blackcapletpricer::BlackCapletPricer,
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
            volatilityindexing::F64Key, volatilitysurface::VolatilitySurface,
        },
    };

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

    /// Builds a simple market data context for caplet tests.
    ///
    /// The volatility surface is a 2×2 grid bracketing (start_date, strike)
    /// so that bilinear interpolation succeeds.
    fn setup_caplet_market_data(
        trade_date: Date,
        start_date: Date,
        end_date: Date,
        market_index: &MarketIndex,
        risk_free_rate: f64,
        flat_vol: f64,
        strike: f64,
    ) -> Result<(
        MarketData,
        FlatForwardTermStructure<ADReal>,
        Rc<RefCell<InterpolatedVolatilitySurface<ADReal>>>,
    )> {
        let discount_curve = FlatForwardTermStructure::new(
            trade_date,
            ADReal::from(risk_free_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("discount_rate".to_string());

        // Build a 2-expiry × 2-strike surface that brackets (start_date, strike).
        // The query at start_date must lie strictly inside the time grid.
        let days_to_start = start_date - trade_date;
        let days_before_start = days_to_start / 2; // earlier slice
        let days_after_start = days_to_start + (end_date - start_date); // later slice

        let strike_lo = strike * 0.5;
        let strike_hi = strike * 1.5;

        let mut surface_points = BTreeMap::new();
        surface_points.insert(
            Period::new(
                i32::try_from(days_before_start).unwrap_or(0),
                TimeUnit::Days,
            ),
            BTreeMap::from([
                (F64Key::new(strike_lo), ADReal::from(flat_vol)),
                (F64Key::new(strike_hi), ADReal::from(flat_vol)),
            ]),
        );
        surface_points.insert(
            Period::new(
                i32::try_from(days_after_start).unwrap_or(0),
                TimeUnit::Days,
            ),
            BTreeMap::from([
                (F64Key::new(strike_lo), ADReal::from(flat_vol)),
                (F64Key::new(strike_hi), ADReal::from(flat_vol)),
            ]),
        );

        let labels = vec![
            "vol_t0_lo".to_string(),
            "vol_t0_hi".to_string(),
            "vol_t1_lo".to_string(),
            "vol_t1_hi".to_string(),
        ];

        let vol_surface = Rc::new(RefCell::new(
            InterpolatedVolatilitySurface::new(trade_date, market_index.clone(), surface_points)
                .with_labels(&labels),
        ));

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            market_index.clone(),
            DiscountCurveElement::new(
                market_index.clone(),
                Currency::USD,
                Rc::new(RefCell::new(discount_curve.clone())),
            ),
        );
        constructed_elements.volatility_surfaces_mut().insert(
            market_index.clone(),
            VolatilitySurfaceElement::new(market_index.clone(), vol_surface.clone()),
        );

        let market_data = MarketData::new(HashMap::new(), constructed_elements);
        Ok((market_data, discount_curve, vol_surface))
    }

    #[test]
    fn black_caplet_price_matches_closed_form() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let start_date = trade_date + Period::new(6, TimeUnit::Months);
        let end_date = start_date + Period::new(3, TimeUnit::Months);
        let market_index = MarketIndex::TermSOFR3m;

        let notional = 1_000_000.0;
        let strike = 0.05;
        let risk_free_rate = 0.04;
        let flat_vol = 0.20;

        let (market_data, discount_curve, vol_surface) = setup_caplet_market_data(
            trade_date,
            start_date,
            end_date,
            &market_index,
            risk_free_rate,
            flat_vol,
            strike,
        )?;

        let rate_def = RateDefinition::default();
        let caplet = Caplet::new(
            "SOFR3M_CAPLET".to_string(),
            market_index,
            start_date,
            end_date,
            end_date,
            CapFloorType::Cap,
            strike,
            rate_def,
        );
        let trade = CapletTrade::new(caplet.clone(), trade_date, notional);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackCapletPricer;
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or(AtlasError::UnexpectedErr("Missing price".to_string()))?;

        // Compute closed-form Black caplet price
        let tau = rate_def
            .day_counter()
            .year_fraction(trade_date, start_date);
        let alpha = caplet.accrual_factor();
        let df_start = discount_curve.discount_factor(start_date)?.value();
        let df_end = discount_curve.discount_factor(end_date)?.value();
        let df_pay = discount_curve.discount_factor(end_date)?.value();
        let fwd = (df_start / df_end - 1.0) / alpha;
        let vol = vol_surface
            .borrow()
            .volatility_from_date(start_date, strike)?
            .value();
        let vol_sqrt_tau = vol * tau.sqrt();
        let d1 = ((fwd / strike).ln() + 0.5 * vol * vol * tau) / vol_sqrt_tau;
        let d2 = d1 - vol_sqrt_tau;
        let closed_form =
            notional * alpha * df_pay * (fwd * norm_cdf(d1) - strike * norm_cdf(d2));

        println!("Pricer price: {price}");
        println!("Closed-form price: {closed_form}");
        assert!((price - closed_form).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn black_caplet_sensitivities_are_non_empty() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let start_date = trade_date + Period::new(6, TimeUnit::Months);
        let end_date = start_date + Period::new(3, TimeUnit::Months);
        let market_index = MarketIndex::TermSOFR3m;

        let notional = 1_000_000.0;
        let strike = 0.05;

        let (market_data, _discount_curve, _vol_surface) = setup_caplet_market_data(
            trade_date,
            start_date,
            end_date,
            &market_index,
            0.04,
            0.20,
            strike,
        )?;

        let caplet = Caplet::new(
            "SOFR3M_CAPLET".to_string(),
            market_index,
            start_date,
            end_date,
            end_date,
            CapFloorType::Cap,
            strike,
            RateDefinition::default(),
        );
        let trade = CapletTrade::new(caplet, trade_date, notional);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackCapletPricer;
        let results =
            pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)?;

        assert!(results.price().is_some());
        let sens = results
            .sensitivities()
            .ok_or(AtlasError::UnexpectedErr("Missing sensitivities".to_string()))?;
        assert!(!sens.instrument_keys().is_empty());
        assert_eq!(sens.instrument_keys().len(), sens.exposure().len());

        Ok(())
    }

    #[test]
    fn black_floorlet_price_positive() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let start_date = trade_date + Period::new(6, TimeUnit::Months);
        let end_date = start_date + Period::new(3, TimeUnit::Months);
        let market_index = MarketIndex::TermSOFR3m;

        let notional = 1_000_000.0;
        // Strike above fwd so floor has intrinsic value
        let strike = 0.08;

        let (market_data, _discount_curve, _vol_surface) = setup_caplet_market_data(
            trade_date,
            start_date,
            end_date,
            &market_index,
            0.04,
            0.20,
            strike,
        )?;

        let caplet = Caplet::new(
            "SOFR3M_FLOORLET".to_string(),
            market_index,
            start_date,
            end_date,
            end_date,
            CapFloorType::Floor,
            strike,
            RateDefinition::default(),
        );
        let trade = CapletTrade::new(caplet, trade_date, notional);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackCapletPricer;
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or(AtlasError::UnexpectedErr("Missing price".to_string()))?;

        println!("Floorlet price: {price}");
        assert!(price > 0.0);

        Ok(())
    }
}

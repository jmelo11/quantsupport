use crate::{
    ad::{
        adreal::{ADReal, IsReal},
        tape::Tape,
    },
    core::{
        collateral::DiscountPolicy,
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
    instruments::rates::capletfloorlet::{CapletFloorlet, CapletFloorletTrade, CapletFloorletType},
    pricers::pricerdefinitions::BlackClosedFormPricer,
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
};
use std::collections::HashSet;

/// # `BlackCapletPricer`
///
/// Prices a caplet or floorlet using the Black (log-normal) model.
///
/// The forward rate for the period is derived from the discount curve using
/// the conventions specified in the caplet's rate definition:
/// `F = curve.forward_rate(start_date, end_date, comp, freq)`
///
/// The effective strike is resolved from the instrument's [`Strike`] before
/// querying the volatility surface:
/// - [`Strike::Absolute(k)`] — `K_eff = k`
/// - [`Strike::Atm`] — `K_eff = F`
/// - [`Strike::Relative(s)`] — `K_eff = F + s`
///
/// When a [`DiscountPolicy`] is set, the pricer uses the CSA discount curve
/// for payment discounting instead of the instrument's `market_index` curve.
pub struct BlackCapletPricer {
    discount_policy: Option<Box<dyn DiscountPolicy<CapletFloorlet>>>,
}

impl BlackCapletPricer {
    /// Creates a new [`BlackCapletPricer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            discount_policy: None,
        }
    }
}

impl Default for BlackCapletPricer {
    fn default() -> Self {
        Self::new()
    }
}

/// State for Black caplet/floorlet pricing.
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

impl HandleValue<CapletFloorletTrade, BlackCapletState> for BlackCapletPricer {
    fn handle_value(
        &self,
        trade: &CapletFloorletTrade,
        state: &mut BlackCapletState,
    ) -> Result<f64> {
        let caplet = trade.instrument();
        let index = caplet.market_index();
        let discount_index = if let Some(policy) = &self.discount_policy {
            policy.accept(caplet)?
        } else {
            index.clone()
        };
        let start_date = caplet.start_date();
        let end_date = caplet.end_date();
        let payment_date = caplet.payment_date();
        let rate_def = caplet.rate_definition();
        let alpha = caplet.accrual_factor();

        // Time to expiry: from trade date to fixing date
        let tau = rate_def
            .day_counter()
            .year_fraction(trade.trade_date(), start_date);

        Tape::start_recording();
        Tape::set_mark();

        state.put_pillars_on_tape()?;

        // Forward rate using the conventions from the caplet's rate definition
        let fwd: ADReal = state
            .get_discount_curve_element(&index)?
            .curve()
            .forward_rate(
                start_date,
                end_date,
                rate_def.compounding(),
                rate_def.frequency(),
            )?;

        // Resolve effective strike from the instrument's strike specification
        let effective_strike = match caplet.strike() {
            Strike::Absolute(k) => k,
            Strike::Atm => fwd.value(),
            Strike::Relative(spread) => fwd.value() + spread,
        };

        // Payment discounting uses CSA collateral curve when a discount policy is provided.
        let df_pay = state
            .get_discount_curve_element(&discount_index)?
            .curve()
            .discount_factor(payment_date)?;

        let vol = state
            .get_volatility_surface_element(&index)?
            .surface()
            .volatility_from_date(start_date, effective_strike)?;

        let is_cap = matches!(caplet.option_type(), CapletFloorletType::Caplet);
        let undiscounted =
            BlackClosedFormPricer::black_forward_price(fwd, effective_strike, vol, tau, is_cap);

        let value: ADReal = (df_pay * undiscounted * alpha * trade.notional()).into();
        state.value = Some(value);

        Tape::stop_recording();
        Ok(value.value())
    }
}

impl HandleSensitivities<CapletFloorletTrade, BlackCapletState> for BlackCapletPricer {
    fn handle_sensitivities(
        &self,
        trade: &CapletFloorletTrade,
        state: &mut BlackCapletState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(v) = state.value {
            v
        } else {
            let _ = self.handle_value(trade, state)?;
            state.value.ok_or_else(|| {
                QSError::UnexpectedErr(
                    "State does not contain price after value computation.".into(),
                )
            })?
        };

        value.backward_to_mark()?;

        let caplet = trade.instrument();
        let index = caplet.market_index();
        let policy_discount_index = if let Some(policy) = &self.discount_policy {
            Some(policy.accept(caplet)?)
        } else {
            None
        };
        let discount_index = policy_discount_index.as_ref().unwrap_or(&index);

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        // Discount curve sensitivities
        for (label, pillar) in state
            .get_discount_curve_element(discount_index)?
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
    type Item = CapletFloorletTrade;
    type Policy = dyn DiscountPolicy<CapletFloorlet>;

    fn evaluate(
        &self,
        trade: &CapletFloorletTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let caplet = trade.instrument();
        let identifier = caplet.identifier();

        let md_request = self
            .market_data_request(trade)
            .ok_or_else(|| QSError::InvalidValueErr("Missing market data request".into()))?;

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

    fn market_data_request(&self, trade: &CapletFloorletTrade) -> Option<MarketDataRequest> {
        let index = trade.instrument().market_index();
        let mut elements = vec![
            ConstructedElementRequest::DiscountCurve {
                market_index: index.clone(),
            },
            ConstructedElementRequest::VolatilitySurface {
                market_index: index.clone(),
            },
        ];
        let fixings = Vec::new();

        let mut seen_indices = HashSet::new();
        seen_indices.insert(index);

        if let Some(policy) = &self.discount_policy {
            for policy_index in policy.discount_indices() {
                if seen_indices.insert(policy_index.clone()) {
                    elements.push(ConstructedElementRequest::DiscountCurve {
                        market_index: policy_index,
                    });
                }
            }
        }

        let mut request = MarketDataRequest::default()
            .with_constructed_elements_request(elements)
            .with_fixings_request(fixings);

        if self.discount_policy.is_some() {
            request = request.with_exchange_rates();
        }
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
            trade::Side,
        },
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        instruments::rates::capletfloorlet::{
            CapletFloorlet, CapletFloorletTrade, CapletFloorletType,
        },
        math::probability::norm_cdf::norm_cdf,
        pricers::rates::blackcapletpricer::BlackCapletPricer,
        rates::{
            compounding::Compounding,
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
            volatilityindexing::{F64Key, Strike},
            volatilitysurface::VolatilitySurface,
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
                &[],
            ))
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    /// Builds market data for caplet tests.
    ///
    /// The volatility surface is a 2x2 grid bracketing (start_date, anchor_strike)
    /// so that bilinear interpolation succeeds.
    ///
    /// Returns `(MarketData, discount_curve, vol_surface)`.
    fn setup_caplet_market_data(
        trade_date: Date,
        start_date: Date,
        end_date: Date,
        market_index: &MarketIndex,
        risk_free_rate: f64,
        flat_vol: f64,
        anchor_strike: f64,
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

        let days_to_start = start_date - trade_date;
        let days_before_start = days_to_start / 2;
        let days_after_start = days_to_start + (end_date - start_date);

        let strike_lo = anchor_strike * 0.5;
        let strike_hi = anchor_strike * 1.5;

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
            Period::new(i32::try_from(days_after_start).unwrap_or(0), TimeUnit::Days),
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

        let market_data = MarketData::new(HashMap::new(), constructed_elements, &[]);
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
        let caplet = CapletFloorlet::new(
            "SOFR3M_CAPLET".to_string(),
            market_index,
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Caplet,
            Strike::Absolute(strike),
            rate_def,
        );
        let trade =
            CapletFloorletTrade::new(caplet.clone(), trade_date, notional, Side::LongRecieve);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackCapletPricer::new();
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".to_string()))?;

        // Compute closed-form Black caplet price using curve.forward_rate
        let tau = rate_def.day_counter().year_fraction(trade_date, start_date);
        let alpha = caplet.accrual_factor();
        let df_pay = discount_curve.discount_factor(end_date)?.value();
        let fwd = discount_curve
            .forward_rate(
                start_date,
                end_date,
                Compounding::Simple,
                rate_def.frequency(),
            )?
            .value();
        let vol = vol_surface
            .borrow()
            .volatility_from_date(start_date, strike)?
            .value();
        let vol_sqrt_tau = vol * tau.sqrt();
        let d1 = ((fwd / strike).ln() + 0.5 * vol * vol * tau) / vol_sqrt_tau;
        let d2 = d1 - vol_sqrt_tau;
        let closed_form = notional * alpha * df_pay * (fwd * norm_cdf(d1) - strike * norm_cdf(d2));

        println!("Pricer price: {price}");
        println!("Closed-form price: {closed_form}");
        assert!((price - closed_form).abs() < 1e-4);

        Ok(())
    }

    /// Verifies that the AD-computed sensitivities match:
    ///   - **Vega** (closed-form): `N · α · df_pay · F · φ(d1) · √τ`
    ///     (sum of all vol-pillar adjoints equals the total Black vega because the
    ///     bilinear interpolation weights sum to 1 at any interior grid point)
    ///   - **Rate sensitivity** (bump-and-reprice): finite-difference derivative
    ///     of the price with respect to the flat discount rate `r`.
    #[test]
    fn black_caplet_sensitivities_match_closed_form() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let start_date = trade_date + Period::new(6, TimeUnit::Months);
        let end_date = start_date + Period::new(3, TimeUnit::Months);
        let market_index = MarketIndex::TermSOFR3m;

        let notional = 1.0; // unit notional for easy comparison
        let strike = 0.05;
        let risk_free_rate = 0.04;
        let flat_vol = 0.20;
        let rate_def = RateDefinition::default();

        // Price + AD sensitivities
        let (market_data, discount_curve, vol_surface) = setup_caplet_market_data(
            trade_date,
            start_date,
            end_date,
            &market_index,
            risk_free_rate,
            flat_vol,
            strike,
        )?;

        let caplet = CapletFloorlet::new(
            "SOFR3M_CAPLET".to_string(),
            market_index.clone(),
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Caplet,
            Strike::Absolute(strike),
            rate_def,
        );
        let trade =
            CapletFloorletTrade::new(caplet.clone(), trade_date, notional, Side::LongRecieve);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackCapletPricer::new();
        let results =
            pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)?;

        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".to_string()))?;
        let sens = results
            .sensitivities()
            .ok_or_else(|| QSError::UnexpectedErr("Missing sensitivities".to_string()))?;
        let tau = rate_def.day_counter().year_fraction(trade_date, start_date);
        let alpha = caplet.accrual_factor();
        let df_pay = discount_curve.discount_factor(end_date)?.value();
        let fwd = discount_curve
            .forward_rate(
                start_date,
                end_date,
                Compounding::Simple,
                rate_def.frequency(),
            )?
            .value();
        let vol = vol_surface
            .borrow()
            .volatility_from_date(start_date, strike)?
            .value();
        let vol_sqrt_tau = vol * tau.sqrt();
        let d1 = ((fwd / strike).ln() + 0.5 * vol * vol * tau) / vol_sqrt_tau;

        // φ(d1) = standard-normal PDF
        let phi_d1 = (-0.5 * d1 * d1).exp() / (2.0 * std::f64::consts::PI).sqrt();
        let closed_form_vega = notional * alpha * df_pay * fwd * phi_d1 * tau.sqrt();

        // AD vega = sum over all vol-pillar adjoints
        // (bilinear weights sum to 1, so the aggregated adjoint equals total vega)
        let ad_vega_total: f64 = ["vol_t0_lo", "vol_t0_hi", "vol_t1_lo", "vol_t1_hi"]
            .iter()
            .filter_map(|&k| {
                sens.instrument_keys()
                    .iter()
                    .zip(sens.exposure().iter().copied())
                    .find(|(key, _)| key.as_str() == k)
                    .map(|(_, v)| v)
            })
            .sum();

        println!("Closed-form vega: {closed_form_vega:.8}");
        println!("AD vega (sum):    {ad_vega_total:.8}");
        assert!(
            (ad_vega_total - closed_form_vega).abs() < 1e-8,
            "vega mismatch: ad={ad_vega_total}, cf={closed_form_vega}"
        );

        // ── Bump-and-reprice rate sensitivity ─────────────────────────────────
        let bump = 1e-5_f64;

        let (md_up, _, _) = setup_caplet_market_data(
            trade_date,
            start_date,
            end_date,
            &market_index,
            risk_free_rate + bump,
            flat_vol,
            strike,
        )?;
        let (md_dn, _, _) = setup_caplet_market_data(
            trade_date,
            start_date,
            end_date,
            &market_index,
            risk_free_rate - bump,
            flat_vol,
            strike,
        )?;

        let price_up = pricer
            .evaluate(
                &trade,
                &[Request::Value],
                &SimpleMarketDataProvider {
                    evaluation_date: trade_date,
                    market_data: md_up,
                },
            )?
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price_up".to_string()))?;

        let price_dn = pricer
            .evaluate(
                &trade,
                &[Request::Value],
                &SimpleMarketDataProvider {
                    evaluation_date: trade_date,
                    market_data: md_dn,
                },
            )?
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price_dn".to_string()))?;

        let fd_rate_sens = (price_up - price_dn) / (2.0 * bump);

        let ad_rate_sens = sens
            .instrument_keys()
            .iter()
            .zip(sens.exposure().iter().copied())
            .find(|(k, _)| k.as_str() == "discount_rate")
            .map(|(_, v)| v)
            .ok_or_else(|| {
                QSError::NotFoundErr("discount_rate sensitivity not found".to_string())
            })?;

        println!("FD rate sensitivity: {fd_rate_sens:.8}");
        println!("AD rate sensitivity: {ad_rate_sens:.8}");
        assert!(
            (ad_rate_sens - fd_rate_sens).abs() < 1e-5,
            "rate sensitivity mismatch: ad={ad_rate_sens}, fd={fd_rate_sens}"
        );

        // Sanity: price is positive and sensitivities were found
        assert!(price > 0.0);

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

        let caplet = CapletFloorlet::new(
            "SOFR3M_CAPLET".to_string(),
            market_index,
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Caplet,
            Strike::Absolute(strike),
            RateDefinition::default(),
        );
        let trade = CapletFloorletTrade::new(caplet, trade_date, notional, Side::LongRecieve);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackCapletPricer::new();
        let results =
            pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)?;

        assert!(results.price().is_some());
        let sens = results
            .sensitivities()
            .ok_or_else(|| QSError::UnexpectedErr("Missing sensitivities".to_string()))?;
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

        let caplet = CapletFloorlet::new(
            "SOFR3M_FLOORLET".to_string(),
            market_index,
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Floorlet,
            Strike::Absolute(strike),
            RateDefinition::default(),
        );
        let trade = CapletFloorletTrade::new(caplet, trade_date, notional, Side::LongRecieve);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackCapletPricer::new();
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".to_string()))?;

        println!("Floorlet price: {price}");
        assert!(price > 0.0);

        Ok(())
    }

    #[test]
    fn black_caplet_atm_strike_prices_positive() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let start_date = trade_date + Period::new(6, TimeUnit::Months);
        let end_date = start_date + Period::new(3, TimeUnit::Months);
        let market_index = MarketIndex::TermSOFR3m;

        let notional = 1_000_000.0;
        let anchor_strike = 0.04; // approximately the forward rate at 4% flat
        let risk_free_rate = 0.04;

        let (market_data, _discount_curve, _vol_surface) = setup_caplet_market_data(
            trade_date,
            start_date,
            end_date,
            &market_index,
            risk_free_rate,
            0.20,
            anchor_strike,
        )?;

        let caplet = CapletFloorlet::new(
            "SOFR3M_CAPLET_ATM".to_string(),
            market_index,
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Caplet,
            Strike::Atm,
            RateDefinition::default(),
        );
        let trade = CapletFloorletTrade::new(caplet, trade_date, notional, Side::LongRecieve);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackCapletPricer::new();
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".to_string()))?;

        println!("ATM caplet price: {price}");
        assert!(price > 0.0);

        Ok(())
    }

    #[test]
    fn black_caplet_relative_strike_prices_positive() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let start_date = trade_date + Period::new(6, TimeUnit::Months);
        let end_date = start_date + Period::new(3, TimeUnit::Months);
        let market_index = MarketIndex::TermSOFR3m;

        let notional = 1_000_000.0;
        // Spread over forward: effective strike = F + 0.01
        // Forward is ~0.04, so effective strike ~0.05 which is in our grid
        let spread = 0.01_f64;
        let anchor_strike = 0.05; // centre the vol grid around the expected effective strike

        let (market_data, _discount_curve, _vol_surface) = setup_caplet_market_data(
            trade_date,
            start_date,
            end_date,
            &market_index,
            0.04,
            0.20,
            anchor_strike,
        )?;

        let caplet = CapletFloorlet::new(
            "SOFR3M_CAPLET_REL".to_string(),
            market_index,
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Caplet,
            Strike::Relative(spread),
            RateDefinition::default(),
        );
        let trade = CapletFloorletTrade::new(caplet, trade_date, notional, Side::LongRecieve);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = BlackCapletPricer::new();
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".to_string()))?;

        println!("Relative-strike caplet price: {price}");
        assert!(price > 0.0);

        Ok(())
    }
}

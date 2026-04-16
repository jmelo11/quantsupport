use std::collections::HashSet;

use crate::{
    ad::{dual::DualFwd, tape::Tape},
    core::{
        collateral::{DiscountPolicy, Discountable},
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::{AssetClass, Instrument},
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            fxrequest::FxRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pillars::Pillars,
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    currencies::currency::Currency,
    instruments::fx::fxoption::{FxOptionTrade, FxOptionType},
    models::brownianmotion::BrownianMotion,
    utils::errors::{QSError, Result},
};

/// State struct for storing intermediate values during FX option pricing.
#[derive(Default)]
struct FxOptionState {
    value: Option<DualFwd>,
    market_data: Option<MarketData>,
}

impl PricerState for FxOptionState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

/// Lightweight [`Discountable`] wrapper used to resolve a discount curve
/// for a specific currency through the discount policy.
struct CurrencyDiscountable {
    currency: Currency,
}

impl Discountable for CurrencyDiscountable {
    fn asset_class(&self) -> AssetClass {
        AssetClass::Fx
    }

    fn currency(&self) -> Currency {
        self.currency
    }
}

/// A pricer for European FX options using the Garman-Kohlhagen model
/// (Black-Scholes adapted for FX).
///
/// The forward rate is computed as
/// `F = S * DF_base(T) / DF_quote(T)`,
/// and the undiscounted price is obtained via the closed-form Black formula.
/// The NPV is then
/// `NPV = DF_quote(T) * BlackPrice(F, K, σ, τ) * N * side`.
///
/// When a [`DiscountPolicy`] is set, the pricer uses the policy-resolved
/// discount curves for both the base and quote currencies.
pub struct FxOptionPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl FxOptionPricer {
    /// Creates a new [`FxOptionPricer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            discount_policy: None,
        }
    }
}

impl Default for FxOptionPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl HandleValue<FxOptionTrade, FxOptionState> for FxOptionPricer {
    fn handle_value(&self, trade: &FxOptionTrade, state: &mut FxOptionState) -> Result<f64> {
        Tape::start_recording_fwd();
        Tape::set_mark_fwd();
        state.put_pillars_on_tape()?;

        let inst = trade.instrument();
        let base = inst.base_currency();
        let quote = inst.quote_currency();

        let policy = self.discount_policy.as_ref().ok_or_else(|| {
            QSError::InvalidValueErr("Discount policy required for FX option pricing".into())
        })?;
        let base_idx = policy.accept(&CurrencyDiscountable { currency: base })?;
        let quote_idx = policy.accept(&CurrencyDiscountable { currency: quote })?;

        let tau = inst
            .day_counter()
            .year_fraction(trade.trade_date(), inst.expiry_date());

        let df_base = state
            .get_discount_curve_element(&base_idx)?
            .curve()
            .discount_factor(inst.expiry_date())?;
        let df_quote = state
            .get_discount_curve_element(&quote_idx)?
            .curve()
            .discount_factor(inst.expiry_date())?;

        let spot = state.get_exchange_rate(base, quote)?;
        // Garman-Kohlhagen forward: F = S * DF_base / DF_quote
        let forward: DualFwd = (spot * df_base / df_quote).into();

        let strike = inst.strike().resolve(forward.value());
        let vol = state
            .get_volatility_surface_element(inst.underlying_index())?
            .surface()
            .volatility_from_date(inst.expiry_date(), strike)?;

        let is_call = matches!(inst.option_type(), FxOptionType::Call);
        let undiscounted =
            BrownianMotion::<DualFwd>::closed_form_price(forward, strike, vol, tau, is_call)?;

        let notional = DualFwd::new(trade.notional());
        let side = DualFwd::new(trade.side().sign());
        let npv: DualFwd = (df_quote * undiscounted * notional * side).into();
        state.value = Some(npv);

        Tape::stop_recording_fwd();
        Ok(npv.value())
    }
}

impl HandleSensitivities<FxOptionTrade, FxOptionState> for FxOptionPricer {
    fn handle_sensitivities(
        &self,
        trade: &FxOptionTrade,
        state: &mut FxOptionState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(v) = state.value {
            v
        } else {
            let _ = self.handle_value(trade, state)?;
            state
                .value
                .ok_or_else(|| QSError::UnexpectedErr("Missing value in FX option state".into()))?
        };

        value.backward_to_mark()?;

        let inst = trade.instrument();
        let policy = self.discount_policy.as_ref().ok_or_else(|| {
            QSError::InvalidValueErr("Discount policy required for FX option pricing".into())
        })?;
        let base_idx = policy.accept(&CurrencyDiscountable {
            currency: inst.base_currency(),
        })?;
        let quote_idx = policy.accept(&CurrencyDiscountable {
            currency: inst.quote_currency(),
        })?;

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        // Discount curve sensitivities
        for idx in [base_idx, quote_idx] {
            let element = state.get_discount_curve_element(&idx)?;
            for (label, value) in element.curve().pillars().into_iter().flatten() {
                ids.push(label);
                exposures.push(value.adjoint().map_or(0.0, |a| a.value()));
            }
        }

        // Volatility surface sensitivities
        for (label, pillar) in state
            .get_volatility_surface_element(inst.underlying_index())?
            .surface()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?.value());
        }

        // FX spot sensitivities
        if let Some(store) = state.get_fx_store() {
            for (label, value) in store.pillars().into_iter().flatten() {
                ids.push(label);
                exposures.push(value.adjoint().map_or(0.0, |a| a.value()));
            }
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures)
            .aggregate())
    }
}

impl Pricer for FxOptionPricer {
    type Item = FxOptionTrade;
    type Policy = dyn DiscountPolicy;

    fn evaluate(
        &self,
        trade: &FxOptionTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let identifier = trade.instrument().identifier();
        let md_request = self.market_data_request(trade).ok_or_else(|| {
            QSError::InvalidValueErr("Missing market-data request for FX option".into())
        })?;

        let mut state = FxOptionState {
            value: None,
            market_data: Some(ctx.handle_request(&md_request)?),
        };

        let mut out = EvaluationResults::new(eval_date, identifier);
        for req in requests {
            match req {
                Request::Value => out = out.with_price(self.handle_value(trade, &mut state)?),
                Request::Sensitivities => {
                    out = out.with_sensitivities(self.handle_sensitivities(trade, &mut state)?);
                }
                _ => {}
            }
        }

        Ok(out)
    }

    fn market_data_request(&self, trade: &FxOptionTrade) -> Option<MarketDataRequest> {
        let policy = self.discount_policy.as_ref()?;
        let inst = trade.instrument();
        let mut elements = Vec::new();
        let mut seen_indices = HashSet::new();

        for ccy in [inst.base_currency(), inst.quote_currency()] {
            if let Ok(idx) = policy.accept(&CurrencyDiscountable { currency: ccy }) {
                if seen_indices.insert(idx.clone()) {
                    elements.push(ConstructedElementRequest::DiscountCurve { market_index: idx });
                }
            }
        }

        elements.push(ConstructedElementRequest::VolatilitySurface {
            market_index: inst.underlying_index().clone(),
        });

        let mut request = MarketDataRequest::default().with_fx_request(vec![FxRequest::pair(
            inst.base_currency(),
            inst.quote_currency(),
        )]);

        if !elements.is_empty() {
            request = request.with_constructed_elements_request(elements);
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
        ad::dual::DualFwd,
        core::{
            collateral::DiscountPolicy,
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
        instruments::fx::fxoption::{FxOption, FxOptionTrade, FxOptionType},
        pricers::fx::fxoptionpricer::FxOptionPricer,
        quotes::fxstore::FxStore,
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
        utils::errors::{QSError, Result},
        volatility::{
            interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
            volatilityindexing::{F64Key, SmileType, Strike, VolatilityType},
        },
    };

    /// A simple discount policy that maps each currency to a named market index.
    struct FxDiscountPolicy {
        base_index: MarketIndex,
        base_currency: Currency,
        quote_index: MarketIndex,
        quote_currency: Currency,
    }

    impl DiscountPolicy for FxDiscountPolicy {
        fn accept(
            &self,
            target: &dyn crate::core::collateral::Discountable,
        ) -> Result<MarketIndex> {
            if target.currency() == self.base_currency {
                Ok(self.base_index.clone())
            } else if target.currency() == self.quote_currency {
                Ok(self.quote_index.clone())
            } else {
                Err(QSError::InvalidValueErr(format!(
                    "Unsupported currency: {}",
                    target.currency()
                )))
            }
        }

        fn discount_indices(&self) -> Vec<MarketIndex> {
            vec![self.base_index.clone(), self.quote_index.clone()]
        }
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
            )
            .with_fx_store(self.market_data.fx_store().cloned().unwrap_or_default()))
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    /// Build market data for an FX option test.
    #[allow(clippy::too_many_arguments)]
    fn setup_fx_option_market_data(
        trade_date: Date,
        expiry_date: Date,
        base_index: &MarketIndex,
        quote_index: &MarketIndex,
        underlying_index: &MarketIndex,
        spot: f64,
        base_rate: f64,
        quote_rate: f64,
        base_ccy: Currency,
        quote_ccy: Currency,
    ) -> Result<MarketData> {
        let days_to_expiry = expiry_date - trade_date;

        let base_curve = FlatForwardTermStructure::new(
            trade_date,
            DualFwd::from(base_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("base_rate".to_string());

        let quote_curve = FlatForwardTermStructure::new(
            trade_date,
            DualFwd::from(quote_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("quote_rate".to_string());

        // Flat vol surface across two tenors
        let mut surface_points = BTreeMap::new();
        let strikes: Vec<(F64Key, DualFwd)> = vec![
            (F64Key::new(0.90), DualFwd::from(0.10)),
            (F64Key::new(1.00), DualFwd::from(0.10)),
            (F64Key::new(1.10), DualFwd::from(0.10)),
            (F64Key::new(1.20), DualFwd::from(0.10)),
            (F64Key::new(1.30), DualFwd::from(0.10)),
        ];

        surface_points.insert(
            Period::new(days_to_expiry as i32, TimeUnit::Days),
            strikes.iter().cloned().collect(),
        );
        surface_points.insert(
            Period::new(days_to_expiry as i32 + 365, TimeUnit::Days),
            strikes.iter().cloned().collect(),
        );

        let labels: Vec<String> = ["6m", "18m"]
            .iter()
            .flat_map(|t| {
                [0.90, 1.00, 1.10, 1.20, 1.30]
                    .iter()
                    .map(move |k| format!("vol_{t}_{k}"))
            })
            .collect();

        let vol_surface = Rc::new(RefCell::new(
            InterpolatedVolatilitySurface::new(
                trade_date,
                underlying_index.clone(),
                surface_points,
                VolatilityType::Black,
                SmileType::Strike,
            )
            .with_labels(&labels),
        ));

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            base_index.clone(),
            DiscountCurveElement::new(base_index.clone(), Rc::new(RefCell::new(base_curve))),
        );
        constructed_elements.discount_curves_mut().insert(
            quote_index.clone(),
            DiscountCurveElement::new(quote_index.clone(), Rc::new(RefCell::new(quote_curve))),
        );
        constructed_elements.volatility_surfaces_mut().insert(
            underlying_index.clone(),
            VolatilitySurfaceElement::new(underlying_index.clone(), vol_surface),
        );

        let mut fx_store = FxStore::new();
        fx_store.add_fx_rate(base_ccy, quote_ccy, DualFwd::new(spot));

        let market_data =
            MarketData::new(HashMap::new(), constructed_elements).with_fx_store(fx_store);

        Ok(market_data)
    }

    #[test]
    fn fx_option_call_price_is_positive() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let base_ccy = Currency::EUR;
        let quote_ccy = Currency::USD;
        let base_index = MarketIndex::Other("EUR_DISC".to_string());
        let quote_index = MarketIndex::Other("USD_DISC".to_string());
        let underlying_index = MarketIndex::Other("EURUSD".to_string());

        let spot = 1.10;
        let strike = 1.12;
        let notional = 1_000_000.0;
        let base_rate = 0.03;
        let quote_rate = 0.05;

        let market_data = setup_fx_option_market_data(
            trade_date,
            expiry_date,
            &base_index,
            &quote_index,
            &underlying_index,
            spot,
            base_rate,
            quote_rate,
            base_ccy,
            quote_ccy,
        )?;

        let option = FxOption::new(
            "EURUSD-CALL".to_string(),
            expiry_date,
            Strike::Absolute(strike),
            FxOptionType::Call,
            base_ccy,
            quote_ccy,
            DayCounter::Actual360,
            underlying_index,
        );
        let trade = FxOptionTrade::new(option, trade_date, notional, Side::LongReceive);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let mut pricer = FxOptionPricer::new();
        pricer.set_discount_policy(Box::new(FxDiscountPolicy {
            base_index,
            base_currency: base_ccy,
            quote_index,
            quote_currency: quote_ccy,
        }));

        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;

        assert!(
            price > 0.0,
            "Call option price should be positive, got {price}"
        );
        Ok(())
    }

    #[test]
    fn fx_option_put_price_is_positive() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let base_ccy = Currency::EUR;
        let quote_ccy = Currency::USD;
        let base_index = MarketIndex::Other("EUR_DISC".to_string());
        let quote_index = MarketIndex::Other("USD_DISC".to_string());
        let underlying_index = MarketIndex::Other("EURUSD".to_string());

        let spot = 1.10;
        let strike = 1.08;
        let notional = 1_000_000.0;

        let market_data = setup_fx_option_market_data(
            trade_date,
            expiry_date,
            &base_index,
            &quote_index,
            &underlying_index,
            spot,
            0.03,
            0.05,
            base_ccy,
            quote_ccy,
        )?;

        let option = FxOption::new(
            "EURUSD-PUT".to_string(),
            expiry_date,
            Strike::Absolute(strike),
            FxOptionType::Put,
            base_ccy,
            quote_ccy,
            DayCounter::Actual360,
            underlying_index,
        );
        let trade = FxOptionTrade::new(option, trade_date, notional, Side::LongReceive);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let mut pricer = FxOptionPricer::new();
        pricer.set_discount_policy(Box::new(FxDiscountPolicy {
            base_index,
            base_currency: base_ccy,
            quote_index,
            quote_currency: quote_ccy,
        }));

        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;

        assert!(
            price > 0.0,
            "Put option price should be positive, got {price}"
        );
        Ok(())
    }

    #[test]
    fn fx_option_put_call_parity() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let base_ccy = Currency::EUR;
        let quote_ccy = Currency::USD;
        let base_index = MarketIndex::Other("EUR_DISC".to_string());
        let quote_index = MarketIndex::Other("USD_DISC".to_string());
        let underlying_index = MarketIndex::Other("EURUSD".to_string());

        let spot = 1.10;
        let strike = 1.12;
        let notional = 1.0;
        let base_rate = 0.03;
        let quote_rate = 0.05;

        // Price call
        let md_call = setup_fx_option_market_data(
            trade_date,
            expiry_date,
            &base_index,
            &quote_index,
            &underlying_index,
            spot,
            base_rate,
            quote_rate,
            base_ccy,
            quote_ccy,
        )?;

        let call = FxOption::new(
            "EURUSD-CALL".to_string(),
            expiry_date,
            Strike::Absolute(strike),
            FxOptionType::Call,
            base_ccy,
            quote_ccy,
            DayCounter::Actual360,
            underlying_index.clone(),
        );
        let call_trade = FxOptionTrade::new(call, trade_date, notional, Side::LongReceive);

        let call_provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data: md_call,
        };

        let mut pricer = FxOptionPricer::new();
        pricer.set_discount_policy(Box::new(FxDiscountPolicy {
            base_index: base_index.clone(),
            base_currency: base_ccy,
            quote_index: quote_index.clone(),
            quote_currency: quote_ccy,
        }));

        let call_price = pricer
            .evaluate(&call_trade, &[Request::Value], &call_provider)?
            .price()
            .unwrap();

        // Price put
        let md_put = setup_fx_option_market_data(
            trade_date,
            expiry_date,
            &base_index,
            &quote_index,
            &underlying_index,
            spot,
            base_rate,
            quote_rate,
            base_ccy,
            quote_ccy,
        )?;

        let put = FxOption::new(
            "EURUSD-PUT".to_string(),
            expiry_date,
            Strike::Absolute(strike),
            FxOptionType::Put,
            base_ccy,
            quote_ccy,
            DayCounter::Actual360,
            underlying_index.clone(),
        );
        let put_trade = FxOptionTrade::new(put, trade_date, notional, Side::LongReceive);

        let put_provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data: md_put,
        };

        let put_price = pricer
            .evaluate(&put_trade, &[Request::Value], &put_provider)?
            .price()
            .unwrap();

        // Put-call parity: C - P = DF_quote * (F - K)
        // where F = S * DF_base / DF_quote
        // Use simple compounding (ACT/360) to match FlatForwardTermStructure default
        let tau = DayCounter::Actual360.year_fraction(trade_date, expiry_date);
        let df_base = 1.0 / (1.0 + base_rate * tau);
        let df_quote = 1.0 / (1.0 + quote_rate * tau);
        let forward = spot * df_base / df_quote;
        let expected_diff = df_quote * (forward - strike);

        let actual_diff = call_price - put_price;
        assert!(
            (actual_diff - expected_diff).abs() < 1e-8,
            "Put-call parity violation: C-P={actual_diff}, DF*(F-K)={expected_diff}"
        );

        Ok(())
    }

    #[test]
    fn fx_option_sensitivities_are_computed() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let base_ccy = Currency::EUR;
        let quote_ccy = Currency::USD;
        let base_index = MarketIndex::Other("EUR_DISC".to_string());
        let quote_index = MarketIndex::Other("USD_DISC".to_string());
        let underlying_index = MarketIndex::Other("EURUSD".to_string());

        let spot = 1.10;
        let strike = 1.10;
        let notional = 1_000_000.0;

        let market_data = setup_fx_option_market_data(
            trade_date,
            expiry_date,
            &base_index,
            &quote_index,
            &underlying_index,
            spot,
            0.03,
            0.05,
            base_ccy,
            quote_ccy,
        )?;

        let option = FxOption::new(
            "EURUSD-CALL".to_string(),
            expiry_date,
            Strike::Absolute(strike),
            FxOptionType::Call,
            base_ccy,
            quote_ccy,
            DayCounter::Actual360,
            underlying_index,
        );
        let trade = FxOptionTrade::new(option, trade_date, notional, Side::LongReceive);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let mut pricer = FxOptionPricer::new();
        pricer.set_discount_policy(Box::new(FxDiscountPolicy {
            base_index,
            base_currency: base_ccy,
            quote_index,
            quote_currency: quote_ccy,
        }));

        let results =
            pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)?;

        let sensitivities = results
            .sensitivities()
            .ok_or_else(|| QSError::UnexpectedErr("Missing sensitivities".into()))?;

        // Should have non-empty sensitivities
        assert!(
            !sensitivities.instrument_keys().is_empty(),
            "Sensitivities should not be empty"
        );

        // Should have FX spot sensitivity
        let has_fx_sens = sensitivities
            .instrument_keys()
            .iter()
            .any(|k| k.contains("EUR") && k.contains("USD"));
        assert!(has_fx_sens, "Should have FX spot sensitivity");

        // Should have discount curve sensitivities
        let has_rate_sens = sensitivities
            .instrument_keys()
            .iter()
            .any(|k| k.contains("rate"));
        assert!(has_rate_sens, "Should have discount curve sensitivities");

        // Should have vol sensitivities
        let has_vol_sens = sensitivities
            .instrument_keys()
            .iter()
            .any(|k| k.contains("vol"));
        assert!(has_vol_sens, "Should have volatility sensitivities");

        Ok(())
    }

    #[test]
    fn fx_option_short_side_negates_price() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let expiry_date = trade_date + Period::new(6, TimeUnit::Months);
        let base_ccy = Currency::EUR;
        let quote_ccy = Currency::USD;
        let base_index = MarketIndex::Other("EUR_DISC".to_string());
        let quote_index = MarketIndex::Other("USD_DISC".to_string());
        let underlying_index = MarketIndex::Other("EURUSD".to_string());

        let spot = 1.10;
        let strike = 1.12;
        let notional = 1_000_000.0;

        // Long trade
        let md_long = setup_fx_option_market_data(
            trade_date,
            expiry_date,
            &base_index,
            &quote_index,
            &underlying_index,
            spot,
            0.03,
            0.05,
            base_ccy,
            quote_ccy,
        )?;
        let option_long = FxOption::new(
            "EURUSD-CALL".to_string(),
            expiry_date,
            Strike::Absolute(strike),
            FxOptionType::Call,
            base_ccy,
            quote_ccy,
            DayCounter::Actual360,
            underlying_index.clone(),
        );
        let long_trade = FxOptionTrade::new(option_long, trade_date, notional, Side::LongReceive);

        let mut pricer = FxOptionPricer::new();
        pricer.set_discount_policy(Box::new(FxDiscountPolicy {
            base_index: base_index.clone(),
            base_currency: base_ccy,
            quote_index: quote_index.clone(),
            quote_currency: quote_ccy,
        }));

        let long_price = pricer
            .evaluate(
                &long_trade,
                &[Request::Value],
                &SimpleMarketDataProvider {
                    evaluation_date: trade_date,
                    market_data: md_long,
                },
            )?
            .price()
            .unwrap();

        // Short trade
        let md_short = setup_fx_option_market_data(
            trade_date,
            expiry_date,
            &base_index,
            &quote_index,
            &underlying_index,
            spot,
            0.03,
            0.05,
            base_ccy,
            quote_ccy,
        )?;
        let option_short = FxOption::new(
            "EURUSD-CALL".to_string(),
            expiry_date,
            Strike::Absolute(strike),
            FxOptionType::Call,
            base_ccy,
            quote_ccy,
            DayCounter::Actual360,
            underlying_index,
        );
        let short_trade = FxOptionTrade::new(option_short, trade_date, notional, Side::PayShort);

        let short_price = pricer
            .evaluate(
                &short_trade,
                &[Request::Value],
                &SimpleMarketDataProvider {
                    evaluation_date: trade_date,
                    market_data: md_short,
                },
            )?
            .price()
            .unwrap();

        assert!(
            (long_price + short_price).abs() < 1e-8,
            "Long + Short should be zero, got {long_price} + {short_price}"
        );

        Ok(())
    }
}

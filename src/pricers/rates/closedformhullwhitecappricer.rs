use crate::{
    ad::{dual::DualFwd, expr::FloatExt, tape::Tape},
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
    instruments::rates::{capfloor::CapFloorTrade, capletfloorlet::CapletFloorletType},
    math::probability::norm_cdf::norm_cdf,
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
};
use std::collections::HashSet;

/// Prices a cap or floor strip using the Hull-White one-factor model.
///
/// Each constituent caplet/floorlet is priced via the Jamshidian
/// bond-option decomposition and the results are summed.
/// See [`super::closedformhullwhitecapletpricer::ClosedFormHullWhiteCapletPricer`]
/// for the single-period formula.
///
/// The model parameters `alpha` (mean-reversion) and `sigma` (volatility)
/// are provided at construction time.
pub struct ClosedFormHullWhiteCapPricer {
    alpha: f64,
    sigma: f64,
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl ClosedFormHullWhiteCapPricer {
    /// Creates a new [`ClosedFormHullWhiteCapPricer`] with the given
    /// Hull-White parameters.
    #[must_use]
    pub fn new(alpha: f64, sigma: f64) -> Self {
        Self {
            alpha,
            sigma,
            discount_policy: None,
        }
    }

    /// Computes $B(t,T) = (1 - e^{-\alpha(T-t)}) / \alpha$.
    #[allow(non_snake_case)]
    fn B(&self, t: f64, big_t: f64) -> f64 {
        (1.0 - (-self.alpha * (big_t - t)).exp()) / self.alpha
    }

    /// ZCB price volatility.
    fn zcb_price_volatility(&self, t: f64, big_t: f64) -> f64 {
        let b = self.B(t, big_t);
        self.sigma * b * ((1.0 - (-2.0 * self.alpha * t).exp()) / (2.0 * self.alpha)).sqrt()
    }
}

/// State for Hull-White cap/floor pricing.
#[derive(Default)]
struct HullWhiteCapState {
    value: Option<DualFwd>,
    market_data: Option<MarketData>,
}

impl PricerState for HullWhiteCapState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl HandleValue<CapFloorTrade, HullWhiteCapState> for ClosedFormHullWhiteCapPricer {
    fn handle_value(&self, trade: &CapFloorTrade, state: &mut HullWhiteCapState) -> Result<f64> {
        let cap = trade.instrument();
        let caplets = cap.caplet_floorlets();
        let fwd_index = cap.market_index();
        let disc_index = if let Some(policy) = &self.discount_policy {
            policy.accept(cap)?
        } else {
            fwd_index.clone()
        };
        let rate_def = fwd_index.rate_index_details()?.rate_definition();
        let dc = rate_def.day_counter();
        let dual_curve = disc_index != fwd_index;

        Tape::start_recording_fwd();
        Tape::set_mark_fwd();
        state.put_pillars_on_tape()?;

        let mut npv = DualFwd::zero();

        for c in caplets {
            let start_date = c.start_accrual_date();
            let end_date = c.end_accrual_date();
            let payment_date = c.payment_date();

            let t = dc.year_fraction(trade.trade_date(), start_date);
            let big_s = dc.year_fraction(trade.trade_date(), payment_date);
            let tau = dc.year_fraction(start_date, end_date);

            // Forward rate
            let fwd: DualFwd = state
                .get_discount_curve_element(&fwd_index)?
                .curve()
                .forward_rate(start_date, end_date, rate_def.compounding(), rate_def.frequency())?;

            let strike = match c.strike() {
                Strike::Absolute(k) => k,
                Strike::Atm => fwd.value(),
                Strike::Relative(pct) => fwd.value() * (1.0 + pct),
            };

            let x = 1.0 / tau.mul_add(strike, 1.0);
            let sigma_p = self.zcb_price_volatility(t, big_s);

            let df_t: DualFwd = state
                .get_discount_curve_element(&fwd_index)?
                .curve()
                .discount_factor(start_date)?;
            let df_s: DualFwd = state
                .get_discount_curve_element(&fwd_index)?
                .curve()
                .discount_factor(payment_date)?;

            let d1: DualFwd =
                (((df_s / (DualFwd::new(x) * df_t)).ln() + DualFwd::new(0.5 * sigma_p * sigma_p))
                    / DualFwd::new(sigma_p))
                .into();
            let d2: DualFwd = (d1 - DualFwd::new(sigma_p)).into();

            let caplet_value: DualFwd = match c.payoff_type() {
                CapletFloorletType::Caplet => {
                    let neg_d2: DualFwd = (DualFwd::new(0.0) - d2).into();
                    let neg_d1: DualFwd = (DualFwd::new(0.0) - d1).into();
                    let put: DualFwd = (DualFwd::new(x) * df_t * norm_cdf(neg_d2)
                        - df_s * norm_cdf(neg_d1))
                        .into();
                    (DualFwd::new(tau.mul_add(strike, 1.0)) * put).into()
                }
                CapletFloorletType::Floorlet => {
                    let call: DualFwd =
                        (df_s * norm_cdf(d1) - DualFwd::new(x) * df_t * norm_cdf(d2)).into();
                    (DualFwd::new(tau.mul_add(strike, 1.0)) * call).into()
                }
            };

            let adjusted: DualFwd = if dual_curve {
                let df_disc_s: DualFwd = state
                    .get_discount_curve_element(&disc_index)?
                    .curve()
                    .discount_factor(payment_date)?;
                (caplet_value * df_disc_s / df_s).into()
            } else {
                caplet_value
            };

            npv = (npv + adjusted * DualFwd::new(trade.notional())).into();
        }

        state.value = Some(npv);
        Tape::stop_recording_fwd();
        Ok(npv.value())
    }
}

impl HandleSensitivities<CapFloorTrade, HullWhiteCapState> for ClosedFormHullWhiteCapPricer {
    fn handle_sensitivities(
        &self,
        trade: &CapFloorTrade,
        state: &mut HullWhiteCapState,
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

        let cap = trade.instrument();
        let fwd_index = cap.market_index();
        let policy_discount_index = if let Some(policy) = &self.discount_policy {
            Some(policy.accept(cap)?)
        } else {
            None
        };
        let discount_index = policy_discount_index.as_ref().unwrap_or(&fwd_index);

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        for (label, pillar) in state
            .get_discount_curve_element(discount_index)?
            .curve()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?.value());
        }

        if &fwd_index != discount_index {
            for (label, pillar) in state
                .get_discount_curve_element(&fwd_index)?
                .curve()
                .pillars()
                .unwrap_or_default()
            {
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

impl Pricer for ClosedFormHullWhiteCapPricer {
    type Item = CapFloorTrade;
    type Policy = dyn DiscountPolicy;

    fn evaluate(
        &self,
        trade: &CapFloorTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let cap = trade.instrument();
        let identifier = cap.identifier();

        let md_request = self
            .market_data_request(trade)
            .ok_or_else(|| QSError::InvalidValueErr("Missing market data request".into()))?;

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = HullWhiteCapState {
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

    fn market_data_request(&self, trade: &CapFloorTrade) -> Option<MarketDataRequest> {
        let index = trade.instrument().market_index();
        let mut elements = vec![ConstructedElementRequest::DiscountCurve {
            market_index: index.clone(),
        }];
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
        collections::HashMap,
        rc::Rc,
    };

    use crate::{
        ad::dual::DualFwd,
        core::{
            elements::curveelement::DiscountCurveElement,
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
        instruments::rates::{
            capfloor::{CapFloor, CapFloorTrade, CapFloorType},
            capletfloorlet::{CapletFloorlet, CapletFloorletType},
        },
        models::hullwhite::hullwhitemodel::HullWhite,
        pricers::rates::closedformhullwhitecappricer::ClosedFormHullWhiteCapPricer,
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::{date::Date, daycounter::DayCounter, enums::TimeUnit, period::Period},
        utils::errors::{QSError, Result},
        volatility::volatilityindexing::Strike,
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

    fn setup_hw_cap_market_data(
        trade_date: Date,
        market_index: &MarketIndex,
        risk_free_rate: f64,
    ) -> MarketData {
        let discount_curve = FlatForwardTermStructure::new(
            trade_date,
            DualFwd::from(risk_free_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("discount_rate".to_string());

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            market_index.clone(),
            DiscountCurveElement::new(
                market_index.clone(),
                Rc::new(RefCell::new(discount_curve)),
            ),
        );

        MarketData::new(HashMap::new(), constructed_elements)
    }

    fn flat_f64_curve(ref_date: Date, rate: f64) -> FlatForwardTermStructure<f64> {
        FlatForwardTermStructure::new(ref_date, rate, RateDefinition::default())
    }

    #[test]
    fn hw_cap_price_equals_sum_of_caplet_prices() -> Result<()> {
        let alpha = 0.1_f64;
        let sigma = 0.01;
        let r = 0.04;
        let strike = 0.05;
        let notional = 1_000_000.0;

        let trade_date = Date::new(2025, 1, 2);
        let market_index = MarketIndex::TermSOFR3m;
        let dc = DayCounter::Actual360;

        // Build a 4-period caplet strip (quarterly, 1Y–2Y)
        let mut caplets = Vec::new();
        let mut date = trade_date + Period::new(1, TimeUnit::Years);
        for i in 0..4 {
            let start = date;
            let end = start + Period::new(3, TimeUnit::Months);
            caplets.push(CapletFloorlet::new(
                format!("CAPLET_{i}"),
                market_index.clone(),
                Currency::USD,
                start,
                start,
                end,
                end,
                CapletFloorletType::Caplet,
                Strike::Absolute(strike),
            ));
            date = end;
        }

        let start_date = trade_date + Period::new(1, TimeUnit::Years);
        let end_date = date;
        let cap = CapFloor::new(
            "HW_CAP".to_string(),
            caplets.clone(),
            market_index.clone(),
            Currency::USD,
            start_date,
            end_date,
            CapFloorType::Cap,
            Strike::Absolute(strike),
        );
        let trade = CapFloorTrade::new(cap, trade_date, notional, Side::LongReceive);

        let market_data = setup_hw_cap_market_data(trade_date, &market_index, r);
        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = ClosedFormHullWhiteCapPricer::new(alpha, sigma);
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let cap_price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;

        // Sum of individual HW caplet prices
        let f64_curve = flat_f64_curve(trade_date, r);
        let hw = HullWhite::new(alpha, &f64_curve);
        let mut expected = 0.0;
        for c in &caplets {
            let t = dc.year_fraction(trade_date, c.start_accrual_date());
            let big_s = dc.year_fraction(trade_date, c.payment_date());
            expected += hw.caplet_price(strike, t, big_s, sigma, &f64_curve)?;
        }
        expected *= notional;

        println!("Cap pricer price:         {cap_price}");
        println!("Sum of HW caplet prices:  {expected}");
        assert!(
            (cap_price - expected).abs() < 1e-4,
            "cap price mismatch: pricer={cap_price}, sum={expected}"
        );
        Ok(())
    }

    #[test]
    fn hw_floor_price_equals_sum_of_floorlet_prices() -> Result<()> {
        let alpha = 0.1_f64;
        let sigma = 0.01;
        let r = 0.04;
        let strike = 0.03;
        let notional = 1_000_000.0;

        let trade_date = Date::new(2025, 1, 2);
        let market_index = MarketIndex::TermSOFR3m;
        let dc = DayCounter::Actual360;

        let mut floorlets = Vec::new();
        let mut date = trade_date + Period::new(1, TimeUnit::Years);
        for i in 0..4 {
            let start = date;
            let end = start + Period::new(3, TimeUnit::Months);
            floorlets.push(CapletFloorlet::new(
                format!("FLOORLET_{i}"),
                market_index.clone(),
                Currency::USD,
                start,
                start,
                end,
                end,
                CapletFloorletType::Floorlet,
                Strike::Absolute(strike),
            ));
            date = end;
        }

        let start_date = trade_date + Period::new(1, TimeUnit::Years);
        let end_date = date;
        let floor = CapFloor::new(
            "HW_FLOOR".to_string(),
            floorlets.clone(),
            market_index.clone(),
            Currency::USD,
            start_date,
            end_date,
            CapFloorType::Floor,
            Strike::Absolute(strike),
        );
        let trade = CapFloorTrade::new(floor, trade_date, notional, Side::LongReceive);

        let market_data = setup_hw_cap_market_data(trade_date, &market_index, r);
        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = ClosedFormHullWhiteCapPricer::new(alpha, sigma);
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let floor_price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;

        let f64_curve = flat_f64_curve(trade_date, r);
        let hw = HullWhite::new(alpha, &f64_curve);
        let mut expected = 0.0;
        for c in &floorlets {
            let t = dc.year_fraction(trade_date, c.start_accrual_date());
            let big_s = dc.year_fraction(trade_date, c.payment_date());
            expected += hw.floorlet_price(strike, t, big_s, sigma, &f64_curve)?;
        }
        expected *= notional;

        println!("Floor pricer price:         {floor_price}");
        println!("Sum of HW floorlet prices:  {expected}");
        assert!(
            (floor_price - expected).abs() < 1e-4,
            "floor price mismatch: pricer={floor_price}, sum={expected}"
        );
        Ok(())
    }

    #[test]
    fn hw_cap_sensitivities_non_empty() -> Result<()> {
        let alpha = 0.1_f64;
        let sigma = 0.01;
        let r = 0.04;
        let strike = 0.05;
        let notional = 1_000_000.0;

        let trade_date = Date::new(2025, 1, 2);
        let market_index = MarketIndex::TermSOFR3m;

        let mut caplets = Vec::new();
        let mut date = trade_date + Period::new(1, TimeUnit::Years);
        for i in 0..4 {
            let start = date;
            let end = start + Period::new(3, TimeUnit::Months);
            caplets.push(CapletFloorlet::new(
                format!("CAPLET_{i}"),
                market_index.clone(),
                Currency::USD,
                start,
                start,
                end,
                end,
                CapletFloorletType::Caplet,
                Strike::Absolute(strike),
            ));
            date = end;
        }

        let start_date = trade_date + Period::new(1, TimeUnit::Years);
        let end_date = date;
        let cap = CapFloor::new(
            "HW_CAP".to_string(),
            caplets,
            market_index.clone(),
            Currency::USD,
            start_date,
            end_date,
            CapFloorType::Cap,
            Strike::Absolute(strike),
        );
        let trade = CapFloorTrade::new(cap, trade_date, notional, Side::LongReceive);

        let market_data = setup_hw_cap_market_data(trade_date, &market_index, r);
        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = ClosedFormHullWhiteCapPricer::new(alpha, sigma);
        let results = pricer.evaluate(
            &trade,
            &[Request::Value, Request::Sensitivities],
            &provider,
        )?;

        let price = results.price().unwrap();
        assert!(price > 0.0, "cap price should be positive");

        let sens = results
            .sensitivities()
            .ok_or_else(|| QSError::UnexpectedErr("Missing sensitivities".into()))?;
        assert!(
            !sens.instrument_keys().is_empty(),
            "sensitivities should not be empty"
        );
        println!(
            "Cap sensitivities: {:?} = {:?}",
            sens.instrument_keys(),
            sens.exposure()
        );
        Ok(())
    }
}

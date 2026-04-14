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
    instruments::rates::capletfloorlet::{CapletFloorletTrade, CapletFloorletType},
    math::probability::norm_cdf::norm_cdf,
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
};
use std::collections::HashSet;

/// Prices a caplet or floorlet using the Hull-White one-factor model.
///
/// The caplet is represented as a zero-coupon bond option via the
/// Jamshidian decomposition:
///
/// $$
///   \text{Caplet}(0) = (1 + \tau K)\,\text{BondPut}(0;\, T,\, S,\, X)
/// $$
///
/// where $T$ is the reset date, $S$ is the payment date, $\tau = S - T$,
/// $K$ is the strike, and $X = 1/(1+\tau K)$.
///
/// The model parameters `alpha` (mean-reversion) and `sigma` (volatility)
/// are provided at construction time.
///
/// When a [`DiscountPolicy`] is set and yields a different discount index,
/// the pricer applies a deterministic ratio adjustment:
/// $\text{df}_{\text{disc}}(S) / \text{df}_{\text{fwd}}(S)$.
pub struct ClosedFormHullWhiteCapletPricer {
    alpha: f64,
    sigma: f64,
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl ClosedFormHullWhiteCapletPricer {
    /// Creates a new [`ClosedFormHullWhiteCapletPricer`] with the given
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

    /// ZCB price volatility: `sigma_P = sigma * B(t,T) * sqrt((1 - exp(-2*alpha*t)) / (2*alpha))`.
    fn zcb_price_volatility(&self, t: f64, big_t: f64) -> f64 {
        let b = self.B(t, big_t);
        self.sigma * b * ((1.0 - (-2.0 * self.alpha * t).exp()) / (2.0 * self.alpha)).sqrt()
    }
}

/// State for Hull-White caplet/floorlet pricing.
#[derive(Default)]
struct HullWhiteCapletState {
    value: Option<DualFwd>,
    market_data: Option<MarketData>,
}

impl PricerState for HullWhiteCapletState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl HandleValue<CapletFloorletTrade, HullWhiteCapletState> for ClosedFormHullWhiteCapletPricer {
    fn handle_value(
        &self,
        trade: &CapletFloorletTrade,
        state: &mut HullWhiteCapletState,
    ) -> Result<f64> {
        let caplet = trade.instrument();
        let fwd_index = caplet.market_index();
        let disc_index = if let Some(policy) = &self.discount_policy {
            policy.accept(caplet)?
        } else {
            fwd_index.clone()
        };

        let start_date = caplet.start_accrual_date();
        let end_date = caplet.end_accrual_date();
        let payment_date = caplet.payment_date();
        let rate_def = fwd_index.rate_index_details()?.rate_definition();
        let dc = rate_def.day_counter();

        // Year fractions
        let t = dc.year_fraction(trade.trade_date(), start_date); // option expiry
        let big_s = dc.year_fraction(trade.trade_date(), payment_date); // bond maturity
        let tau = dc.year_fraction(start_date, end_date); // accrual period

        // Resolve strike
        Tape::start_recording_fwd();
        Tape::set_mark_fwd();
        state.put_pillars_on_tape()?;

        let fwd: DualFwd = state
            .get_discount_curve_element(&fwd_index)?
            .curve()
            .forward_rate(
                start_date,
                end_date,
                rate_def.compounding(),
                rate_def.frequency(),
            )?;

        let strike = match caplet.strike() {
            Strike::Absolute(k) => k,
            Strike::Atm => fwd.value(),
            Strike::Relative(pct) => fwd.value() * (1.0 + pct),
        };

        // Bond-option parameters (all f64 — model params only)
        let x = 1.0 / tau.mul_add(strike, 1.0); // bond strike
        let sigma_p = self.zcb_price_volatility(t, big_s); // f64

        // DualFwd discount factors from the forward curve
        let df_t: DualFwd = state
            .get_discount_curve_element(&fwd_index)?
            .curve()
            .discount_factor(start_date)?;
        let df_s: DualFwd = state
            .get_discount_curve_element(&fwd_index)?
            .curve()
            .discount_factor(payment_date)?;

        // d1, d2 in DualFwd: ln(df_s / (X * df_t)) lives on the AD tape
        let d1: DualFwd = (((df_s / (DualFwd::new(x) * df_t)).ln()
            + DualFwd::new(0.5 * sigma_p * sigma_p))
            / DualFwd::new(sigma_p))
        .into();
        let d2: DualFwd = (d1 - DualFwd::new(sigma_p)).into();

        // Bond option price in DualFwd
        let undiscounted: DualFwd = match caplet.payoff_type() {
            // Caplet = (1+τK) * BondPut = (1+τK) * [X·P(0,T)·Φ(−d2) − P(0,S)·Φ(−d1)]
            CapletFloorletType::Caplet => {
                let neg_d2: DualFwd = (DualFwd::new(0.0) - d2).into();
                let neg_d1: DualFwd = (DualFwd::new(0.0) - d1).into();
                let put: DualFwd =
                    (DualFwd::new(x) * df_t * norm_cdf(neg_d2) - df_s * norm_cdf(neg_d1)).into();
                (DualFwd::new(tau.mul_add(strike, 1.0)) * put).into()
            }
            // Floorlet = (1+τK) * BondCall = (1+τK) * [P(0,S)·Φ(d1) − X·P(0,T)·Φ(d2)]
            CapletFloorletType::Floorlet => {
                let call: DualFwd =
                    (df_s * norm_cdf(d1) - DualFwd::new(x) * df_t * norm_cdf(d2)).into();
                (DualFwd::new(tau.mul_add(strike, 1.0)) * call).into()
            }
        };

        // Dual-curve adjustment: if discount index != forward index, multiply
        // by df_disc(S) / df_fwd(S)
        let value: DualFwd = if disc_index == fwd_index {
            (undiscounted * DualFwd::new(trade.notional())).into()
        } else {
            let df_disc_s: DualFwd = state
                .get_discount_curve_element(&disc_index)?
                .curve()
                .discount_factor(payment_date)?;
            (undiscounted * df_disc_s / df_s * DualFwd::new(trade.notional())).into()
        };

        state.value = Some(value);
        Tape::stop_recording_fwd();
        Ok(value.value())
    }
}

impl HandleSensitivities<CapletFloorletTrade, HullWhiteCapletState>
    for ClosedFormHullWhiteCapletPricer
{
    fn handle_sensitivities(
        &self,
        trade: &CapletFloorletTrade,
        state: &mut HullWhiteCapletState,
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
        let fwd_index = caplet.market_index();
        let policy_discount_index = if let Some(policy) = &self.discount_policy {
            Some(policy.accept(caplet)?)
        } else {
            None
        };
        let discount_index = policy_discount_index.as_ref().unwrap_or(&fwd_index);

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
            exposures.push(pillar.adjoint()?.value());
        }

        // Forward curve sensitivities (when different from discount)
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

impl Pricer for ClosedFormHullWhiteCapletPricer {
    type Item = CapletFloorletTrade;
    type Policy = dyn DiscountPolicy;

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
        let mut state = HullWhiteCapletState {
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
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

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
        instruments::rates::capletfloorlet::{
            CapletFloorlet, CapletFloorletTrade, CapletFloorletType,
        },
        models::hullwhite::hullwhitemodel::HullWhite,
        pricers::rates::closedformhullwhitecapletpricer::ClosedFormHullWhiteCapletPricer,
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::{
                flatforwardtermstructure::FlatForwardTermStructure,
                interestratestermstructure::InterestRatesTermStructure,
            },
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

    /// Builds market data with a flat DualFwd discount curve.
    fn setup_hw_market_data(
        trade_date: Date,
        market_index: &MarketIndex,
        risk_free_rate: f64,
    ) -> (MarketData, FlatForwardTermStructure<DualFwd>) {
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
                Rc::new(RefCell::new(discount_curve.clone())),
            ),
        );

        let market_data = MarketData::new(HashMap::new(), constructed_elements);
        (market_data, discount_curve)
    }

    /// Builds an f64 flat curve for use with `HullWhite::caplet_price`.
    fn flat_f64_curve(ref_date: Date, rate: f64) -> FlatForwardTermStructure<f64> {
        FlatForwardTermStructure::new(ref_date, rate, RateDefinition::default())
    }

    #[test]
    fn hw_caplet_matches_closed_form() -> Result<()> {
        let alpha = 0.1_f64;
        let sigma = 0.01;
        let r = 0.04;
        let strike = 0.05;
        let notional = 1_000_000.0;

        let trade_date = Date::new(2025, 1, 2);
        let start_date = trade_date + Period::new(1, TimeUnit::Years);
        let end_date = start_date + Period::new(3, TimeUnit::Months);
        let market_index = MarketIndex::TermSOFR3m;

        // DC = Actual360 (from TermSOFR3m rate_definition)
        let dc = DayCounter::Actual360;
        let t = dc.year_fraction(trade_date, start_date);
        let big_s = dc.year_fraction(trade_date, end_date);

        // Reference: HullWhite model closed-form caplet price
        let f64_curve = flat_f64_curve(trade_date, r);
        let hw = HullWhite::new(alpha, &f64_curve);
        let hw_price = hw.caplet_price(strike, t, big_s, sigma, &f64_curve)?;

        // Pricer
        let (market_data, _) = setup_hw_market_data(trade_date, &market_index, r);
        let caplet = CapletFloorlet::new(
            "HW_CAPLET".to_string(),
            market_index,
            Currency::USD,
            start_date,
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Caplet,
            Strike::Absolute(strike),
        );
        let trade = CapletFloorletTrade::new(caplet, trade_date, notional, Side::LongReceive);
        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = ClosedFormHullWhiteCapletPricer::new(alpha, sigma);
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;

        let expected = hw_price * notional;
        println!("Pricer price:      {price}");
        println!("HW closed-form:    {expected}");
        assert!(
            (price - expected).abs() < 1e-6,
            "caplet price mismatch: pricer={price}, hw={expected}"
        );
        Ok(())
    }

    #[test]
    fn hw_floorlet_matches_closed_form() -> Result<()> {
        let alpha = 0.1_f64;
        let sigma = 0.01;
        let r = 0.04;
        let strike = 0.03;
        let notional = 1_000_000.0;

        let trade_date = Date::new(2025, 1, 2);
        let start_date = trade_date + Period::new(1, TimeUnit::Years);
        let end_date = start_date + Period::new(3, TimeUnit::Months);
        let market_index = MarketIndex::TermSOFR3m;

        let dc = DayCounter::Actual360;
        let t = dc.year_fraction(trade_date, start_date);
        let big_s = dc.year_fraction(trade_date, end_date);

        let f64_curve = flat_f64_curve(trade_date, r);
        let hw = HullWhite::new(alpha, &f64_curve);
        let hw_price = hw.floorlet_price(strike, t, big_s, sigma, &f64_curve)?;

        let (market_data, _) = setup_hw_market_data(trade_date, &market_index, r);
        let floorlet = CapletFloorlet::new(
            "HW_FLOORLET".to_string(),
            market_index,
            Currency::USD,
            start_date,
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Floorlet,
            Strike::Absolute(strike),
        );
        let trade = CapletFloorletTrade::new(floorlet, trade_date, notional, Side::LongReceive);
        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let pricer = ClosedFormHullWhiteCapletPricer::new(alpha, sigma);
        let results = pricer.evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;

        let expected = hw_price * notional;
        println!("Pricer price:      {price}");
        println!("HW closed-form:    {expected}");
        assert!(
            (price - expected).abs() < 1e-6,
            "floorlet price mismatch: pricer={price}, hw={expected}"
        );
        Ok(())
    }

    #[test]
    fn hw_caplet_sensitivities_bump_and_reprice() -> Result<()> {
        let alpha = 0.1_f64;
        let sigma = 0.01;
        let r = 0.04;
        let strike = 0.05;
        let notional = 1.0;
        let bump = 1e-5;

        let trade_date = Date::new(2025, 1, 2);
        let start_date = trade_date + Period::new(1, TimeUnit::Years);
        let end_date = start_date + Period::new(3, TimeUnit::Months);
        let market_index = MarketIndex::TermSOFR3m;

        let caplet = CapletFloorlet::new(
            "HW_CAPLET".to_string(),
            market_index.clone(),
            Currency::USD,
            start_date,
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Caplet,
            Strike::Absolute(strike),
        );
        let trade = CapletFloorletTrade::new(caplet, trade_date, notional, Side::LongReceive);
        let pricer = ClosedFormHullWhiteCapletPricer::new(alpha, sigma);

        // AD sensitivities
        let (md, _) = setup_hw_market_data(trade_date, &market_index, r);
        let results = pricer.evaluate(
            &trade,
            &[Request::Value, Request::Sensitivities],
            &SimpleMarketDataProvider {
                evaluation_date: trade_date,
                market_data: md,
            },
        )?;
        let sens = results
            .sensitivities()
            .ok_or_else(|| QSError::UnexpectedErr("Missing sensitivities".into()))?;

        let ad_rate_sens = sens
            .instrument_keys()
            .iter()
            .zip(sens.exposure().iter().copied())
            .find(|(k, _)| k.as_str() == "discount_rate")
            .map(|(_, v)| v)
            .ok_or_else(|| QSError::NotFoundErr("discount_rate not found".into()))?;

        // Bump-and-reprice
        let (md_up, _) = setup_hw_market_data(trade_date, &market_index, r + bump);
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
            .unwrap();

        let (md_dn, _) = setup_hw_market_data(trade_date, &market_index, r - bump);
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
            .unwrap();

        let fd_sens = (price_up - price_dn) / (2.0 * bump);

        println!("AD rate sens: {ad_rate_sens:.8}");
        println!("FD rate sens: {fd_sens:.8}");
        assert!(
            (ad_rate_sens - fd_sens).abs() < 1e-4,
            "rate sensitivity mismatch: ad={ad_rate_sens}, fd={fd_sens}"
        );
        Ok(())
    }

    #[test]
    fn hw_caplet_put_call_parity() -> Result<()> {
        let alpha = 0.1_f64;
        let sigma = 0.01;
        let r = 0.04;
        let strike = 0.04; // ATM for clean parity
        let notional = 1_000_000.0;

        let trade_date = Date::new(2025, 1, 2);
        let start_date = trade_date + Period::new(1, TimeUnit::Years);
        let end_date = start_date + Period::new(3, TimeUnit::Months);
        let market_index = MarketIndex::TermSOFR3m;

        let pricer = ClosedFormHullWhiteCapletPricer::new(alpha, sigma);

        let caplet = CapletFloorlet::new(
            "HW_CAPLET".to_string(),
            market_index.clone(),
            Currency::USD,
            start_date,
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Caplet,
            Strike::Absolute(strike),
        );
        let floorlet = CapletFloorlet::new(
            "HW_FLOORLET".to_string(),
            market_index.clone(),
            Currency::USD,
            start_date,
            start_date,
            end_date,
            end_date,
            CapletFloorletType::Floorlet,
            Strike::Absolute(strike),
        );

        let (md, discount_curve) = setup_hw_market_data(trade_date, &market_index, r);

        let cap_trade = CapletFloorletTrade::new(caplet, trade_date, notional, Side::LongReceive);
        let floor_trade =
            CapletFloorletTrade::new(floorlet, trade_date, notional, Side::LongReceive);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data: md,
        };

        let cap_price = pricer
            .evaluate(&cap_trade, &[Request::Value], &provider)?
            .price()
            .unwrap();
        let floor_price = pricer
            .evaluate(&floor_trade, &[Request::Value], &provider)?
            .price()
            .unwrap();

        // Put-call parity: Caplet - Floorlet = notional * tau * (F - K) * df_pay
        let rate_def = RateDefinition::default();
        let dc = rate_def.day_counter();
        let tau = dc.year_fraction(start_date, end_date);
        let fwd = discount_curve
            .forward_rate(
                start_date,
                end_date,
                rate_def.compounding(),
                rate_def.frequency(),
            )?
            .value();
        let df_pay = discount_curve.discount_factor(end_date)?.value();
        let parity_rhs = notional * tau * (fwd - strike) * df_pay;

        let parity_lhs = cap_price - floor_price;
        println!("Caplet - Floorlet: {parity_lhs:.6}");
        println!("tau*(F-K)*df*N:    {parity_rhs:.6}");
        assert!(
            (parity_lhs - parity_rhs).abs() < 1e-2,
            "put-call parity violated: lhs={parity_lhs}, rhs={parity_rhs}"
        );
        Ok(())
    }
}

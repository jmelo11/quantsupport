use crate::{
    ad::{dual::DualFwd, tape::Tape},
    core::{
        collateral::{DiscountPolicy, Discountable},
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            fixingrequest::FixingRequest,
            fxrequest::FxRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::equity::equityforward::EquityForwardTrade,
    utils::errors::{QSError, Result},
};

/// Forward valuation state and the fixing date used for spot.
#[derive(Default)]
struct EquityForwardState {
    evaluation_date: crate::time::date::Date,
    value: Option<DualFwd>,
    spot: Option<DualFwd>,
    market_data: Option<MarketData>,
}

impl PricerState for EquityForwardState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

/// Prices equity forwards as `N * side * (S * DF_dividend / DF_forward - K) * DF_payment`.
///
/// The forward curve belongs to the equity index; an optional discount policy
/// selects the payment curve. Spot is the fixing at the context evaluation date.
/// A missing dividend curve means zero dividends. Supports value, fair rate and
/// sensitivities to spot and both curves.
pub struct EquityForwardPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl EquityForwardPricer {
    /// Creates a new [`EquityForwardPricer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            discount_policy: None,
        }
    }
}

impl Default for EquityForwardPricer {
    fn default() -> Self {
        Self::new()
    }
}

// Always stop recording if a missing fixing or curve aborts valuation.
struct RecordingGuard;
impl Drop for RecordingGuard {
    fn drop(&mut self) {
        Tape::stop_recording_fwd();
    }
}

impl HandleValue<EquityForwardTrade, EquityForwardState> for EquityForwardPricer {
    fn handle_value(
        &self,
        trade: &EquityForwardTrade,
        state: &mut EquityForwardState,
    ) -> Result<f64> {
        let forward = trade.instrument();
        let expiry = forward.delivery_date();
        let index = forward.market_index();
        let discount_index = if let Some(policy) = &self.discount_policy {
            policy.accept(forward)?
        } else {
            index.clone()
        };

        Tape::start_recording_fwd();
        let _recording_guard = RecordingGuard;
        Tape::set_mark_fwd();

        // Record spot separately to report delta alongside curve risk.
        let spot = state.get_fixing(&index, state.evaluation_date)?;
        let mut spot_ad = DualFwd::new(spot);
        spot_ad.put_on_tape();
        state.spot = Some(spot_ad);

        state.put_pillars_on_tape()?;

        let df_forward = state
            .get_discount_curve_element(&index)?
            .curve()
            .discount_factor(expiry)?;
        let df_r = state
            .get_discount_curve_element(&discount_index)?
            .curve()
            .discount_factor(expiry)?;
        let df_q = if let Ok(curve) = state.get_dividend_curve_element(&index) {
            curve.curve().discount_factor(forward.delivery_date())?
        } else {
            DualFwd::one()
        };

        let fwd: DualFwd = (spot_ad * df_q / df_forward).into();
        let value: DualFwd =
            (df_r * (fwd - forward.strike()) * trade.notional() * trade.side().sign()).into();
        let source = Discountable::currency(forward);
        let value = if let Some(target) = state
            .get_market_data_reponse()
            .and_then(MarketData::base_currency)
            .filter(|target| *target != source)
        {
            (value * state.get_exchange_rate(source, target)?).into()
        } else {
            value
        };
        state.value = Some(value);
        Ok(value.value())
    }
}

impl HandleSensitivities<EquityForwardTrade, EquityForwardState> for EquityForwardPricer {
    fn handle_sensitivities(
        &self,
        trade: &EquityForwardTrade,
        state: &mut EquityForwardState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(value) = state.value {
            value
        } else {
            let _ = self.handle_value(trade, state)?;
            state.value.ok_or_else(|| {
                QSError::UnexpectedErr(
                    "Equity forward price was not recorded after valuation.".into(),
                )
            })?
        };

        value.backward_to_mark()?;
        let forward = trade.instrument();
        let index = forward.market_index();
        let policy_discount_index = if let Some(policy) = &self.discount_policy {
            Some(policy.accept(forward)?)
        } else {
            None
        };
        let discount_index = policy_discount_index.as_ref().unwrap_or(&index);

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        ids.push(index.to_string());
        exposures.push(
            state
                .spot
                .ok_or_else(|| {
                    QSError::UnexpectedErr(
                        "Equity forward spot was not recorded for sensitivities".into(),
                    )
                })?
                .adjoint()?
                .value(),
        );

        if discount_index != &index {
            for (label, pillar) in state
                .get_discount_curve_element(&index)?
                .curve()
                .pillars()
                .unwrap_or_default()
            {
                ids.push(label);
                exposures.push(pillar.adjoint()?.value());
            }
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

        if let Ok(curve) = state.get_dividend_curve_element(&index) {
            for (label, pillar) in curve.curve().pillars().unwrap_or_default() {
                ids.push(label);
                exposures.push(pillar.adjoint()?.value());
            }
        }

        for (label, exposure) in state.get_fx_sensitivities()? {
            ids.push(label);
            exposures.push(exposure);
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures)
            .aggregate())
    }
}

impl Pricer for EquityForwardPricer {
    type Item = EquityForwardTrade;
    type Policy = dyn DiscountPolicy;
    fn evaluate(
        &self,
        trade: &EquityForwardTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        Request::validate(requests)?;
        let eval_date = ctx.evaluation_date();
        if !trade.notional().is_finite()
            || trade.notional() < 0.0
            || !trade.instrument().strike().is_finite()
        {
            return Err(QSError::InvalidValueErr(
                "Equity forward requires a finite strike and finite non-negative notional".into(),
            ));
        }
        if trade.instrument().delivery_date() < eval_date {
            return Err(QSError::InvalidValueErr(
                "Equity forward delivery precedes the evaluation date".into(),
            ));
        }
        let forward = trade.instrument();
        let identifier = forward.identifier();

        let md_request = self
            .market_data_request(trade)
            .ok_or_else(|| QSError::InvalidValueErr("Missing market data request".into()))?
            .with_fixings_request(vec![FixingRequest::new(forward.market_index(), eval_date)]);

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = EquityForwardState {
            evaluation_date: eval_date,
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
                Request::FairRate(target) => {
                    if *target != 0 {
                        return Err(QSError::InvalidValueErr(
                            "Equity-forward fair-rate target must be zero".into(),
                        ));
                    }
                    let expiry = forward.delivery_date();
                    let spot = state.get_fixing(&forward.market_index(), eval_date)?;
                    let dividend = state
                        .get_dividend_curve_element(&forward.market_index())
                        .map_or_else(
                            |_| Ok(DualFwd::one()),
                            |e| e.curve().discount_factor(expiry),
                        )?;
                    let forward_df = state
                        .get_discount_curve_element(&forward.market_index())?
                        .curve()
                        .discount_factor(expiry)?;
                    results = results.with_fair_rate(spot * dividend.value() / forward_df.value());
                }
                _ => {
                    return Err(QSError::InvalidValueErr(
                        "Equity forward supports only Value, FairRate and Sensitivities".into(),
                    ))
                }
            }
        }

        Ok(results)
    }

    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest> {
        let forward = trade.instrument();
        let index = forward.market_index();
        let mut elements = vec![
            ConstructedElementRequest::DiscountCurve {
                market_index: index.clone(),
            },
            ConstructedElementRequest::OptionalDividendCurve {
                market_index: index.clone(),
            },
        ];
        let fixings = vec![FixingRequest::new(index.clone(), trade.trade_date())];

        if let Some(policy) = &self.discount_policy {
            let collateral_index = policy.accept(forward).ok()?;
            if collateral_index != index {
                elements.push(ConstructedElementRequest::DiscountCurve {
                    market_index: collateral_index,
                });
            }
        }

        let mut request = MarketDataRequest::default()
            .with_constructed_elements_request(elements)
            .with_fixings_request(fixings);
        let mut fx = request.fx_request().cloned().unwrap_or_default();
        fx.push(FxRequest::single(Discountable::currency(
            trade.instrument(),
        )));
        request = request.with_fx_request(fx);
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
    use crate::prelude::*;
    use std::{
        cell::RefCell,
        collections::{BTreeMap, HashMap},
        rc::Rc,
    };
    struct Provider {
        data: MarketData,
    }
    impl MarketDataProvider for Provider {
        fn evaluation_date(&self) -> Date {
            Date::new(2026, 1, 2)
        }
        fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
            Ok(MarketData::new(
                self.data.fixings().clone(),
                self.data.constructed_elements().clone(),
            ))
        }
    }
    /// Builds spot, forwarding, dividend and payment-curve inputs for equity-forward tests.
    fn provider(spot: f64, forward: f64, dividend: Option<f64>, payment: f64) -> Provider {
        let date = Date::new(2026, 1, 2);
        let index = MarketIndex::Equity("SPX".into());
        let definition = RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        );
        let curve = |rate, label: &str| {
            Rc::new(RefCell::new(
                FlatForwardTermStructure::new(date, DualFwd::from(rate), definition)
                    .with_pillar_label(label.into()),
            ))
        };
        let mut elements = ConstructedElementStore::default();
        elements.discount_curves_mut().insert(
            index.clone(),
            DiscountCurveElement::new(index.clone(), curve(forward, "forward")),
        );
        elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(MarketIndex::SOFR, curve(payment, "payment")),
        );
        if let Some(rate) = dividend {
            elements.dividend_curves_mut().insert(
                index.clone(),
                DividendCurveElement::new(index.clone(), curve(rate, "dividend")),
            );
        }
        Provider {
            data: MarketData::new(
                HashMap::from([(index, BTreeMap::from([(date, spot)]))]),
                elements,
            ),
        }
    }
    /// Builds an equity-forward position with the requested trade direction.
    fn trade(side: Side) -> Result<EquityForwardTrade> {
        let instrument = MakeEquityForward::default()
            .with_identifier("FORWARD".into())
            .with_market_index(MarketIndex::Equity("SPX".into()))
            .with_currency(Currency::USD)
            .with_delivery_date(Date::new(2027, 1, 2))
            .with_strike(100.0)
            .build()?;
        // Spot must come from evaluation date, although this position was executed earlier.
        Ok(EquityForwardTrade::new(
            instrument,
            Date::new(2025, 1, 2),
            10.0,
            side,
        ))
    }
    /// Builds an equity-forward pricer with USD SOFR collateral discounting.
    fn pricer() -> EquityForwardPricer {
        let mut pricer = EquityForwardPricer::new();
        pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
            MarketIndex::SOFR,
            Currency::USD,
        )));
        pricer
    }
    #[test]
    fn prices_dividends_and_collateral_and_matches_finite_difference_risk() -> Result<()> {
        let spot = 100.0;
        let forward = 0.05;
        let dividend = 0.02;
        let payment = 0.03;
        let position = trade(Side::LongReceive)?;
        let result = pricer().evaluate(
            &position,
            &[Request::Value, Request::FairRate(0), Request::Sensitivities],
            &provider(spot, forward, Some(dividend), payment),
        )?;
        let fair = spot * (forward - dividend).exp();
        let expected = 10.0 * (fair - 100.0) * (-payment).exp();
        assert!((result.price().unwrap() - expected).abs() < 1e-10);
        assert!((result.fair_rate().unwrap() - fair).abs() < 1e-10);
        let risk = result.sensitivities().unwrap();
        let bumps = [
            ("EQ:SPX", [1e-5, 0.0, 0.0, 0.0]),
            ("forward", [0.0, 1e-5, 0.0, 0.0]),
            ("dividend", [0.0, 0.0, 1e-5, 0.0]),
            ("payment", [0.0, 0.0, 0.0, 1e-5]),
        ];
        for (label, bump) in bumps {
            let value = |sign: f64| -> Result<f64> {
                Ok(pricer()
                    .evaluate(
                        &position,
                        &[Request::Value],
                        &provider(
                            spot + sign * bump[0],
                            forward + sign * bump[1],
                            Some(dividend + sign * bump[2]),
                            payment + sign * bump[3],
                        ),
                    )?
                    .price()
                    .unwrap())
            };
            let numerical = (value(1.0)? - value(-1.0)?) / 2e-5;
            let index = risk
                .instrument_keys()
                .iter()
                .position(|key| key == label)
                .unwrap();
            assert!((risk.exposure()[index] - numerical).abs() < 1e-5, "{label}");
        }
        let short = pricer()
            .evaluate(
                &trade(Side::PayShort)?,
                &[Request::Value],
                &provider(spot, forward, Some(dividend), payment),
            )?
            .price()
            .unwrap();
        assert!((short + expected).abs() < 1e-10);
        Ok(())
    }
    #[test]
    fn missing_dividends_and_market_data_and_unsupported_requests() -> Result<()> {
        let position = trade(Side::LongReceive)?;
        let result = pricer().evaluate(
            &position,
            &[Request::FairRate(0)],
            &provider(100.0, 0.05, None, 0.03),
        )?;
        assert!((result.fair_rate().unwrap() - 100.0 * 0.05_f64.exp()).abs() < 1e-10);
        let mut missing = provider(100.0, 0.05, None, 0.03);
        missing.data = MarketData::new(HashMap::new(), missing.data.constructed_elements().clone());
        assert!(pricer()
            .evaluate(&position, &[Request::Value], &missing)
            .is_err());
        assert!(!Tape::<f64>::is_active());
        assert!(pricer()
            .evaluate(
                &position,
                &[Request::ModifiedDuration],
                &provider(100.0, 0.05, None, 0.03)
            )
            .is_err());
        Ok(())
    }
}

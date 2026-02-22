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
            ADReal::zero()
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

        for (label, pillar) in state
            .get_dividend_curve_element(index)?
            .curve()
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

use crate::{
    ad::{
        adreal::{ADReal, IsReal},
        tape::Tape,
    },
    core::{
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        marketdataprovider::{
            DerivedElementRequest, MarketDataProvider, MarketDataRequest, MarketDataResponse,
        },
        pricer::Pricer,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::fixedincome::deposit::DepositTrade,
    pricers::pricers::DiscountedCashflowPricer,
    utils::errors::{AtlasError, Result},
};

/// # `DepositPriceEvaluationState`
///
/// Holds state information for deposit price evaluation.
#[derive(Default)]
struct DepositPriceEvaluationState {
    /// Price placeholder for perfomance reasons.
    pub value: Option<ADReal>,
    pub md_response: Option<MarketDataResponse>,
}

impl HandleValue<DepositTrade, DepositPriceEvaluationState> for DiscountedCashflowPricer {
    fn handle_value(
        &self,
        trade: &DepositTrade,
        state: &mut DepositPriceEvaluationState,
    ) -> Result<f64> {
        Tape::start_recording();
        Tape::set_mark();
        let index = trade.instrument().market_index();
        let final_amount = trade.instrument().final_payment().unwrap();
        let discount_curve_element = state
            .md_response
            .as_ref()
            .unwrap()
            .discount_curves
            .get(&index)
            .unwrap();

        let df = discount_curve_element
            .curve
            .discount_factor(trade.instrument().maturity_date())?;
        let value: ADReal = (df * final_amount).into();
        state.value = Some(value);
        Tape::stop_recording();
        Tape::rewind_to_mark();
        Ok(value.value())
    }
}

impl HandleSensitivities<DepositTrade, DepositPriceEvaluationState> for DiscountedCashflowPricer {
    fn handle_sensitivities(
        &self,
        trade: &DepositTrade,
        state: &mut DepositPriceEvaluationState,
    ) -> Result<SensitivityMap> {
        let price: ADReal;
        match state.value {
            Some(p) => {
                price = p;
            }
            None => {
                let _ = self.handle_value(trade, state)?;
                price = state.value.unwrap();
            }
        }
        let _ = price.backward()?;
        let mut ids = Vec::new();
        let mut exposures = Vec::new();
        let index = trade.instrument().market_index();
        let discount_curve_element = state
            .md_response
            .as_ref()
            .unwrap()
            .discount_curves
            .get(&index)
            .unwrap();

        for (label, component) in discount_curve_element.pillars.iter() {
            ids.push(label.clone());
            let s = component.adjoint()?;
            exposures.push(s);
        }
        let sensitivities = SensitivityMap::default()
            .with_instrument_keys(ids)
            .with_exposure(exposures);
        Ok(sensitivities)
    }
}

impl Pricer for DiscountedCashflowPricer {
    type Item = DepositTrade;

    fn evaluate(
        &self,
        trade: &DepositTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let depo = trade.instrument();
        let identifier = depo.identifier();

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = DepositPriceEvaluationState::default();
        let md_request = self
            .market_data_request(trade)
            .ok_or(AtlasError::InvalidValueErr(
                "A market data request should have been returned!".into(),
            ))?;

        state.md_response = Some(ctx.handle_request(&md_request)?);

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
        let discount_curve = DerivedElementRequest::DiscountCurve {
            market_index: trade.instrument().market_index(),
        };
        Some(MarketDataRequest::default().with_element_requests(vec![discount_curve]))
    }
}

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
            .as_mut()
            .unwrap()
            .discount_curves
            .get_mut(&index)
            .unwrap();

        for (_, pillar) in &mut discount_curve_element.pillars {
            pillar.put_on_tape();
        }

        let df = discount_curve_element
            .curve
            .discount_factor(trade.instrument().maturity_date())?;
        let value: ADReal = (df * final_amount).into();
        state.value = Some(value);
        Tape::stop_recording();
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


#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        ad::adreal::ADReal,
        core::{
            contextmanager::ContextManager,
            inmemorymarketdataprovider::InMemoryMarketDataProvider,
            instrument::Instrument,
            marketdataprovider::DiscountCurveElement,
            pricer::Pricer,
            request::Request,
        },
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        instruments::fixedincome::deposit::{Deposit, DepositTrade},
        marketdata::{fixingstore::FixingStore, quotestore::QuoteStore},
        pricers::pricers::DiscountedCashflowPricer,
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::date::Date,
    };

    #[test]
    fn deposit_value_and_sensitivities() {
        let eval = Date::new(2025, 1, 1);
        let maturity = Date::new(2025, 7, 1);
        let index = MarketIndex::SOFR;

        let depo = Deposit::new(
            "D1".to_string(),
            100.0,
            crate::rates::interestrate::InterestRate::from_rate_definition(0.05, RateDefinition::default()),
            eval,
            maturity,
            index.clone(),
        );
        let ctx_mgr = ContextManager::new(QuoteStore::new(eval), FixingStore::new());
        let resolved = depo.resolve(&ctx_mgr).expect("resolved deposit");
        let trade = DepositTrade::new(resolved, eval, 100.0);

        let curve = Arc::new(FlatForwardTermStructure::<ADReal>::new(
            eval,
            ADReal::from(0.03),
            RateDefinition::default(),
        ));

        let md = InMemoryMarketDataProvider::new(eval).with_discount_curve(DiscountCurveElement {
            market_index: index,
            currency: Currency::USD,
            pillars: vec![
                ("p0".to_string(), ADReal::from(1.0)),
                ("p1".to_string(), ADReal::from(0.98)),
            ],
            curve,
        });

        let pricer = DiscountedCashflowPricer;
        let results = pricer
            .evaluate(&trade, &[Request::Value, Request::Sensitivities], &md)
            .expect("pricing works");

        assert!(results.price().is_some());
        let sens = results.sensitivities().expect("sensitivities present");
        assert!(!sens.instrument_keys().is_empty());
        assert_eq!(sens.instrument_keys().len(), sens.exposure().len());
    }
}

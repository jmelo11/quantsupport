use rayon::str;

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
    utils::errors::{AtlasError, Result},
};

pub struct DiscountedDepositPricer;

/// # `DepositPriceEvaluationState`
///
/// Holds state information for deposit price evaluation.
#[derive(Default)]
struct DepositPriceEvaluationState {
    /// Price placeholder for perfomance reasons.
    pub value: Option<ADReal>,
    pub md_response: Option<MarketDataResponse>,
}

impl HandleValue<DepositTrade, DepositPriceEvaluationState> for DiscountedDepositPricer {
    fn handle_value(
        &self,
        trade: &DepositTrade,
        state: &mut DepositPriceEvaluationState,
    ) -> Result<f64> {
        Tape::start_recording();
        let index = trade.instrument().market_index();
        let final_amount = trade.instrument().final_payment().unwrap();
        let element = state
            .md_response
            .as_mut()
            .unwrap()
            .discount_curves
            .get_mut(&index)
            .unwrap();

        element.curve_mut().put_pillars_on_tape();

        let df = element
            .curve()
            .discount_factor(trade.instrument().maturity_date())?;
        let value = (df * final_amount).into();
        state.value = Some(value);

        Tape::stop_recording();
        Ok(value.value())
    }
}

impl HandleSensitivities<DepositTrade, DepositPriceEvaluationState> for DiscountedDepositPricer {
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
        let _ = price.backward_to_mark()?;
        let mut ids = Vec::new();
        let mut exposures = Vec::new();
        let index = trade.instrument().market_index();
        let element = state
            .md_response
            .as_ref()
            .unwrap()
            .discount_curves
            .get(&index)
            .unwrap();

        if let Some(pillars) = element.curve().pillars() {
            for (label, value) in pillars {
                ids.push(label);
                exposures.push(value.adjoint()?);
            }
        }

        let sensitivities = SensitivityMap::default()
            .with_instrument_keys(ids)
            .with_exposure(exposures);
        Ok(sensitivities)
    }
}

impl Pricer for DiscountedDepositPricer {
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

    fn market_data_request(&self, trade: &DepositTrade) -> Option<MarketDataRequest> {
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
            contextmanager::ContextManager, inmemorymarketdataprovider::InMemoryMarketDataProvider,
            instrument::Instrument, marketdataprovider::DiscountCurveElement, pricer::Pricer,
            request::Request, trade::Trade,
        },
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        instruments::fixedincome::deposit::{Deposit, DepositTrade},
        marketdata::{fixingstore::FixingStore, quotestore::QuoteStore},
        pricers::fixedincome::depositdiscountedcashflowpricer::DiscountedDepositPricer,
        rates::{
            interestrate::{InterestRate, RateDefinition},
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::date::Date,
    };

    #[test]
    fn deposit_value_and_sensitivities() {
        let eval = Date::new(2025, 1, 1);
        let maturity = Date::new(2025, 7, 1);
        let index = MarketIndex::TermSOFR3m;

        let depo = Deposit::new(
            "TEST".into(),
            100.0,
            InterestRate::from_rate_definition(0.05, RateDefinition::default()),
            eval,
            maturity,
            index.clone(),
        );
        let ctx_mgr = ContextManager::new(QuoteStore::new(eval), FixingStore::new());
        let resolved = depo.resolve(&ctx_mgr).expect("resolved deposit");
        let trade = DepositTrade::new(resolved, eval, 100.0);

        let curve = Box::new(
            FlatForwardTermStructure::<ADReal>::new(
                eval,
                ADReal::from(0.03),
                RateDefinition::default(),
            )
            .with_pillar_label("Rate".into()),
        );

        let md = InMemoryMarketDataProvider::new(eval)
            .with_discount_curve(DiscountCurveElement::new(index, Currency::USD, curve));

        let pricer = DiscountedDepositPricer;
        let results = pricer
            .evaluate(&trade, &[Request::Value, Request::Sensitivities], &md)
            .expect("pricing works");

        assert!(results.price().is_some());
        let sens = results.sensitivities().expect("sensitivities present");
        println!("Final Payment: {:?}", trade.instrument().final_payment());
        println!("Price: {:?}", results.price().unwrap());
        println!("Sensitivities: {:?}", sens);
        assert!(!sens.instrument_keys().is_empty());
        assert_eq!(sens.instrument_keys().len(), sens.exposure().len());
    }
}

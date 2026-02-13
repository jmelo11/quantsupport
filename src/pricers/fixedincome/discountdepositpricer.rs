use crate::{
    ad::{
        adreal::{ADReal, Expr, IsReal},
        tape::Tape,
    },
    core::{
        assets::AssetType,
        contextmanager::ContextManager,
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        pricer::Pricer,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::fixedincome::deposit::DepositTrade,
    rates::{
        interest_rate_curve::InterestRateCurveAsset,
        yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    },
    utils::errors::{AtlasError, Result},
};

/// # `DiscountDepositPricer`
///
/// Implementation of pricer for deposit instruments.
pub struct DiscountDepositPricer;

/// # `DepositPriceEvaluationState`
///
/// Holds state information for deposit price evaluation.
#[derive(Default)]
struct DepositPriceEvaluationState {
    /// Price placeholder for perfomance reasons.
    pub price: Option<ADReal>,
    pub view: Option<MarketView>,
}

impl HandleValue<DepositTrade, DepositPriceEvaluationState> for DiscountDepositPricer {
    fn handle_value(
        &self,
        trade: &DepositTrade,
        ctx: &ContextManager,
        state: &mut DepositPriceEvaluationState,
    ) -> Result<f64> {
        let deposit = trade.instrument();
        let request: ViewRequest = trade.data_request();

        Tape::start_recording();
        Tape::set_mark();

        let view: MarketView = ctx.handle_request(request);
        view.inputs().add_to_tape()?;

        let df: ADReal = view.df(); // this could have multiple values, what happens with montecarlo?
        let fx: ADReal = view.fx();
        let amount: f64 = trade.notional() * deposit.final_payment().unwrap() / deposit.units();
        let value = df * fx * amount;
        state.price = Some(value.into());
        state.view = Some(view.into());

        Tape::stop_recording();
        Tape::rewind_to_mark();
        Ok((value).into())
    }
}

impl HandleSensitivities<DepositTrade, DepositPriceEvaluationState> for DiscountDepositPricer {
    fn handle_sensitivities(
        &self,
        _trade: &DepositTrade,
        _ctx: &ContextManager,
        state: &mut DepositPriceEvaluationState,
    ) -> Result<SensitivityMap> {
        match state.price {
            Some(price) => {
                let _ = price.backward()?;
                let mut ids: Vec<String> = Vec::new();
                let mut exposures: Vec<f64> = Vec::new();
                for component in state.view.unwrap() {
                    ids.push(component.id);
                    let s: f64 = component.value.adjoint()?;
                    exposures.push(s);
                }
                let sensitivities = SensitivityMap::new()
                    .with_instrument_keys(ids)
                    .with_exposure(exposures);
                Ok(sensitivities)
            }
            None => Err(AtlasError::ValueNotSetErr("Pricing not requested".into())),
        }
    }
}

impl Pricer for DiscountDepositPricer {
    type Item = DepositTrade;

    fn evaluate(
        &self,
        trade: &DepositTrade,
        requests: &[Request],
        ctx: &ContextManager,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let depo = trade.instrument();
        let identifier = depo.identifier();

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = DepositPriceEvaluationState::default();

        for request in requests {
            match request {
                Request::Value => {
                    let price = self.handle_value(trade, ctx, &mut state)?;
                    results = results.with_price(price);
                }

                Request::Sensitivities => {
                    let sensitivities = self.handle_sensitivities(trade, ctx, &mut state)?;
                    results = results.with_sensitivities(sensitivities);
                }
                _ => {}
            }
        }

        Ok(results)
    }
}

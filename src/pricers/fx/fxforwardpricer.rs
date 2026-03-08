use crate::{
    ad::{
        adreal::{ADReal, IsReal},
        tape::Tape,
    },
    core::{
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        marketdatahandling::marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        pillars::Pillars,
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    currencies::currency::Currency,
    instruments::fx::fxforward::FxForwardTrade,
    utils::errors::{QSError, Result},
};

/// Pricer for FX forward quotes.
///
/// The model quote is computed as
/// `F = S * DF_quote(T) / DF_base(T)`.
#[derive(Debug, Clone, Default)]
pub struct FxForwardPricer;

impl FxForwardPricer {
    /// Creates a new [`FxForwardPricer`].
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[derive(Default)]
struct FxForwardState {
    value: Option<ADReal>,
    market_data: Option<MarketData>,
}

impl PricerState for FxForwardState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl FxForwardState {
    fn resolve_curve_index_for_currency(
        &self,
        ccy: Currency,
    ) -> Result<crate::indices::marketindex::MarketIndex> {
        let md = self
            .get_market_data_reponse()
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?;

        let mut matches = md
            .constructed_elements()
            .discount_curves()
            .iter()
            .filter(|(_, elem)| elem.currency() == ccy)
            .map(|(idx, _)| idx.clone());

        let first = matches.next().ok_or_else(|| {
            QSError::NotFoundErr(format!("No discount curve found for currency {ccy}"))
        })?;

        if matches.next().is_some() {
            return Err(QSError::InvalidValueErr(format!(
                "Multiple discount curves found for currency {ccy}; cannot disambiguate"
            )));
        }

        Ok(first)
    }
}

impl HandleValue<FxForwardTrade, FxForwardState> for FxForwardPricer {
    fn handle_value(&self, trade: &FxForwardTrade, state: &mut FxForwardState) -> Result<f64> {
        Tape::start_recording();
        Tape::set_mark();
        state.put_pillars_on_tape()?;

        let inst = trade.instrument();
        let base = inst.base_currency();
        let quote = inst.quote_currency();

        let base_idx = state.resolve_curve_index_for_currency(base)?;
        let quote_idx = state.resolve_curve_index_for_currency(quote)?;

        let df_base = state
            .get_discount_curve_element(&base_idx)?
            .curve()
            .discount_factor(inst.delivery_date())?;
        let df_quote = state
            .get_discount_curve_element(&quote_idx)?
            .curve()
            .discount_factor(inst.delivery_date())?;

        let spot = state.get_exchange_rate(base, quote)?;
        let forward = (spot * df_quote / df_base).into();
        state.value = Some(forward);

        Tape::stop_recording();
        Ok(forward.value())
    }
}

impl HandleSensitivities<FxForwardTrade, FxForwardState> for FxForwardPricer {
    fn handle_sensitivities(
        &self,
        trade: &FxForwardTrade,
        state: &mut FxForwardState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(v) = state.value {
            v
        } else {
            let _ = self.handle_value(trade, state)?;
            state
                .value
                .ok_or_else(|| QSError::UnexpectedErr("Missing value in FX forward state".into()))?
        };

        value.backward_to_mark()?;

        let inst = trade.instrument();
        let base_idx = state.resolve_curve_index_for_currency(inst.base_currency())?;
        let quote_idx = state.resolve_curve_index_for_currency(inst.quote_currency())?;

        let mut ids = Vec::new();
        let mut exposures = Vec::new();
        for idx in [base_idx, quote_idx] {
            let element = state.get_discount_curve_element(&idx)?;
            for (label, value) in element.curve().pillars().into_iter().flatten() {
                ids.push(label);
                exposures.push(value.adjoint().unwrap_or(0.0));
            }
        }

        if let Some(store) = state.get_exchange_rate_store() {
            for (label, value) in store.pillars().into_iter().flatten() {
                ids.push(label);
                exposures.push(value.adjoint().unwrap_or(0.0));
            }
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures))
    }
}

impl Pricer for FxForwardPricer {
    type Item = FxForwardTrade;
    type Policy = ();

    fn evaluate(
        &self,
        trade: &FxForwardTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let identifier = trade.instrument().identifier();
        let md_request = self.market_data_request(trade).ok_or_else(|| {
            QSError::InvalidValueErr("Missing market-data request for FX forward".into())
        })?;

        let mut state = FxForwardState {
            value: None,
            market_data: Some(ctx.handle_request(&md_request)?),
        };

        let mut out = EvaluationResults::new(eval_date, identifier);
        for req in requests {
            match req {
                Request::Value => out = out.with_price(self.handle_value(trade, &mut state)?),
                Request::Sensitivities => {
                    out = out.with_sensitivities(self.handle_sensitivities(trade, &mut state)?)
                }
                _ => {}
            }
        }

        Ok(out)
    }

    fn market_data_request(&self, _trade: &FxForwardTrade) -> Option<MarketDataRequest> {
        Some(MarketDataRequest::default().with_exchange_rates())
    }

    fn set_discount_policy(&mut self, _policy: Box<Self::Policy>) {
        // No-op: FxForwardPricer does not use a discount policy.
    }

    fn discount_policy(&self) -> Option<&Self::Policy> {
        None
    }
}

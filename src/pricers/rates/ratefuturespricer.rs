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
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::rates::ratefutures::RateFuturesTrade,
    utils::errors::{QSError, Result},
};

/// Pricer for rate futures quotes.
///
/// The model quote is computed as `100 - 100 * F`, where `F` is the forward
/// rate implied by the reference discount curve over the contract accrual period.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let pricer = RateFuturesPricer::new();
///
/// // Build the instrument:
/// let rate_futures = MakeRateFutures::default()
///     .with_identifier("SR3-M24".to_string())
///     .with_market_index(MarketIndex::SOFR)
///     .with_start_date(Date::new(2024, 3, 20))
///     .with_end_date(Date::new(2024, 6, 20))
///     .with_futures_price(95.25)
///     .build()
///     .expect("failed to build rate futures");
///
/// // Wrap in a trade and evaluate with a MarketDataProvider:
/// //   let trade = RateFuturesTrade::new(rate_futures, Date::new(2024, 1, 1), 1.0);
/// //   let results = pricer.evaluate(&trade, &[Request::Value], &ctx);
/// ```
#[derive(Debug, Clone, Default)]
pub struct RateFuturesPricer;

impl RateFuturesPricer {
    /// Creates a new [`RateFuturesPricer`].
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[derive(Default)]
struct RateFuturesState {
    value: Option<ADReal>,
    market_data: Option<MarketData>,
}

impl PricerState for RateFuturesState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl HandleValue<RateFuturesTrade, RateFuturesState> for RateFuturesPricer {
    fn handle_value(&self, trade: &RateFuturesTrade, state: &mut RateFuturesState) -> Result<f64> {
        Tape::start_recording();
        Tape::set_mark();
        state.put_pillars_on_tape()?;

        let inst = trade.instrument();
        let rd = inst.rate_definition();
        let quote: ADReal = {
            let curve = state
                .get_discount_curve_element(&inst.market_index())?
                .curve();
            let fwd = curve.forward_rate(
                inst.start_date(),
                inst.end_date(),
                rd.compounding(),
                rd.frequency(),
            )?;
            (ADReal::new(100.0) - fwd * 100.0).into()
        };
        state.value = Some(quote);

        Tape::stop_recording();
        Ok(quote.value())
    }
}

impl HandleSensitivities<RateFuturesTrade, RateFuturesState> for RateFuturesPricer {
    fn handle_sensitivities(
        &self,
        trade: &RateFuturesTrade,
        state: &mut RateFuturesState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(v) = state.value {
            v
        } else {
            let _ = self.handle_value(trade, state)?;
            state
                .value
                .ok_or_else(|| QSError::UnexpectedErr("Missing value in futures state".into()))?
        };

        value.backward_to_mark()?;

        let element = state.get_discount_curve_element(&trade.instrument().market_index())?;
        let (ids, exposures): (Vec<_>, Vec<_>) = element
            .curve()
            .pillars()
            .into_iter()
            .flat_map(std::iter::IntoIterator::into_iter)
            .map(|(label, val)| (label, val.adjoint().ok()))
            .unzip();
        let exposures: Vec<f64> = exposures.into_iter().flatten().collect();

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures))
    }
}

impl Pricer for RateFuturesPricer {
    type Item = RateFuturesTrade;
    type Policy = ();

    fn evaluate(
        &self,
        trade: &RateFuturesTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let identifier = trade.instrument().identifier();

        let md_request = self.market_data_request(trade).ok_or_else(|| {
            QSError::InvalidValueErr("Missing market-data request for rate futures".into())
        })?;

        let mut state = RateFuturesState {
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

    fn market_data_request(&self, trade: &RateFuturesTrade) -> Option<MarketDataRequest> {
        Some(
            MarketDataRequest::default().with_constructed_elements_request(vec![
                ConstructedElementRequest::DiscountCurve {
                    market_index: trade.instrument().market_index(),
                },
            ]),
        )
    }

    fn set_discount_policy(&mut self, _policy: Box<Self::Policy>) {
        // No-op: RateFuturesPricer does not use a discount policy.
    }

    fn discount_policy(&self) -> Option<&Self::Policy> {
        None
    }
}

use crate::{
    ad::{dual::DualFwd, tape::Tape},
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
/// Value is a quoted price, not currency P&L: trade side, number of contracts,
/// contract multiplier, and the stored entry price do not scale it. No
/// futures/forward convexity adjustment is applied.
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
/// let trade = RateFuturesTrade::new(
///     rate_futures, Date::new(2024, 1, 1), 1.0, Side::LongReceive,
/// );
/// assert_eq!(trade.num_contracts(), 1.0);
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
    value: Option<DualFwd>,
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
        Tape::start_recording_fwd();
        Tape::set_mark_fwd();
        state.put_pillars_on_tape()?;

        let inst = trade.instrument();
        let rd = inst.rate_definition();
        let quote: DualFwd = {
            let curve = state
                .get_discount_curve_element(&inst.market_index())?
                .curve();
            let fwd = curve.forward_rate(
                inst.start_date(),
                inst.end_date(),
                rd.compounding(),
                rd.frequency(),
            )?;
            (DualFwd::new(100.0) - fwd * 100.0).into()
        };
        state.value = Some(quote);

        Tape::stop_recording_fwd();
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
        let exposures: Vec<f64> = exposures.into_iter().flatten().map(|v| v.value()).collect();

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures)
            .aggregate())
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

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use super::RateFuturesPricer;
    use crate::{
        ad::dual::DualFwd,
        core::{
            elements::curveelement::DiscountCurveElement,
            evaluationresults::EvaluationResults,
            marketdatahandling::{
                constructedelementstore::ConstructedElementStore,
                marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
            },
            pricer::Pricer,
            request::Request,
            trade::Side,
        },
        indices::marketindex::MarketIndex,
        instruments::rates::{makeratefutures::MakeRateFutures, ratefutures::RateFuturesTrade},
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::{
                flatforwardtermstructure::FlatForwardTermStructure,
                interestratestermstructure::InterestRatesTermStructure,
            },
        },
        time::date::Date,
        utils::errors::{QSError, Result},
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

    fn flat_curve(reference_date: Date, rate: f64) -> FlatForwardTermStructure<DualFwd> {
        FlatForwardTermStructure::new(
            reference_date,
            DualFwd::from(rate),
            RateDefinition::default(),
        )
        .with_pillar_label("rate".to_string())
    }

    /// Evaluates a rate futures trade on a flat curve at the given rate.
    fn evaluate_rate_futures(
        eval_date: Date,
        start_date: Date,
        end_date: Date,
        rate: f64,
        requests: &[Request],
    ) -> Result<EvaluationResults> {
        let market_index = MarketIndex::SOFR;
        let curve = flat_curve(eval_date, rate);

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            market_index.clone(),
            DiscountCurveElement::new(market_index.clone(), Rc::new(RefCell::new(curve))),
        );
        let market_data = MarketData::new(HashMap::new(), constructed_elements);

        let futures = MakeRateFutures::default()
            .with_identifier("SR3-TEST".to_string())
            .with_market_index(market_index)
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_futures_price(95.0)
            .build()?;
        let trade = RateFuturesTrade::new(futures, eval_date, 1.0, Side::LongReceive);

        let provider = SimpleMarketDataProvider {
            evaluation_date: eval_date,
            market_data,
        };
        RateFuturesPricer::new().evaluate(&trade, requests, &provider)
    }

    #[test]
    fn futures_quote_matches_flat_curve_forward() -> Result<()> {
        let eval_date = Date::new(2025, 1, 2);
        let start_date = Date::new(2025, 3, 19);
        let end_date = Date::new(2025, 6, 18);
        let rate = 0.04;

        let results =
            evaluate_rate_futures(eval_date, start_date, end_date, rate, &[Request::Value])?;
        let quote = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;

        // Closed form: quote = 100 - 100 * F where F is the curve forward.
        let futures = MakeRateFutures::default()
            .with_identifier("SR3-TEST".to_string())
            .with_market_index(MarketIndex::SOFR)
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_futures_price(95.0)
            .build()?;
        let rd = futures.rate_definition();
        let fwd = flat_curve(eval_date, rate)
            .forward_rate(start_date, end_date, rd.compounding(), rd.frequency())?
            .value();
        let expected = 100.0f64.mul_add(-fwd, 100.0);

        assert!(
            (quote - expected).abs() < 1e-10,
            "Futures quote {quote} should match closed form {expected}"
        );
        Ok(())
    }

    /// Boundary test: a zero-rate curve must imply a par quote of exactly 100.
    #[test]
    fn zero_rate_curve_implies_par_quote() -> Result<()> {
        let eval_date = Date::new(2025, 1, 2);
        let results = evaluate_rate_futures(
            eval_date,
            Date::new(2025, 3, 19),
            Date::new(2025, 6, 18),
            0.0,
            &[Request::Value],
        )?;
        let quote = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;
        assert!(
            (quote - 100.0).abs() < 1e-10,
            "Zero-rate quote should be 100, got {quote}"
        );
        Ok(())
    }

    /// Ladder test: across a ladder of curve levels, the AD rate sensitivity
    /// must match a central finite-difference bump of the flat rate.
    #[test]
    fn rate_sensitivity_ladder_matches_finite_difference() -> Result<()> {
        let eval_date = Date::new(2025, 1, 2);
        let start_date = Date::new(2025, 3, 19);
        let end_date = Date::new(2025, 6, 18);
        let bump = 1e-6;

        for rate in [0.0, 0.01, 0.02, 0.03, 0.05, 0.10] {
            let results = evaluate_rate_futures(
                eval_date,
                start_date,
                end_date,
                rate,
                &[Request::Value, Request::Sensitivities],
            )?;
            let sensitivities = results
                .sensitivities()
                .ok_or_else(|| QSError::UnexpectedErr("Missing sensitivities".into()))?;
            let ad_sens = sensitivities
                .instrument_keys()
                .iter()
                .zip(sensitivities.exposure().iter().copied())
                .find(|(key, _)| key.as_str() == "rate")
                .map(|(_, exposure)| exposure)
                .ok_or_else(|| QSError::NotFoundErr("Rate sensitivity not found".into()))?;

            let quote_at = |r: f64| -> Result<f64> {
                evaluate_rate_futures(eval_date, start_date, end_date, r, &[Request::Value])?
                    .price()
                    .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))
            };
            let fd = (quote_at(rate + bump)? - quote_at(rate - bump)?) / (2.0 * bump);

            assert!(
                (ad_sens - fd).abs() < 1e-4,
                "AD sensitivity {ad_sens} vs FD {fd} mismatch at rate {rate}"
            );
        }
        Ok(())
    }
}

//! Monte Carlo pricer for European equity options.
//!
//! Consumes a pre-built simulation element (see
//! [`SimulationBuilder`](crate::simulations::simulationbuilder::SimulationBuilder)).

use crate::{
    ad::{dual::DualFwd, tape::Tape},
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
    instruments::equity::equityeuropeanoption::{EquityEuropeanOptionTrade, EuroOptionType},
    utils::errors::{QSError, Result},
};

/// State struct for storing intermediate values during Monte Carlo pricing of an equity option.
#[derive(Default)]
struct MonteCarloState {
    value: Option<DualFwd>,
    market_data: Option<MarketData>,
}

impl PricerState for MonteCarloState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

/// A Monte Carlo pricer for European equity options.
///
/// The pricer consumes a pre-generated simulation of the underlying (built by
/// the [`SimulationBuilder`](crate::simulations::simulationbuilder::SimulationBuilder)
/// from a model configuration, e.g. Brownian motion with a constant or
/// surface-implied volatility) and averages the discounted payoff over the
/// simulated spot levels at the simulation date closest to expiry.
///
/// When a [`DiscountPolicy`] is set, the pricer uses the CSA discount curve
/// for payment discounting instead of the instrument's `market_index` curve.
pub struct BlackMCEuropeanOptionPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl BlackMCEuropeanOptionPricer {
    /// Creates a new [`BlackMCEuropeanOptionPricer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            discount_policy: None,
        }
    }
}

impl Default for BlackMCEuropeanOptionPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl HandleValue<EquityEuropeanOptionTrade, MonteCarloState> for BlackMCEuropeanOptionPricer {
    fn handle_value(
        &self,
        trade: &EquityEuropeanOptionTrade,
        state: &mut MonteCarloState,
    ) -> Result<f64> {
        let option = trade.instrument();
        let expiry = option.expiry_date();
        let index = option.market_index().clone();
        let discount_index = if let Some(policy) = &self.discount_policy {
            policy.accept(option)?
        } else {
            index.clone()
        };

        Tape::start_recording_fwd();
        Tape::set_mark_fwd();

        state.put_pillars_on_tape()?;

        let simulation_element = state.get_simulation_element(&index)?.clone();
        let simulation = simulation_element.simulation().borrow();

        // Pick the simulation date closest to expiry.
        let date_idx = simulation
            .dates()
            .iter()
            .enumerate()
            .min_by_key(|(_, d)| (**d - expiry).abs())
            .map(|(i, _)| i)
            .ok_or_else(|| QSError::InvalidValueErr("Simulation contains no dates".into()))?;

        let paths = simulation.path();
        if paths.is_empty() {
            return Err(QSError::InvalidValueErr(
                "Simulation contains no paths".into(),
            ));
        }
        let terminal_spots: Vec<DualFwd> = paths
            .iter()
            .map(|path| {
                path.get(date_idx).copied().ok_or_else(|| {
                    QSError::InvalidValueErr("Simulation path shorter than date grid".into())
                })
            })
            .collect::<Result<_>>()?;

        // Resolve the strike using the mean terminal level as forward proxy.
        #[allow(clippy::cast_precision_loss)]
        let n = terminal_spots.len() as f64;
        let mean_terminal: f64 = terminal_spots.iter().map(DualFwd::value).sum::<f64>() / n;
        let strike = option.strike().resolve(mean_terminal);
        let strike_ad = DualFwd::new(strike);
        let is_call = matches!(option.option_type(), EuroOptionType::Call);

        let mut payoff_sum = DualFwd::zero();
        for terminal in terminal_spots {
            let intrinsic: DualFwd = if is_call {
                (terminal - strike_ad).into()
            } else {
                (strike_ad - terminal).into()
            };
            let payoff = intrinsic.max(DualFwd::zero());
            payoff_sum = (payoff_sum + payoff).into();
        }
        let mean_payoff: DualFwd = (payoff_sum / DualFwd::new(n)).into();

        let df = state
            .get_discount_curve_element(&discount_index)?
            .curve()
            .discount_factor(expiry)?;

        let value: DualFwd = (df * mean_payoff * trade.notional()).into();
        state.value = Some(value);
        Tape::stop_recording_fwd();
        Ok(value.value())
    }
}

impl HandleSensitivities<EquityEuropeanOptionTrade, MonteCarloState>
    for BlackMCEuropeanOptionPricer
{
    fn handle_sensitivities(
        &self,
        trade: &EquityEuropeanOptionTrade,
        state: &mut MonteCarloState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(value) = state.value {
            value
        } else {
            let _ = self.handle_value(trade, state)?;
            state.value.ok_or_else(|| {
                QSError::UnexpectedErr(
                    "State does not contain price, although it was requested.".into(),
                )
            })?
        };

        value.backward_to_mark()?;
        let option = trade.instrument();
        let index = option.market_index();
        let policy_discount_index = if let Some(policy) = &self.discount_policy {
            Some(policy.accept(option)?)
        } else {
            None
        };
        let discount_index = policy_discount_index.as_ref().unwrap_or(index);

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        for (label, pillar) in state
            .get_discount_curve_element(discount_index)?
            .curve()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?.value());
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures)
            .aggregate())
    }
}

impl Pricer for BlackMCEuropeanOptionPricer {
    type Item = EquityEuropeanOptionTrade;
    type Policy = dyn DiscountPolicy;
    fn evaluate(
        &self,
        trade: &EquityEuropeanOptionTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let option = trade.instrument();
        let identifier = option.identifier();

        let md_request = self
            .market_data_request(trade)
            .ok_or_else(|| QSError::InvalidValueErr("Missing market data request".into()))?;

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = MonteCarloState {
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

    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest> {
        let option = trade.instrument();
        let index = option.market_index().clone();
        let mut elements = vec![
            ConstructedElementRequest::Simulation {
                market_index: index.clone(),
            },
            ConstructedElementRequest::DiscountCurve {
                market_index: index.clone(),
            },
        ];

        if let Some(policy) = &self.discount_policy {
            let collateral_index = policy.accept(option).ok()?;
            if collateral_index != index {
                elements.push(ConstructedElementRequest::DiscountCurve {
                    market_index: collateral_index,
                });
            }
        }

        let request = MarketDataRequest::default().with_constructed_elements_request(elements);
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
    use std::{
        cell::RefCell,
        collections::{BTreeMap, HashMap},
        rc::Rc,
    };

    use super::*;
    use crate::{
        core::{
            elements::curveelement::DiscountCurveElement,
            marketdatahandling::constructedelementstore::ConstructedElementStore, trade::Side,
        },
        indices::marketindex::MarketIndex,
        instruments::equity::equityeuropeanoption::EquityEuropeanOption,
        math::interpolation::interpolator::Interpolator,
        models::{
            brownianmotion::BrownianMotion,
            modelconfiguration::{
                LognormalModelParameters, ModelConfiguration, ParameterSource,
                SimulationConfiguration,
            },
        },
        quotes::{fixingstore::FixingStore, quote::Level, quotestore::QuoteStore},
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        simulations::simulationbuilder::SimulationBuilder,
        time::{
            date::Date,
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        volatility::volatilityindexing::Strike,
    };

    struct SimpleMarketDataProvider {
        evaluation_date: Date,
        market_data: MarketData,
    }

    impl MarketDataProvider for SimpleMarketDataProvider {
        fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
            Ok(self.market_data.clone())
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    #[test]
    fn mc_price_converges_to_black_closed_form() -> Result<()> {
        let reference_date = Date::new(2025, 1, 2);
        let expiry = reference_date + Period::new(1, TimeUnit::Years);
        let index = MarketIndex::Equity("SPX".to_string());
        let spot = 100.0_f64;
        let rate = 0.03_f64;
        let vol = 0.20_f64;
        let dc = DayCounter::Actual365;

        // Discount curve (AD).
        let dates = vec![
            reference_date,
            expiry,
            reference_date + Period::new(5, TimeUnit::Years),
        ];
        let dfs: Vec<DualFwd> = dates
            .iter()
            .map(|d| DualFwd::new((-rate * dc.year_fraction(reference_date, *d)).exp()))
            .collect();
        let curve =
            DiscountTermStructure::<DualFwd>::new(dates, dfs, dc, Interpolator::LogLinear, true)?
                .with_pillar_labels(vec![
                "df_0d".to_string(),
                "df_1y".to_string(),
                "df_5y".to_string(),
            ])?;
        let mut store = ConstructedElementStore::default();
        store.discount_curves_mut().insert(
            index.clone(),
            DiscountCurveElement::new(index.clone(), Rc::new(RefCell::new(curve))),
        );

        // Simulation from a JSON-style configuration.
        let quotes = QuoteStore::new(reference_date);
        let mut fixing_store = FixingStore::default();
        fixing_store.add_fixing(&index, reference_date, spot);
        let builder = SimulationBuilder::new(vec![SimulationConfiguration::new(
            index.clone(),
            ModelConfiguration::BrownianMotion {
                parameter_source: ParameterSource::Fixed(LognormalModelParameters::new(vol)),
                dividend_rate: None,
            },
            50_000,
            42,
            Period::new(1, TimeUnit::Years),
            Frequency::Monthly,
        )]);
        let simulations = builder.build(&store, &quotes, &fixing_store, Level::Mid)?;
        for (idx, simulation) in simulations {
            store.simulations_mut().insert(idx, simulation);
        }

        let fixings = HashMap::from([(index.clone(), BTreeMap::from([(reference_date, spot)]))]);
        let provider = SimpleMarketDataProvider {
            evaluation_date: reference_date,
            market_data: MarketData::new(fixings, store),
        };

        let option = EquityEuropeanOption::new(
            index,
            expiry,
            Strike::Absolute(100.0),
            EuroOptionType::Call,
            "SPX_ATM_CALL".to_string(),
        );
        let trade = EquityEuropeanOptionTrade::new(option, 1.0, reference_date, Side::LongReceive);

        let pricer = BlackMCEuropeanOptionPricer::new();
        let results =
            pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)?;
        let mc_price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("No MC price".into()))?;

        // Closed-form Black reference on the same forward/discount.
        let tau = dc.year_fraction(reference_date, expiry);
        let df = (-rate * tau).exp();
        let fwd = spot / df;
        let bs_price = df * BrownianMotion::<f64>::closed_form_price(fwd, 100.0, vol, tau, true)?;

        assert!(
            (mc_price - bs_price).abs() / bs_price < 0.02,
            "MC price {mc_price:.4} should be within 2% of Black price {bs_price:.4}"
        );

        let sensitivities = results
            .sensitivities()
            .ok_or_else(|| QSError::UnexpectedErr("No sensitivities".into()))?;
        assert!(
            !sensitivities.instrument_keys().is_empty(),
            "discount curve sensitivities expected"
        );
        Ok(())
    }
}

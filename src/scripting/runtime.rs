//! Model-backed scripting runtime.

use std::collections::HashMap;

use crate::{
    ad::dual::DualFwd,
    core::marketdatahandling::discountrequest::DiscountRequest,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    scripting::{
        data::simulationdata::{Scenario, SimulationData},
        nodes::{event::EventStream, traits::NodeVisitor},
        request::SimulationDataRequest,
        utils::errors::{Result, ScriptingError},
        visitors::{
            domainprocessor::DomainProcessor,
            evaluator::{Evaluator, SingleScenarioEvaluator, Value},
            fuzzyevaluator::FuzzyEvaluator,
            ifconditiontransform::IfConditionTransform,
            ifprocessor::IfProcessor,
            varindexer::VarIndexer,
        },
    },
    time::date::Date,
    xva::visitors::{
        marketmodel::{MarketModel, PathScenario, SimulationResponse},
        preprocessorexecutor::SimulationRequest,
    },
};

#[derive(Clone, Copy)]
struct ResponseRange {
    start: usize,
    len: usize,
}

#[derive(Clone, Copy)]
struct EventResponseMap {
    discounts: ResponseRange,
    forwards: ResponseRange,
    fx: ResponseRange,
}

/// Compiled event stream ready to run on an existing market model.
///
/// Compilation parses no market data and creates no model. It only assigns
/// variable and response indices and translates the script's requirements to
/// the request types shared by the rest of QuantSupport.
pub struct ScriptEngine {
    events: EventStream,
    reference_date: Date,
    local_currency: Currency,
    local_discount_index: MarketIndex,
    requests: Vec<SimulationDataRequest>,
    model_requests: Vec<SimulationRequest>,
    response_maps: Vec<EventResponseMap>,
    variable_indexes: HashMap<String, usize>,
    n_variables: usize,
    max_nested_ifs: usize,
}

impl ScriptEngine {
    /// Indexes an event stream and compiles its market-data requests.
    ///
    /// # Errors
    /// Returns an error if a financial expression cannot be indexed.
    pub fn new(
        mut events: EventStream,
        reference_date: Date,
        local_currency: Currency,
        local_discount_index: MarketIndex,
    ) -> Result<Self> {
        let event_dates = events.event_dates();
        if event_dates.is_empty() {
            return Err(ScriptingError::InvalidOperation(
                "a script must contain at least one event".to_string(),
            ));
        }
        if event_dates.iter().any(|date| *date < reference_date) {
            return Err(ScriptingError::InvalidOperation(
                "scripted event dates cannot precede the reference date".to_string(),
            ));
        }
        if event_dates.windows(2).any(|dates| dates[0] > dates[1]) {
            return Err(ScriptingError::InvalidOperation(
                "scripted events must be ordered by date".to_string(),
            ));
        }

        let indexer = VarIndexer::new()
            .with_local_currency(local_currency)
            .with_local_market_index(local_discount_index.clone());
        indexer.visit_events(&mut events)?;
        let requests = indexer.get_request();
        let (model_requests, response_maps) = flatten_requests(&requests);
        let n_variables = indexer.get_variables_size();

        let condition_transform = IfConditionTransform::new();
        for event in events.mut_events() {
            condition_transform.visit(event.mut_expr());
        }

        let if_processor = IfProcessor::new();
        if_processor.visit_events(&mut events)?;
        let max_nested_ifs = if_processor.max_nested_ifs();

        let domain_processor = DomainProcessor::new(n_variables);
        for event in events.mut_events() {
            domain_processor.visit(event.mut_expr())?;
        }

        Ok(Self {
            events,
            reference_date,
            local_currency,
            local_discount_index,
            requests,
            model_requests,
            response_maps,
            variable_indexes: indexer.get_variable_indexes(),
            n_variables,
            max_nested_ifs,
        })
    }

    /// Returns the indexed event stream.
    #[must_use]
    pub const fn events(&self) -> &EventStream {
        &self.events
    }

    /// Returns the per-event scripting requests.
    #[must_use]
    pub fn requests(&self) -> &[SimulationDataRequest] {
        &self.requests
    }

    /// Returns the flattened requests consumed by a market model.
    #[must_use]
    pub fn model_requests(&self) -> &[SimulationRequest] {
        &self.model_requests
    }

    /// Returns the script reference date.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Returns the last event or requested cashflow date.
    #[must_use]
    pub fn maturity(&self) -> Date {
        let event_dates = self.events.event_dates();
        let request_dates = self.requests.iter().flat_map(|request| {
            request
                .dfs()
                .iter()
                .map(DiscountRequest::date)
                .chain(request.fwds().iter().flat_map(|forward| {
                    [
                        forward.fixing_date(),
                        forward.start_date().unwrap_or(forward.fixing_date()),
                        forward.end_date().unwrap_or(forward.fixing_date()),
                    ]
                }))
                .chain(request.fxs().iter().filter_map(|fx| fx.date()))
        });
        event_dates
            .into_iter()
            .chain(request_dates)
            .max()
            .unwrap_or(self.reference_date)
    }

    /// Returns whether the compiled script defines `name`.
    #[must_use]
    pub fn has_variable(&self, name: &str) -> bool {
        self.variable_indexes.contains_key(name)
    }

    /// Returns the local reporting currency.
    #[must_use]
    pub const fn local_currency(&self) -> Currency {
        self.local_currency
    }

    /// Generates and evaluates every path exposed by `model`.
    ///
    /// The model is configured with the event dates and the same core request
    /// objects used by XVA. The returned numbers remain [`DualFwd`] values, so
    /// callers can run the normal AD backward pass on a selected result.
    ///
    /// # Errors
    /// Returns an error when path generation, response extraction, or script
    /// evaluation fails.
    pub fn evaluate(&self, model: &mut dyn MarketModel<DualFwd>) -> Result<HashMap<String, Value>> {
        let scenarios = self.generate_scenarios(model)?;
        if self.max_nested_ifs == 0 {
            Evaluator::new(self.n_variables, &scenarios)
                .visit_events(&self.events, &self.variable_indexes)
        } else {
            let results = scenarios
                .iter()
                .map(|scenario| {
                    FuzzyEvaluator::new(self.n_variables, self.max_nested_ifs)
                        .with_scenario(scenario)
                        .visit_events(&self.events, &self.variable_indexes)
                })
                .collect::<Result<Vec<_>>>()?;
            average_results(&results)
        }
    }

    /// Generates scripting scenarios without evaluating the event stream.
    ///
    /// # Errors
    /// Returns an error if the model cannot produce a path or a requested
    /// response is absent.
    pub fn generate_scenarios(
        &self,
        model: &mut dyn MarketModel<DualFwd>,
    ) -> Result<Vec<Scenario>> {
        if model.n_paths() == 0 {
            return Err(ScriptingError::EvaluationError(
                "market model exposes no paths".to_string(),
            ));
        }
        model.set_evaluation_dates(self.events.event_dates());
        model.set_requests(self.model_requests.clone());

        let numeraires = self.event_numeraires(model)?;
        (0..model.n_paths())
            .map(|path_index| {
                let path = model.generate_path(path_index).ok_or_else(|| {
                    ScriptingError::EvaluationError(format!(
                        "market model failed to generate path {path_index}"
                    ))
                })?;
                self.scenario_from_path(&path, &numeraires)
            })
            .collect()
    }

    /// Evaluates one indexed payment from a contingent claim's response block.
    pub(crate) fn evaluate_payment(
        &self,
        payment_id: usize,
        valuation_date: Date,
        responses: &[SimulationResponse<DualFwd>],
    ) -> Result<DualFwd> {
        let scenario = self.scenario_from_responses(responses)?;
        if self.max_nested_ifs == 0 {
            let evaluator = SingleScenarioEvaluator::new()
                .with_variables(self.n_variables)
                .with_scenario(&scenario)
                .with_valuation_date(valuation_date)
                .with_payment_capture(payment_id);
            evaluator.visit_events(&self.events, &self.variable_indexes)?;
            Ok(evaluator
                .captured_payment_value()
                .unwrap_or_else(DualFwd::zero))
        } else {
            let evaluator = FuzzyEvaluator::new(self.n_variables, self.max_nested_ifs)
                .with_scenario(&scenario)
                .with_valuation_date(valuation_date)
                .with_payment_capture(payment_id);
            evaluator.visit_events(&self.events, &self.variable_indexes)?;
            Ok(evaluator
                .captured_payment_value()
                .unwrap_or_else(DualFwd::zero))
        }
    }

    fn event_numeraires(&self, model: &dyn MarketModel<DualFwd>) -> Result<Vec<DualFwd>> {
        self.events
            .event_dates()
            .into_iter()
            .zip(&self.requests)
            .map(|(event_date, request)| {
                if !request.requires_numeraire() || event_date <= self.reference_date {
                    return Ok(DualFwd::one());
                }
                let request = DiscountRequest::new(self.local_discount_index.clone(), event_date);
                let discount = model.resolve_discount_request(self.reference_date, &request)?;
                Ok((DualFwd::one() / discount).into())
            })
            .collect()
    }

    fn scenario_from_path(
        &self,
        path: &PathScenario<DualFwd>,
        numeraires: &[DualFwd],
    ) -> Result<Scenario> {
        if path.len() != self.response_maps.len() {
            return Err(ScriptingError::EvaluationError(format!(
                "model returned {} dates for {} scripted events",
                path.len(),
                self.response_maps.len()
            )));
        }

        path.iter()
            .zip(self.response_maps.iter())
            .zip(numeraires.iter())
            .enumerate()
            .map(|(event_index, ((responses, mapping), numeraire))| {
                Ok(SimulationData::new(
                    *numeraire,
                    collect_responses(
                        responses,
                        mapping.discounts,
                        event_index,
                        "discount",
                        |response| response.discounts,
                    )?,
                    collect_responses(
                        responses,
                        mapping.forwards,
                        event_index,
                        "forward",
                        |response| response.forward_rates,
                    )?,
                    collect_responses(responses, mapping.fx, event_index, "FX", |response| {
                        response.fx_rates
                    })?,
                ))
            })
            .collect()
    }

    fn scenario_from_responses(
        &self,
        responses: &[SimulationResponse<DualFwd>],
    ) -> Result<Scenario> {
        self.response_maps
            .iter()
            .enumerate()
            .map(|(event_index, mapping)| {
                Ok(SimulationData::new(
                    DualFwd::one(),
                    collect_responses(
                        responses,
                        mapping.discounts,
                        event_index,
                        "discount",
                        |response| response.discounts,
                    )?,
                    collect_responses(
                        responses,
                        mapping.forwards,
                        event_index,
                        "forward",
                        |response| response.forward_rates,
                    )?,
                    collect_responses(responses, mapping.fx, event_index, "FX", |response| {
                        response.fx_rates
                    })?,
                ))
            })
            .collect()
    }
}

fn average_results(results: &[HashMap<String, Value>]) -> Result<HashMap<String, Value>> {
    if results.is_empty() {
        return Err(ScriptingError::EvaluationError(
            "cannot average an empty set of scripted scenarios".to_string(),
        ));
    }

    let n_scenarios = results.len() as f64;
    let mut averaged = HashMap::new();
    for result in results {
        for (name, value) in result {
            let entry = averaged
                .entry(name.clone())
                .or_insert(Value::Number(DualFwd::zero()));
            if let (Value::Number(total), Value::Number(value)) = (entry, value) {
                *total = (*total + *value / n_scenarios).into();
            }
        }
    }
    Ok(averaged)
}

fn flatten_requests(
    requests: &[SimulationDataRequest],
) -> (Vec<SimulationRequest>, Vec<EventResponseMap>) {
    let mut flattened = Vec::new();
    let mut maps = Vec::with_capacity(requests.len());

    for request in requests {
        let discount_start = flattened.len();
        flattened.extend(
            request
                .dfs()
                .iter()
                .cloned()
                .map(|discount_request| SimulationRequest {
                    discount_request: Some(discount_request),
                    ..SimulationRequest::default()
                }),
        );

        let forward_start = flattened.len();
        flattened.extend(request.fwds().iter().cloned().map(|forward_rate_request| {
            SimulationRequest {
                forward_rate_request: Some(forward_rate_request),
                ..SimulationRequest::default()
            }
        }));

        let fx_start = flattened.len();
        flattened.extend(
            request
                .fxs()
                .iter()
                .cloned()
                .map(|fx_request| SimulationRequest {
                    fx_request: Some(fx_request),
                    ..SimulationRequest::default()
                }),
        );

        maps.push(EventResponseMap {
            discounts: ResponseRange {
                start: discount_start,
                len: request.dfs().len(),
            },
            forwards: ResponseRange {
                start: forward_start,
                len: request.fwds().len(),
            },
            fx: ResponseRange {
                start: fx_start,
                len: request.fxs().len(),
            },
        });
    }

    (flattened, maps)
}

fn collect_responses(
    responses: &[SimulationResponse<DualFwd>],
    range: ResponseRange,
    event_index: usize,
    kind: &str,
    select: impl Fn(&SimulationResponse<DualFwd>) -> Option<DualFwd>,
) -> Result<Vec<DualFwd>> {
    (range.start..range.start + range.len)
        .map(|response_index| {
            responses
                .get(response_index)
                .and_then(&select)
                .ok_or_else(|| {
                    ScriptingError::EvaluationError(format!(
                        "missing {kind} response {response_index} for event {event_index}"
                    ))
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ad::{scalar::Scalar, tape::Tape},
        math::interpolation::interpolator::Interpolator,
        models::lgm::{lgmcomponents::LgmRateModel, lgmmarketmodel::LgmMarketModel},
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        scripting::nodes::event::CodedEvent,
        time::{daycounter::DayCounter, enums::TimeUnit},
    };

    #[test]
    fn evaluates_script_with_existing_lgm_model_and_requests() -> Result<()> {
        Tape::start_recording_fwd();

        let reference_date = Date::new(2025, 1, 1);
        let event_date = reference_date.advance(1, TimeUnit::Years);
        let final_date = reference_date.advance(2, TimeUnit::Years);
        let day_counter = DayCounter::Actual365;
        let rate = 0.05_f64;
        let event_discount_value =
            (-rate * day_counter.year_fraction(reference_date, event_date)).exp();
        let final_discount_value =
            (-rate * day_counter.year_fraction(reference_date, final_date)).exp();
        let event_discount = DualFwd::new(event_discount_value);
        let curve = DiscountTermStructure::<DualFwd>::new(
            vec![reference_date, event_date, final_date],
            vec![
                DualFwd::one(),
                event_discount,
                DualFwd::new(final_discount_value),
            ],
            day_counter,
            Interpolator::LogLinear,
            true,
        )?;
        let rate_model = LgmRateModel::new(DualFwd::scalar(0.05), DualFwd::zero(), &curve);
        let mut model = LgmMarketModel::new(
            Currency::USD,
            MarketIndex::SOFR,
            reference_date,
            day_counter,
        )
        .with_n_paths(4)
        .with_seed(7);
        model.add_curve_model(MarketIndex::SOFR, rate_model);

        let events = EventStream::try_from(vec![CodedEvent::new(
            event_date,
            "opt = 0; opt pays 100;".to_string(),
        )])?;
        let engine = ScriptEngine::new(events, reference_date, Currency::USD, MarketIndex::SOFR)?;
        let values = engine.evaluate(&mut model)?;
        let value = match values.get("opt") {
            Some(Value::Number(value)) => *value,
            other => {
                return Err(ScriptingError::EvaluationError(format!(
                    "expected numeric opt result, got {other:?}"
                )))
            }
        };

        assert!((value.value() - 100.0 * event_discount_value).abs() < 1.0e-10);
        value.backward()?;
        assert!((event_discount.adjoint()?.value() - 100.0).abs() < 1.0e-8);

        Tape::stop_recording_fwd();
        Ok(())
    }
}

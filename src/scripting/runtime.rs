/*
This file is part of QuantSupport's Rust rewrite and adaptation of the
derivatives scripting code written by Antoine Savine in 2018.

The original code is the strict intellectual property of Antoine Savine.

A license to use and alter the original code for personal and commercial
applications is freely granted to any person or company that purchased a copy
of the book:

Modern Computational Finance: Scripting for Derivatives and XVA
Jesper Andreasen and Antoine Savine
Wiley, 2018

This attribution and license notice must be preserved at the top of this file.
*/

//! Model-backed scripting runtime.
use std::{collections::HashMap, ops::Range};

use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    ad::{dual::DualFwd, tape::Tape},
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
            evaluator::{SingleScenarioEvaluator, Value},
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
    event_start: usize,
    discounts: ResponseRange,
    forwards: ResponseRange,
    forward_discounts: ResponseRange,
    fx: ResponseRange,
    spots: ResponseRange,
}

impl EventResponseMap {
    fn for_layout(self, compact: bool) -> Self {
        if !compact {
            return self;
        }
        let local = |range: ResponseRange| ResponseRange {
            start: range.start - self.event_start,
            len: range.len,
        };
        Self {
            event_start: 0,
            discounts: local(self.discounts),
            forwards: local(self.forwards),
            forward_discounts: local(self.forward_discounts),
            fx: local(self.fx),
            spots: local(self.spots),
        }
    }
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
    model_request_dates: Vec<Option<Date>>,
    response_maps: Vec<EventResponseMap>,
    variable_indexes: HashMap<String, usize>,
    n_variables: usize,
    max_nested_ifs: usize,
}

/// Callback used by [`ScriptModelSetup`] to expose a model whose AD leaves
/// belong to the calling Rayon worker's thread-local tape.
pub type ScriptModelCallback<'a, R> =
    dyn FnMut(&mut dyn MarketModel<DualFwd>, &[(String, DualFwd)]) -> Result<R> + 'a;

/// Factory for rebuilding a script market model on each Rayon worker.
///
/// A [`DualFwd`] value contains a pointer into a thread-local reverse-mode
/// tape. Consequently, parallel pricing must rebuild curves, model
/// parameters, and tracked leaves inside each worker instead of sharing one
/// model created on the caller's tape.
pub trait ScriptModelSetup: Send + Sync {
    /// Returns the total number of Monte Carlo paths.
    fn n_paths(&self) -> usize;

    /// Builds one model on the current worker's tape and invokes `callback`.
    ///
    /// `leaves` contains stable labels and the corresponding AD leaves. Their
    /// adjoints are reduced across workers by
    /// [`ScriptEngine::evaluate_parallel`].
    ///
    /// # Errors
    /// Returns an error if model construction or callback execution fails.
    fn with_model<R>(&self, callback: &mut ScriptModelCallback<'_, R>) -> Result<R>;
}

/// Values and first-order sensitivities produced by parallel script pricing.
#[derive(Debug)]
pub struct ParallelScriptEvaluation {
    /// Path-averaged numeric script variables.
    pub values: HashMap<String, f64>,
    /// First-order adjoints keyed by the labels supplied by
    /// [`ScriptModelSetup::with_model`].
    pub sensitivities: Vec<(String, f64)>,
}

struct ScriptChunkResult {
    values: HashMap<String, f64>,
    sensitivities: Vec<(String, f64)>,
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
        let (model_requests, model_request_dates, response_maps) =
            flatten_requests(&requests, &event_dates, &local_discount_index);
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
            model_request_dates,
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
                .chain(request.spots().iter().map(|spot| spot.date()))
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

    /// Generates and evaluates every path exposed by `model`, one path at a
    /// time, and returns the path-averaged value of every numeric script
    /// variable.
    ///
    /// The tape footprint is bounded: the tape is marked after the shared
    /// pre-path state (numeraires, curve pillars), then per path it is
    /// rewound to the mark, the path is generated and evaluated, and — when
    /// `result_variable` is given — `result / n_paths` is back-propagated to
    /// the mark before the path's tape segment is dropped. Peak memory is one
    /// path's tape, independent of `n_paths` (the same mark/rewind pattern as
    /// the XVA exposure evaluator).
    ///
    /// When `result_variable` is `Some`, the accumulated mark adjoints are
    /// propagated to the start of the tape after the loop, so leaves recorded
    /// before this call (curve pillar quotes, model parameters) expose
    /// `d(mean result)/d(leaf)` through [`adjoint`](DualFwd::adjoint).
    /// Callers must NOT run their own backward pass.
    ///
    /// # Errors
    /// Returns an error when `result_variable` is not defined by the script,
    /// path generation, response extraction or evaluation fails, or adjoint
    /// propagation fails.
    pub fn evaluate(
        &self,
        model: &mut dyn MarketModel<DualFwd>,
        result_variable: Option<&str>,
    ) -> Result<HashMap<String, f64>> {
        if let Some(name) = result_variable {
            if !self.variable_indexes.contains_key(name) {
                return Err(ScriptingError::EvaluationError(format!(
                    "result variable '{name}' is not defined by the script"
                )));
            }
        }
        let n_paths = model.n_paths();
        if n_paths == 0 {
            return Err(ScriptingError::EvaluationError(
                "market model exposes no paths".to_string(),
            ));
        }
        model.set_evaluation_dates(self.events.event_dates());
        model.set_requests(self.model_requests.clone());
        model.set_request_dates(self.model_request_dates.clone());
        let compact_responses = model.uses_compact_dated_requests();
        let numeraires = self.event_numeraires(model)?;
        let n_scenarios = n_paths as f64;

        // Estimate two martingale-control coefficients on a small pilot set:
        // discounted zero-coupon bonds and discounted forward payoffs. The
        // main pass then has the same path count and AAD behavior as before,
        // while its noisy linear rate component is anchored to curve-exact
        // expectations. Treating the fitted coefficients as constants keeps
        // the estimator stable and avoids differentiating the regression.
        let control_expectations = self.control_expectations(model)?;
        let use_controls = result_variable.is_some()
            && n_paths >= 16
            && self
                .response_maps
                .iter()
                .any(|mapping| mapping.discounts.len + mapping.forwards.len > 0);

        let mut averages: HashMap<String, f64> = HashMap::new();
        Tape::set_mark_fwd();
        let control_betas = if use_controls {
            let pilot_paths = n_paths.min(64);
            let mut pilot = Vec::with_capacity(pilot_paths);
            for pilot_offset in 0..pilot_paths {
                // Keep coefficient fitting disjoint from the reported path
                // set, avoiding the small in-sample control-variate bias.
                let path_index = n_paths + pilot_offset;
                Tape::rewind_to_mark_fwd();
                let path = model.generate_path(path_index).ok_or_else(|| {
                    ScriptingError::EvaluationError(format!(
                        "market model failed to generate pilot path {path_index}"
                    ))
                })?;
                let scenario = self.scenario_from_path(&path, &numeraires, compact_responses)?;
                let values = self.evaluate_scenario(&scenario)?;
                let result = result_variable
                    .and_then(|name| values.get(name))
                    .and_then(|value| match value {
                        Value::Number(number) => Some(number.value()),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        ScriptingError::EvaluationError(
                            "result variable did not produce a numeric pilot value".to_string(),
                        )
                    })?;
                let controls = self.path_controls(&path, &numeraires, compact_responses)?;
                pilot.push([result, controls[0].value(), controls[1].value()]);
            }
            estimate_control_betas(&pilot)
        } else {
            [0.0; 2]
        };
        let paths_result = (0..n_paths).try_for_each(|path_index| -> Result<()> {
            Tape::rewind_to_mark_fwd();
            let path = model.generate_path(path_index).ok_or_else(|| {
                ScriptingError::EvaluationError(format!(
                    "market model failed to generate path {path_index}"
                ))
            })?;
            let scenario = self.scenario_from_path(&path, &numeraires, compact_responses)?;
            let values = self.evaluate_scenario(&scenario)?;
            let adjusted_result: Option<DualFwd> = if use_controls {
                let controls = self.path_controls(&path, &numeraires, compact_responses)?;
                result_variable
                    .and_then(|name| values.get(name))
                    .and_then(|value| {
                        if let Value::Number(number) = value {
                            Some(
                                (*number
                                    - (controls[0] - control_expectations[0]) * control_betas[0]
                                    - (controls[1] - control_expectations[1]) * control_betas[1])
                                    .into(),
                            )
                        } else {
                            None
                        }
                    })
            } else {
                None
            };
            for (name, value) in &values {
                if let Value::Number(number) = value {
                    let path_value = if result_variable == Some(name.as_str()) {
                        adjusted_result.unwrap_or(*number)
                    } else {
                        *number
                    };
                    *averages.entry(name.clone()).or_insert(0.0) +=
                        path_value.value() / n_scenarios;
                }
            }
            if let Some(number) = adjusted_result.or_else(|| {
                result_variable
                    .and_then(|name| values.get(name))
                    .and_then(|value| match value {
                        Value::Number(number) => Some(*number),
                        _ => None,
                    })
            }) {
                let contribution: DualFwd = (number / n_scenarios).into();
                if contribution.is_on_tape() {
                    contribution.backward_to_mark()?;
                }
            }
            Ok(())
        });
        // Propagate whatever was accumulated and restore the mark even when a
        // path failed, so the caller's tape stays usable.
        let propagated = Tape::propagate_mark_to_start_fwd();
        Tape::reset_mark_fwd();
        paths_result?;
        propagated?;
        Ok(averages)
    }

    /// Evaluates Monte Carlo paths in parallel with one model and AD tape per
    /// Rayon worker.
    ///
    /// The setup rebuilds the model on every worker so no [`DualFwd`] node is
    /// shared across thread-local tapes. Paths are divided into contiguous,
    /// deterministic ranges, and worker values and adjoints are summed after
    /// all ranges complete. The normalization always uses the total path
    /// count, so results do not depend on the number of Rayon workers.
    ///
    /// # Errors
    /// Returns an error when the result variable is absent, setup/model path
    /// counts disagree, model construction fails, a path cannot be evaluated,
    /// or adjoint propagation fails.
    pub fn evaluate_parallel<S: ScriptModelSetup>(
        &self,
        setup: &S,
        result_variable: Option<&str>,
    ) -> Result<ParallelScriptEvaluation> {
        if let Some(name) = result_variable {
            if !self.variable_indexes.contains_key(name) {
                return Err(ScriptingError::EvaluationError(format!(
                    "result variable '{name}' is not defined by the script"
                )));
            }
        }
        let n_paths = setup.n_paths();
        if n_paths == 0 {
            return Err(ScriptingError::EvaluationError(
                "market model exposes no paths".to_string(),
            ));
        }
        let use_controls = self.uses_controls(result_variable, n_paths);
        let control_betas = if use_controls {
            self.fit_parallel_control_betas(setup, result_variable, n_paths)?
        } else {
            [0.0; 2]
        };

        let n_workers = rayon::current_num_threads().min(n_paths);
        let chunk_size = n_paths.div_ceil(n_workers);
        let chunks: Vec<Range<usize>> = (0..n_workers)
            .map(|worker| {
                let start = worker * chunk_size;
                start..(start + chunk_size).min(n_paths)
            })
            .filter(|range| !range.is_empty())
            .collect();

        let chunk_results: Vec<ScriptChunkResult> = chunks
            .into_par_iter()
            .map(|range| -> Result<ScriptChunkResult> {
                Tape::rewind_to_init_fwd();
                Tape::start_recording_fwd();
                let result = setup.with_model(&mut |model, leaves| {
                    self.configure_model(model, n_paths)?;
                    let numeraires = self.event_numeraires(model)?;
                    let control_expectations = if use_controls {
                        self.control_expectations(model)?
                    } else {
                        [DualFwd::zero(); 2]
                    };
                    Tape::set_mark_fwd();
                    let values = self.evaluate_path_range(
                        model,
                        result_variable,
                        range.clone(),
                        n_paths,
                        &numeraires,
                        use_controls,
                        control_expectations,
                        control_betas,
                    )?;
                    let sensitivities = leaves
                        .iter()
                        .filter_map(|(label, leaf)| {
                            leaf.adjoint()
                                .ok()
                                .map(|adjoint| (label.clone(), adjoint.value()))
                        })
                        .collect();
                    Ok(ScriptChunkResult {
                        values,
                        sensitivities,
                    })
                });
                Tape::stop_recording_fwd();
                result
            })
            .collect::<Result<Vec<_>>>()?;

        let mut values = HashMap::new();
        let mut sensitivity_map = HashMap::new();
        for chunk in chunk_results {
            for (name, value) in chunk.values {
                *values.entry(name).or_insert(0.0) += value;
            }
            for (label, sensitivity) in chunk.sensitivities {
                *sensitivity_map.entry(label).or_insert(0.0) += sensitivity;
            }
        }
        let mut sensitivities: Vec<_> = sensitivity_map.into_iter().collect();
        sensitivities.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(ParallelScriptEvaluation {
            values,
            sensitivities,
        })
    }

    fn uses_controls(&self, result_variable: Option<&str>, n_paths: usize) -> bool {
        result_variable.is_some()
            && n_paths >= 16
            && self
                .response_maps
                .iter()
                .any(|mapping| mapping.discounts.len + mapping.forwards.len > 0)
    }

    fn configure_model(&self, model: &mut dyn MarketModel<DualFwd>, n_paths: usize) -> Result<()> {
        if model.n_paths() != n_paths {
            return Err(ScriptingError::EvaluationError(format!(
                "model exposes {} paths but setup declares {n_paths}",
                model.n_paths()
            )));
        }
        model.set_evaluation_dates(self.events.event_dates());
        model.set_requests(self.model_requests.clone());
        model.set_request_dates(self.model_request_dates.clone());
        Ok(())
    }

    fn fit_parallel_control_betas<S: ScriptModelSetup>(
        &self,
        setup: &S,
        result_variable: Option<&str>,
        n_paths: usize,
    ) -> Result<[f64; 2]> {
        Tape::rewind_to_init_fwd();
        Tape::start_recording_fwd();
        let result = setup.with_model(&mut |model, _| {
            self.configure_model(model, n_paths)?;
            let numeraires = self.event_numeraires(model)?;
            let compact_responses = model.uses_compact_dated_requests();
            Tape::set_mark_fwd();
            let pilot_paths = n_paths.min(64);
            let mut pilot = Vec::with_capacity(pilot_paths);
            for pilot_offset in 0..pilot_paths {
                let path_index = n_paths + pilot_offset;
                Tape::rewind_to_mark_fwd();
                let path = model.generate_path(path_index).ok_or_else(|| {
                    ScriptingError::EvaluationError(format!(
                        "market model failed to generate pilot path {path_index}"
                    ))
                })?;
                let scenario = self.scenario_from_path(&path, &numeraires, compact_responses)?;
                let values = self.evaluate_scenario(&scenario)?;
                let result = result_variable
                    .and_then(|name| values.get(name))
                    .and_then(|value| match value {
                        Value::Number(number) => Some(number.value()),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        ScriptingError::EvaluationError(
                            "result variable did not produce a numeric pilot value".to_string(),
                        )
                    })?;
                let controls = self.path_controls(&path, &numeraires, compact_responses)?;
                pilot.push([result, controls[0].value(), controls[1].value()]);
            }
            Ok(estimate_control_betas(&pilot))
        });
        Tape::reset_mark_fwd();
        Tape::stop_recording_fwd();
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_path_range(
        &self,
        model: &dyn MarketModel<DualFwd>,
        result_variable: Option<&str>,
        path_range: Range<usize>,
        n_paths: usize,
        numeraires: &[DualFwd],
        use_controls: bool,
        control_expectations: [DualFwd; 2],
        control_betas: [f64; 2],
    ) -> Result<HashMap<String, f64>> {
        let compact_responses = model.uses_compact_dated_requests();
        let n_scenarios = n_paths as f64;
        let mut averages = HashMap::new();
        let paths_result = path_range
            .into_iter()
            .try_for_each(|path_index| -> Result<()> {
                Tape::rewind_to_mark_fwd();
                let path = model.generate_path(path_index).ok_or_else(|| {
                    ScriptingError::EvaluationError(format!(
                        "market model failed to generate path {path_index}"
                    ))
                })?;
                let scenario = self.scenario_from_path(&path, numeraires, compact_responses)?;
                let values = self.evaluate_scenario(&scenario)?;
                let adjusted_result = if use_controls {
                    let controls = self.path_controls(&path, numeraires, compact_responses)?;
                    result_variable
                        .and_then(|name| values.get(name))
                        .and_then(|value| {
                            if let Value::Number(number) = value {
                                Some(
                                    (*number
                                        - (controls[0] - control_expectations[0])
                                            * control_betas[0]
                                        - (controls[1] - control_expectations[1])
                                            * control_betas[1])
                                        .into(),
                                )
                            } else {
                                None
                            }
                        })
                } else {
                    None
                };
                for (name, value) in &values {
                    if let Value::Number(number) = value {
                        let path_value = if result_variable == Some(name.as_str()) {
                            adjusted_result.unwrap_or(*number)
                        } else {
                            *number
                        };
                        *averages.entry(name.clone()).or_insert(0.0) +=
                            path_value.value() / n_scenarios;
                    }
                }
                if let Some(number) = adjusted_result.or_else(|| {
                    result_variable.and_then(|name| values.get(name)).and_then(
                        |value| match value {
                            Value::Number(number) => Some(*number),
                            _ => None,
                        },
                    )
                }) {
                    let contribution: DualFwd = (number / n_scenarios).into();
                    if contribution.is_on_tape() {
                        contribution.backward_to_mark()?;
                    }
                }
                Ok(())
            });
        let propagated = Tape::propagate_mark_to_start_fwd();
        Tape::reset_mark_fwd();
        paths_result?;
        propagated?;
        Ok(averages)
    }

    fn evaluate_scenario(&self, scenario: &Scenario) -> Result<HashMap<String, Value>> {
        if self.max_nested_ifs == 0 {
            SingleScenarioEvaluator::new()
                .with_variables(self.n_variables)
                .with_scenario(scenario)
                .visit_events(&self.events, &self.variable_indexes)
        } else {
            FuzzyEvaluator::new(self.n_variables, self.max_nested_ifs)
                .with_scenario(scenario)
                .visit_events(&self.events, &self.variable_indexes)
        }
    }

    fn control_expectations(&self, model: &dyn MarketModel<DualFwd>) -> Result<[DualFwd; 2]> {
        let mut bonds = DualFwd::zero();
        let mut forwards = DualFwd::zero();
        for request in &self.requests {
            for discount in request.dfs() {
                bonds += model.resolve_discount_request(self.reference_date, discount)?;
            }
            for forward in request.fwds() {
                let forward_value =
                    model.resolve_forward_rate_request(self.reference_date, forward)?;
                let payment_date = forward.end_date().unwrap_or_else(|| forward.fixing_date());
                let discount = model.resolve_discount_request(
                    self.reference_date,
                    &DiscountRequest::new(self.local_discount_index.clone(), payment_date),
                )?;
                forwards += forward_value * discount;
            }
        }
        Ok([bonds, forwards])
    }

    fn path_controls(
        &self,
        path: &PathScenario<DualFwd>,
        numeraires: &[DualFwd],
        compact_responses: bool,
    ) -> Result<[DualFwd; 2]> {
        let mut bonds = DualFwd::zero();
        let mut forwards = DualFwd::zero();
        for (event_index, ((responses, mapping), fallback_numeraire)) in path
            .iter()
            .zip(&self.response_maps)
            .zip(numeraires)
            .enumerate()
        {
            let mapping = mapping.for_layout(compact_responses);
            let numeraire = responses
                .iter()
                .find_map(|response| response.numeraire)
                .unwrap_or(*fallback_numeraire);
            for response_index in
                mapping.discounts.start..mapping.discounts.start + mapping.discounts.len
            {
                let discount = responses
                    .get(response_index)
                    .and_then(|response| response.discounts)
                    .ok_or_else(|| {
                        ScriptingError::EvaluationError(format!(
                            "missing bond control response {response_index} for event {event_index}"
                        ))
                    })?;
                bonds += discount / numeraire;
            }
            for offset in 0..mapping.forwards.len {
                let forward_index = mapping.forwards.start + offset;
                let discount_index = mapping.forward_discounts.start + offset;
                let forward = responses
                    .get(forward_index)
                    .and_then(|response| response.forward_rates)
                    .ok_or_else(|| {
                        ScriptingError::EvaluationError(format!(
                            "missing forward control response {forward_index} for event {event_index}"
                        ))
                    })?;
                let discount = responses
                    .get(discount_index)
                    .and_then(|response| response.discounts)
                    .ok_or_else(|| {
                        ScriptingError::EvaluationError(format!(
                            "missing forward discount response {discount_index} for event {event_index}"
                        ))
                    })?;
                forwards += forward * discount / numeraire;
            }
        }
        Ok([bonds, forwards])
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
        model.set_request_dates(self.model_request_dates.clone());
        let compact_responses = model.uses_compact_dated_requests();

        let numeraires = self.event_numeraires(model)?;
        (0..model.n_paths())
            .map(|path_index| {
                let path = model.generate_path(path_index).ok_or_else(|| {
                    ScriptingError::EvaluationError(format!(
                        "market model failed to generate path {path_index}"
                    ))
                })?;
                self.scenario_from_path(&path, &numeraires, compact_responses)
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
        compact_responses: bool,
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
                let mapping = mapping.for_layout(compact_responses);
                let path_numeraire = responses
                    .iter()
                    .find_map(|response| response.numeraire)
                    .unwrap_or(*numeraire);
                Ok(SimulationData::new(
                    path_numeraire,
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
                    collect_responses(responses, mapping.spots, event_index, "spot", |response| {
                        response.spots
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
                    collect_responses(responses, mapping.spots, event_index, "spot", |response| {
                        response.spots
                    })?,
                ))
            })
            .collect()
    }
}

fn flatten_requests(
    requests: &[SimulationDataRequest],
    event_dates: &[Date],
    local_discount_index: &MarketIndex,
) -> (
    Vec<SimulationRequest>,
    Vec<Option<Date>>,
    Vec<EventResponseMap>,
) {
    let mut flattened = Vec::new();
    let mut request_dates = Vec::new();
    let mut maps = Vec::with_capacity(requests.len());

    for (request, event_date) in requests.iter().zip(event_dates) {
        let event_start = flattened.len();
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

        // Hidden discount responses turn each simulated forward into a
        // discounted forward payoff. Its expectation is known from today's
        // curve, making it an effective martingale control variate for
        // scripted rate products without changing their public syntax.
        let forward_discount_start = flattened.len();
        flattened.extend(request.fwds().iter().map(|forward| SimulationRequest {
            discount_request: Some(DiscountRequest::new(
                local_discount_index.clone(),
                forward.end_date().unwrap_or_else(|| forward.fixing_date()),
            )),
            ..SimulationRequest::default()
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

        let spot_start = flattened.len();
        flattened.extend(
            request
                .spots()
                .iter()
                .cloned()
                .map(|spot_request| SimulationRequest {
                    spot_request: Some(spot_request),
                    ..SimulationRequest::default()
                }),
        );

        maps.push(EventResponseMap {
            event_start,
            discounts: ResponseRange {
                start: discount_start,
                len: request.dfs().len(),
            },
            forwards: ResponseRange {
                start: forward_start,
                len: request.fwds().len(),
            },
            forward_discounts: ResponseRange {
                start: forward_discount_start,
                len: request.fwds().len(),
            },
            fx: ResponseRange {
                start: fx_start,
                len: request.fxs().len(),
            },
            spots: ResponseRange {
                start: spot_start,
                len: request.spots().len(),
            },
        });
        request_dates.resize(flattened.len(), Some(*event_date));
    }

    (flattened, request_dates, maps)
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

/// Least-squares coefficients for two centered controls. Standardizing first
/// keeps the tiny bond/forward values numerically well conditioned next to
/// notionals that can be several orders of magnitude larger.
fn estimate_control_betas(samples: &[[f64; 3]]) -> [f64; 2] {
    if samples.len() < 2 {
        return [0.0; 2];
    }
    let count = samples.len() as f64;
    let means = samples.iter().fold([0.0; 3], |mut sums, sample| {
        for index in 0..3 {
            sums[index] += sample[index] / count;
        }
        sums
    });
    let mut moments = [0.0; 5];
    for sample in samples {
        let y = sample[0] - means[0];
        let x0 = sample[1] - means[1];
        let x1 = sample[2] - means[2];
        moments[0] += x0 * x0;
        moments[1] += x1 * x1;
        moments[2] += x0 * x1;
        moments[3] += x0 * y;
        moments[4] += x1 * y;
    }
    let sd0 = moments[0].sqrt();
    let sd1 = moments[1].sqrt();
    let active0 = sd0 > 1.0e-14;
    let active1 = sd1 > 1.0e-14;
    match (active0, active1) {
        (false, false) => [0.0; 2],
        (true, false) => [moments[3] / moments[0], 0.0],
        (false, true) => [0.0, moments[4] / moments[1]],
        (true, true) => {
            let correlation = (moments[2] / (sd0 * sd1)).clamp(-0.999_999, 0.999_999);
            let cov_y_z0 = moments[3] / sd0;
            let cov_y_z1 = moments[4] / sd1;
            // A small ridge protects scripts whose bond and forward controls
            // are almost collinear on the pilot paths.
            let determinant = 1.0 - correlation * correlation + 1.0e-6;
            let gamma0 = (cov_y_z0 - correlation * cov_y_z1) / determinant;
            let gamma1 = (cov_y_z1 - correlation * cov_y_z0) / determinant;
            [gamma0 / sd0, gamma1 / sd1]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::{
        ad::{scalar::Scalar, tape::Tape},
        math::interpolation::interpolator::Interpolator,
        models::lgm::{lgmcomponents::LgmRateModel, lgmmarketmodel::LgmMarketModel},
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        scripting::nodes::event::CodedEvent,
        time::{daycounter::DayCounter, enums::TimeUnit},
    };

    struct ParallelFlatSetup {
        reference_date: Date,
        payment_date: Date,
        discount: f64,
        calls: AtomicUsize,
    }

    impl ScriptModelSetup for ParallelFlatSetup {
        fn n_paths(&self) -> usize {
            64
        }

        fn with_model<R>(&self, callback: &mut ScriptModelCallback<'_, R>) -> Result<R> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let curve = DiscountTermStructure::<DualFwd>::new(
                vec![self.reference_date, self.payment_date],
                vec![DualFwd::one(), DualFwd::scalar(self.discount)],
                DayCounter::Actual365,
                Interpolator::LogLinear,
                true,
            )?;
            let rate_model = LgmRateModel::new(DualFwd::scalar(0.03), DualFwd::zero(), &curve);
            let mut model = LgmMarketModel::new(
                Currency::USD,
                MarketIndex::SOFR,
                self.reference_date,
                DayCounter::Actual365,
            )
            .with_n_paths(self.n_paths())
            .with_seed(11);
            model.add_curve_model(MarketIndex::SOFR, rate_model);
            callback(&mut model, &[])
        }
    }

    #[test]
    fn control_regression_recovers_linear_coefficients() {
        let samples: Vec<[f64; 3]> = (0..20)
            .map(|index| {
                let x0 = f64::from(index) - 7.0;
                let x1 = f64::from((index * index + 3) % 11) - 4.0;
                [5.0 + 2.0 * x0 - 4.0 * x1, x0, x1]
            })
            .collect();
        let betas = estimate_control_betas(&samples);
        assert!((betas[0] - 2.0).abs() < 1.0e-4);
        assert!((betas[1] + 4.0).abs() < 1.0e-4);
    }

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
        let values = engine.evaluate(&mut model, Some("opt"))?;
        let value = *values.get("opt").ok_or_else(|| {
            ScriptingError::EvaluationError("expected numeric opt result".to_string())
        })?;

        assert!((value - 100.0 * event_discount_value).abs() < 1.0e-10);
        assert!(
            (event_discount.adjoint()?.value() - 100.0).abs() < 1.0e-8,
            "pillar adjoints must be accumulated by evaluate itself"
        );

        Tape::stop_recording_fwd();
        Ok(())
    }

    #[test]
    fn market_dependent_ifs_smooth_payoffs_without_explicit_fif() -> Result<()> {
        Tape::start_recording_fwd();

        let reference_date = Date::new(2025, 1, 1);
        let observation_date = reference_date.advance(1, TimeUnit::Years);
        let payment_date = reference_date.advance(2, TimeUnit::Years);
        let day_counter = DayCounter::Actual365;
        let rate = 0.04_f64;
        let observation_df =
            (-rate * day_counter.year_fraction(reference_date, observation_date)).exp();
        let payment_df = (-rate * day_counter.year_fraction(reference_date, payment_date)).exp();
        let curve = DiscountTermStructure::<DualFwd>::new(
            vec![reference_date, observation_date, payment_date],
            vec![
                DualFwd::one(),
                DualFwd::new(observation_df),
                DualFwd::new(payment_df),
            ],
            day_counter,
            Interpolator::LogLinear,
            true,
        )?;
        let rate_model = LgmRateModel::new(DualFwd::scalar(0.03), DualFwd::zero(), &curve);
        let mut model = LgmMarketModel::new(
            Currency::USD,
            MarketIndex::SOFR,
            reference_date,
            day_counter,
        )
        .with_n_paths(1)
        .with_seed(7);
        model.add_curve_model(MarketIndex::SOFR, rate_model);

        let source = format!(
            r#"
            note = 0;
            breaches = 0;
            fixing = RateIndex("SOFR", "{observation_date}", "{payment_date}");
            if fixing > 0.05 {{ breaches += 1; }}
            if breaches == 0 {{
                note pays 107 on "{payment_date}";
            }} else {{
                note pays 102 on "{payment_date}";
            }}
            "#
        );
        let events = EventStream::try_from(vec![CodedEvent::new(observation_date, source)])?;
        let engine = ScriptEngine::new(events, reference_date, Currency::USD, MarketIndex::SOFR)?;
        let values = engine.evaluate(&mut model, Some("note"))?;
        let value = *values.get("note").ok_or_else(|| {
            ScriptingError::EvaluationError("expected numeric note result".to_string())
        })?;

        assert!(
            (value - 107.0 * payment_df).abs() < 1.0e-8,
            "ordinary if should produce the high redemption, got {value}; values: {values:?}; events: {:?}",
            engine.events()
        );

        Tape::stop_recording_fwd();
        Ok(())
    }

    #[test]
    fn evaluates_equity_spot_script_with_lgm_equity_model() -> Result<()> {
        use crate::models::lgm::lgmcomponents::LgmEquityModel;

        Tape::start_recording_fwd();

        let reference_date = Date::new(2025, 1, 1);
        let event_date = reference_date.advance(1, TimeUnit::Years);
        let day_counter = DayCounter::Actual365;
        let rate = 0.05_f64;
        let tau = day_counter.year_fraction(reference_date, event_date);
        let event_discount_value = (-rate * tau).exp();
        let curve = DiscountTermStructure::<DualFwd>::new(
            vec![reference_date, event_date],
            vec![DualFwd::one(), DualFwd::new(event_discount_value)],
            day_counter,
            Interpolator::LogLinear,
            true,
        )?;
        let rate_model = LgmRateModel::new(DualFwd::scalar(0.05), DualFwd::zero(), &curve);
        let eq_rate_model = LgmRateModel::new(DualFwd::scalar(0.05), DualFwd::zero(), &curve);
        let spot_0 = DualFwd::new(100.0);
        // Zero vol and zero dividend yield: S_T = S_0 * exp(r * tau) exactly,
        // so the discounted payoff equals S_0 on every path.
        let equity_model = LgmEquityModel::new(
            &eq_rate_model,
            DualFwd::zero(),
            spot_0,
            DualFwd::zero(),
            DualFwd::zero(),
        );
        let mut model = LgmMarketModel::new(
            Currency::USD,
            MarketIndex::SOFR,
            reference_date,
            day_counter,
        )
        .with_n_paths(4)
        .with_seed(7);
        model.add_curve_model(MarketIndex::SOFR, rate_model);
        model.add_equity_model("ACME".to_string(), equity_model);

        let events = EventStream::try_from(vec![CodedEvent::new(
            event_date,
            "payoff = 0; payoff pays Spot(\"ACME\");".to_string(),
        )])?;
        let engine = ScriptEngine::new(events, reference_date, Currency::USD, MarketIndex::SOFR)?;
        let values = engine.evaluate(&mut model, Some("payoff"))?;
        let value = *values.get("payoff").ok_or_else(|| {
            ScriptingError::EvaluationError("expected numeric payoff result".to_string())
        })?;

        assert!(
            (value - 100.0).abs() < 1.0e-5,
            "discounted zero-vol equity forward must equal spot, got {value}"
        );
        assert!(
            (spot_0.adjoint()?.value() - 1.0).abs() < 1.0e-6,
            "d(value)/d(spot) must be 1 for a zero-vol forward"
        );

        Tape::stop_recording_fwd();
        Ok(())
    }

    #[test]
    fn parallel_evaluation_builds_one_model_per_worker_and_preserves_paths() -> Result<()> {
        let reference_date = Date::new(2025, 1, 1);
        let payment_date = reference_date.advance(1, TimeUnit::Years);
        let discount = (-0.05_f64).exp();
        let setup = ParallelFlatSetup {
            reference_date,
            payment_date,
            discount,
            calls: AtomicUsize::new(0),
        };
        let source = format!(
            "deal_value = 0; r = RateIndex(\"SOFR\", \"{reference_date}\", \"{payment_date}\"); if r < 0.10 {{ deal_value pays 100 on \"{payment_date}\"; }} else {{ deal_value pays 0 on \"{payment_date}\"; }}"
        );
        let events = EventStream::try_from(vec![CodedEvent::new(reference_date, source)])?;
        let engine = ScriptEngine::new(events, reference_date, Currency::USD, MarketIndex::SOFR)?;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .map_err(|error| ScriptingError::EvaluationError(error.to_string()))?;
        let result = pool.install(|| engine.evaluate_parallel(&setup, Some("deal_value")))?;
        let value = result.values.get("deal_value").copied().ok_or_else(|| {
            ScriptingError::EvaluationError("parallel result omitted deal_value".into())
        })?;

        assert!((value - 100.0 * discount).abs() < 1.0e-10);
        // One pilot model plus one model for each of the four path chunks.
        assert_eq!(setup.calls.load(Ordering::Relaxed), 5);
        Ok(())
    }
}

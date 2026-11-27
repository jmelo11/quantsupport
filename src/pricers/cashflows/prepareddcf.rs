//! Explicit numerical snapshots for repeated linear-cashflow valuation.

use crate::{
    ad::{graph::Graph, jit, nativekernel::NativeKernel},
    core::{
        evaluationresults::{EvaluationResults, SensitivityMap},
        request::Request,
    },
    math::compensatedsum::CompensatedSum,
    time::date::Date,
    utils::errors::{QSError, Result},
};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Weak},
};

pub(super) struct DcfCapture {
    pub date: Date,
    pub identifier: String,
    pub graph: Graph,
    pub inputs: Vec<f64>,
    pub active: usize,
    pub risks: Vec<(String, usize)>,
}

struct Code {
    graph: Graph,
    active: usize,
    values: NativeKernel,
    risks: NativeKernel,
}

impl Code {
    fn matches(&self, graph: &Graph, active: usize) -> bool {
        self.active == active
            && self.graph.input_count() == graph.input_count()
            && self.graph.output() == graph.output()
            && self.graph.operation_count() == graph.operation_count()
            && self
                .graph
                .operations()
                .iter()
                .zip(graph.operations())
                .all(|(left, right)| left.same_instruction(right))
    }
}

thread_local! {
    static CODE: RefCell<Vec<Weak<Code>>> = const { RefCell::new(Vec::new()) };
}

fn code_for(graph: Graph, active: usize, previous: Option<&Arc<Code>>) -> Result<Arc<Code>> {
    if let Some(code) = previous.filter(|code| code.matches(&graph, active)) {
        return Ok(Arc::clone(code));
    }
    // Match the ordinary gradient compiler's bound on a single compilation unit.
    // Larger trades keep using that pricer's partitioned execution until this
    // prototype also has a reusable workspace for partitioned value/risk kernels.
    if graph.operation_count() > 2048 {
        return Err(QSError::InvalidValueErr(
            "Prepared DCF supports at most 2048 graph operations; use the ordinary pricer for larger trades".into(),
        ));
    }
    CODE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.retain(|entry| entry.strong_count() != 0);
        for entry in cache.iter().filter_map(Weak::upgrade) {
            if entry.matches(&graph, active) {
                return Ok(entry);
            }
        }
        let code = Arc::new(Code {
            values: jit::compile_outputs(&graph, &[graph.output()], 0)
                .map_err(|error| QSError::EvaluationErr(error.to_string()))?,
            risks: jit::compile_outputs(&graph, &[graph.output()], active)
                .map_err(|error| QSError::EvaluationErr(error.to_string()))?,
            graph,
            active,
        });
        if cache.len() >= 64 {
            cache.clear();
        }
        cache.push(Arc::downgrade(&code));
        Ok(code)
    })
}

fn measures(requests: &[Request]) -> Result<(bool, bool)> {
    if requests
        .iter()
        .any(|request| !matches!(request, Request::Value | Request::Sensitivities))
    {
        return Err(QSError::InvalidValueErr(
            "Prepared DCF supports Value and Sensitivities; use the ordinary pricer for other measures".into(),
        ));
    }
    Ok((
        requests.contains(&Request::Value),
        requests.contains(&Request::Sensitivities),
    ))
}

/// Reusable native valuation of one explicitly bound contract and market snapshot.
///
/// Construct with [`DiscountedCashflowPricer::prepare`](super::discountedcashflowpricer::DiscountedCashflowPricer::prepare).
/// Evaluation supports Value and Sensitivities and performs no recording, graph
/// comparison or market access. Refresh explicitly through the originating
/// pricer after changing market inputs, fixings, terms, dates or discount policy.
/// Each clone owns its numerical buffers and shares only immutable native code.
/// This prototype limits each member to 2048 graph operations; larger trades
/// continue to use the ordinary pricer's partitioned compilation.
#[derive(Clone)]
pub struct PreparedDcfValuation {
    code: Arc<Code>,
    date: Date,
    identifier: String,
    inputs: Vec<f64>,
    labels: Vec<String>,
    positions: Vec<Vec<usize>>,
    gradient: Vec<f64>,
    valid: bool,
}

impl PreparedDcfValuation {
    pub(super) fn from_capture(capture: DcfCapture, previous: Option<&Self>) -> Result<Self> {
        if capture.inputs.iter().any(|value| !value.is_finite()) {
            return Err(QSError::InvalidValueErr(
                "Nonfinite prepared DCF input".into(),
            ));
        }
        let code = code_for(
            capture.graph,
            capture.active,
            previous.map(|plan| &plan.code),
        )?;
        let mut labels = Vec::new();
        let mut positions: Vec<Vec<usize>> = Vec::new();
        let mut columns = HashMap::new();
        for (label, input) in capture.risks {
            if input >= capture.active {
                return Err(QSError::InvalidValueErr(
                    "Prepared DCF risk input is outside the active layout".into(),
                ));
            }
            let column = *columns.entry(label.clone()).or_insert_with(|| {
                labels.push(label);
                positions.push(Vec::new());
                positions.len() - 1
            });
            positions[column].push(input);
        }
        Ok(Self {
            code,
            date: capture.date,
            identifier: capture.identifier,
            inputs: capture.inputs,
            labels,
            positions,
            gradient: vec![0.0; capture.active],
            valid: true,
        })
    }

    pub(super) const fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Returns whether two bindings share the same executable code.
    #[must_use]
    pub fn shares_code_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.code, &other.code)
    }

    /// Evaluates the bound snapshot using private, reusable execution buffers.
    ///
    /// # Errors
    /// Rejects unsupported measures, a failed refresh, nonfinite results or native layout errors.
    pub fn evaluate(&mut self, requests: &[Request]) -> Result<EvaluationResults> {
        let (price, risk) = measures(requests)?;
        self.check_valid()?;
        let mut result = EvaluationResults::new(self.date, self.identifier.clone());
        if price || risk {
            let value = self.run(risk)?;
            if price {
                result = result.with_price(value);
            }
            if risk {
                let exposures: Vec<f64> = self
                    .positions
                    .iter()
                    .map(|positions| positions.iter().map(|&input| self.gradient[input]).sum())
                    .collect();
                if exposures.iter().any(|value| !value.is_finite()) {
                    return Err(QSError::EvaluationErr(
                        "Nonfinite prepared DCF exposure".into(),
                    ));
                }
                result = result.with_sensitivities(
                    SensitivityMap::default()
                        .with_instrument_keys(&self.labels)
                        .with_exposure(&exposures),
                );
            }
        }
        Ok(result)
    }

    fn check_valid(&self) -> Result<()> {
        if !self.valid {
            return Err(QSError::InvalidValueErr(
                "Prepared DCF binding was invalidated by a failed refresh".into(),
            ));
        }
        Ok(())
    }

    fn run(&mut self, risk: bool) -> Result<f64> {
        let value = if risk {
            self.code
                .risks
                .value_and_grad(&self.inputs, &mut self.gradient)
        } else {
            self.code.values.value_and_grad(&self.inputs, &mut [])
        }
        .map_err(|error| QSError::EvaluationErr(error.to_string()))?;
        if !value.is_finite() || (risk && self.gradient.iter().any(|value| !value.is_finite())) {
            return Err(QSError::EvaluationErr(
                "Nonfinite prepared DCF result".into(),
            ));
        }
        Ok(value)
    }
}

/// Ordered prepared DCF members with numerical portfolio aggregation.
///
/// Warm calls execute each member's native code and aggregate its numbers;
/// market requests, recordings and per-member result objects belong to preparation.
/// Update through [`DiscountedCashflowPricer::refresh_portfolio`](super::discountedcashflowpricer::DiscountedCashflowPricer::refresh_portfolio).
pub struct PreparedDcfPortfolio {
    date: Date,
    identifier: String,
    members: Vec<PreparedDcfValuation>,
    labels: Vec<String>,
    columns: Vec<Vec<usize>>,
    sums: Vec<CompensatedSum>,
    valid: bool,
}

impl PreparedDcfPortfolio {
    pub(super) fn new(identifier: String, members: Vec<PreparedDcfValuation>) -> Result<Self> {
        let date = members
            .first()
            .ok_or_else(|| QSError::InvalidValueErr("Prepared portfolio requires members".into()))?
            .date;
        let mut labels = Vec::new();
        let mut lookup = HashMap::new();
        let mut columns = Vec::with_capacity(members.len());
        for member in &members {
            if member.date != date {
                return Err(QSError::InvalidValueErr(
                    "Prepared portfolio members must share an evaluation date".into(),
                ));
            }
            columns.push(
                member
                    .labels
                    .iter()
                    .map(|label| {
                        *lookup.entry(label.clone()).or_insert_with(|| {
                            labels.push(label.clone());
                            labels.len() - 1
                        })
                    })
                    .collect(),
            );
        }
        let sums = (0..labels.len())
            .map(|_| CompensatedSum::default())
            .collect();
        Ok(Self {
            date,
            identifier,
            members,
            labels,
            columns,
            sums,
            valid: true,
        })
    }

    pub(super) const fn invalidate(&mut self) {
        self.valid = false;
    }

    pub(super) fn members(&self) -> &[PreparedDcfValuation] {
        &self.members
    }

    /// Executes the bound book and returns only the requested PV and first risk.
    ///
    /// # Errors
    /// Rejects unsupported requests, a failed refresh and invalid native/aggregation results.
    pub fn evaluate(&mut self, requests: &[Request]) -> Result<EvaluationResults> {
        let (price, risk) = measures(requests)?;
        if !self.valid {
            return Err(QSError::InvalidValueErr(
                "Prepared portfolio binding was invalidated by a failed refresh".into(),
            ));
        }
        let mut result = EvaluationResults::new(self.date, self.identifier.clone());
        if !price && !risk {
            return Ok(result);
        }
        let mut pv = CompensatedSum::default();
        for sum in &mut self.sums {
            *sum = CompensatedSum::default();
        }
        for (member, columns) in self.members.iter_mut().zip(&self.columns) {
            pv.add(member.run(risk)?)?;
            if risk {
                for (positions, &column) in member.positions.iter().zip(columns) {
                    let exposure: f64 = positions.iter().map(|&input| member.gradient[input]).sum();
                    self.sums[column].add(exposure)?;
                }
            }
        }
        if price {
            result = result.with_price(pv.total());
        }
        if risk {
            let exposures: Vec<_> = self.sums.iter().map(CompensatedSum::total).collect();
            result = result.with_sensitivities(
                SensitivityMap::default()
                    .with_instrument_keys(&self.labels)
                    .with_exposure(&exposures),
            );
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "prepareddcf_tests.rs"]
mod tests;

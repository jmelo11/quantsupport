//! Native path execution owned by the existing scripting runtime.

use super::runtime::{ExpectedCashflow, ParallelScriptEvaluation};
use crate::{
    ad::CompiledProgram,
    currencies::currency::Currency,
    models::lgm::marketpathplan::MarketPathPlan,
    time::date::Date,
    utils::errors::{QSError, Result},
};
use rayon::prelude::*;
use std::collections::HashMap;

/// Stable output layout for one script's native numerical program.
pub struct ScriptLayout {
    /// Published model-input labels in Jacobian column order.
    labels: Vec<String>,
    /// Numeric script variables in observation order.
    variables: Vec<String>,
    /// Payment date and currency for each amount/PV observation pair.
    cashflows: Vec<(Date, Currency)>,
    /// Total path count, including all worker ranges.
    paths: usize,
}

impl ScriptLayout {
    /// Defines risk columns, variable/payment observations and the complete path count.
    #[must_use]
    pub const fn new(
        labels: Vec<String>,
        variables: Vec<String>,
        cashflows: Vec<(Date, Currency)>,
        paths: usize,
    ) -> Self {
        Self {
            labels,
            variables,
            cashflows,
            paths,
        }
    }
}

/// Reusable native code with a current script market binding.
pub struct ScriptExecution {
    program: CompiledProgram,
    market: MarketPathPlan<f64>,
    coefficients: Vec<f64>,
    jacobian: Vec<Vec<f64>>,
    layout: ScriptLayout,
}

struct Chunk {
    values: Vec<f64>,
    amounts: Vec<f64>,
    present_values: Vec<f64>,
    gradient: Vec<f64>,
}

impl ScriptExecution {
    /// Binds current coefficients and published-risk coordinates to a native script.
    #[must_use]
    pub const fn new(
        program: CompiledProgram,
        market: MarketPathPlan<f64>,
        coefficients: Vec<f64>,
        jacobian: Vec<Vec<f64>>,
        layout: ScriptLayout,
    ) -> Self {
        Self {
            program,
            market,
            coefficients,
            jacobian,
            layout,
        }
    }
    /// Calculates current path values, payments and first sensitivities.
    ///
    /// # Errors
    /// Propagates random-draw, native execution, comparison and nonfinite-output errors.
    pub fn run(&self) -> Result<ParallelScriptEvaluation> {
        let workers = rayon::current_num_threads().min(self.layout.paths);
        let chunk_size = self.layout.paths.div_ceil(workers);
        let variable_count = self.layout.variables.len();
        let flow_count = self.layout.cashflows.len();
        let ranges: Vec<_> = (0..self.layout.paths)
            .step_by(chunk_size)
            .map(|start| start..(start + chunk_size).min(self.layout.paths))
            .collect();
        #[allow(clippy::cast_precision_loss)]
        let weight = 1.0 / self.layout.paths as f64;
        let chunks = ranges
            .into_par_iter()
            .map(|range| -> Result<Chunk> {
                let mut inputs = self.coefficients.clone();
                inputs.resize(self.program.input_count(), 0.0);
                let mut scratch = vec![0.0; self.market.factor_count()];
                let mut outputs = vec![0.0; self.program.output_count()];
                let mut gradient = vec![0.0; self.program.gradient_count()];
                let mut chunk = Chunk {
                    values: vec![0.0; variable_count],
                    amounts: vec![0.0; flow_count],
                    present_values: vec![0.0; flow_count],
                    gradient: vec![0.0; gradient.len()],
                };
                for path in range {
                    self.market.fill_draws(
                        path,
                        &mut inputs[self.coefficients.len()..],
                        &mut scratch,
                    )?;
                    self.program
                        .evaluate(&inputs, &mut outputs, &mut gradient)
                        .map_err(|error| {
                            QSError::EvaluationErr(format!("native script path {path}: {error}"))
                        })?;
                    if outputs[..variable_count + 2 * flow_count]
                        .iter()
                        .chain(&gradient)
                        .any(|value| !value.is_finite())
                    {
                        return Err(QSError::EvaluationErr(format!(
                            "nonfinite native script path {path}"
                        )));
                    }
                    for (sum, &value) in chunk.values.iter_mut().zip(&outputs) {
                        *sum += value * weight;
                    }
                    for index in 0..flow_count {
                        chunk.amounts[index] += outputs[variable_count + 2 * index] * weight;
                        chunk.present_values[index] +=
                            outputs[variable_count + 2 * index + 1] * weight;
                    }
                    for (sum, &value) in chunk.gradient.iter_mut().zip(&gradient) {
                        *sum += value * weight;
                    }
                }
                Ok(chunk)
            })
            .collect::<Result<Vec<_>>>()?;
        let mut values = vec![0.0; variable_count];
        let mut amounts = vec![0.0; flow_count];
        let mut present_values = vec![0.0; flow_count];
        let mut gradients = vec![0.0; self.program.gradient_count()];
        for chunk in chunks {
            for (sum, value) in values.iter_mut().zip(chunk.values) {
                *sum += value;
            }
            for (sum, value) in amounts.iter_mut().zip(chunk.amounts) {
                *sum += value;
            }
            for (sum, value) in present_values.iter_mut().zip(chunk.present_values) {
                *sum += value;
            }
            for (sum, value) in gradients.iter_mut().zip(chunk.gradient) {
                *sum += value;
            }
        }
        let mut risks: HashMap<String, f64> = self
            .layout
            .labels
            .iter()
            .map(|label| (label.clone(), 0.0))
            .collect();
        for (&gradient, row) in gradients.iter().zip(&self.jacobian) {
            for (label, &partial) in self.layout.labels.iter().zip(row) {
                *risks.entry(label.clone()).or_insert(0.0) += gradient * partial;
            }
        }
        let mut sensitivities: Vec<_> = risks.into_iter().collect();
        sensitivities.sort_by(|a, b| a.0.cmp(&b.0));
        let mut cashflows = Vec::new();
        for (index, &(date, currency)) in self.layout.cashflows.iter().enumerate() {
            if let Some(existing) = cashflows
                .iter_mut()
                .find(|flow: &&mut ExpectedCashflow| flow.date == date && flow.currency == currency)
            {
                existing.amount += amounts[index];
                existing.present_value += present_values[index];
            } else {
                cashflows.push(ExpectedCashflow {
                    date,
                    currency,
                    amount: amounts[index],
                    present_value: present_values[index],
                });
            }
        }
        cashflows.sort_by_key(|flow| flow.date);
        Ok(ParallelScriptEvaluation {
            values: self.layout.variables.iter().cloned().zip(values).collect(),
            sensitivities,
            cashflows,
        })
    }
}

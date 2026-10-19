//! Python view of [`quantsupport::prelude::EvaluationResults`].
//!
//! Tabular results (sensitivities, cashflows) are returned as pandas
//! DataFrames.

use pyo3::prelude::*;
use quantsupport::prelude::EvaluationResults as QsEvaluationResults;
use serde::Deserialize;

use crate::common::conversion::{dataframe, extract_date, from_py};
use crate::QuantSupportError;

/// A single cashflow row extracted from the Rust `CashflowsTable`.
#[derive(Clone)]
struct CashflowRow {
    payment_date: String,
    cashflow_type: String,
    amount: f64,
    fixing: Option<f64>,
    accrual_period: f64,
    currency: String,
    caplet_strike: Option<f64>,
    floorlet_strike: Option<f64>,
    leg_index: usize,
}

#[derive(Deserialize)]
struct CashflowInput {
    payment_date: String,
    #[serde(rename = "type", alias = "cashflow_type")]
    cashflow_type: String,
    amount: f64,
    #[serde(default)]
    fixing: Option<f64>,
    #[serde(default)]
    accrual_period: f64,
    currency: String,
    #[serde(default)]
    caplet_strike: Option<f64>,
    #[serde(default)]
    floorlet_strike: Option<f64>,
    #[serde(default)]
    leg_index: usize,
}

impl From<CashflowInput> for CashflowRow {
    fn from(value: CashflowInput) -> Self {
        Self {
            payment_date: value.payment_date,
            cashflow_type: value.cashflow_type,
            amount: value.amount,
            fixing: value.fixing,
            accrual_period: value.accrual_period,
            currency: value.currency,
            caplet_strike: value.caplet_strike,
            floorlet_strike: value.floorlet_strike,
            leg_index: value.leg_index,
        }
    }
}

fn extract_sensitivities(obj: &Bound<'_, PyAny>) -> PyResult<Vec<(String, f64)>> {
    if let Ok(pairs) = obj.extract::<Vec<(String, f64)>>() {
        return Ok(pairs);
    }
    if let Ok(map) = obj.extract::<std::collections::HashMap<String, f64>>() {
        let mut pairs: Vec<_> = map.into_iter().collect();
        pairs.sort_by(|left, right| left.0.cmp(&right.0));
        return Ok(pairs);
    }
    Err(QuantSupportError::new_err(
        "sensitivities must be a mapping or an iterable of (pillar, value) pairs",
    ))
}

/// Results of evaluating a trade.
#[pyclass(module = "quantsupport", name = "EvaluationResults", subclass, dict)]
pub struct EvaluationResults {
    evaluation_date: quantsupport::prelude::Date,
    identifier: String,
    price: Option<f64>,
    yield_to_maturity: Option<f64>,
    modified_duration: Option<f64>,
    z_spread: Option<f64>,
    oas: Option<f64>,
    fair_rate: Option<f64>,
    fair_rates: Option<Vec<f64>>,
    sensitivities: Option<Vec<(String, f64)>>,
    cashflows: Option<Vec<CashflowRow>>,
}

impl EvaluationResults {
    pub fn from_qs(res: &QsEvaluationResults) -> Self {
        let sensitivities = res.sensitivities().map(|s| {
            s.instrument_keys()
                .iter()
                .cloned()
                .zip(s.exposure().iter().copied())
                .collect()
        });
        let cashflows = res.cashflows().map(|t| {
            (0..t.payment_dates().len())
                .map(|i| CashflowRow {
                    payment_date: t.payment_dates()[i].to_string(),
                    cashflow_type: t.cashflow_types()[i].clone(),
                    amount: t.amounts()[i],
                    fixing: t.fixing()[i],
                    accrual_period: t.accrual_periods()[i],
                    currency: t.currencies()[i].to_string(),
                    caplet_strike: t.caplet_strikes()[i],
                    floorlet_strike: t.floorlet_strikes()[i],
                    leg_index: t.leg_indices()[i],
                })
                .collect()
        });
        Self {
            evaluation_date: res.evaluation_date(),
            identifier: res.identifier().to_string(),
            price: res.price(),
            yield_to_maturity: res.ytm(),
            modified_duration: res.modified_duration(),
            z_spread: res.z_spread(),
            oas: res.oas(),
            fair_rate: res.fair_rate(),
            fair_rates: res.fair_rates().map(<[f64]>::to_vec),
            sensitivities,
            cashflows,
        }
    }
}

#[pymethods]
impl EvaluationResults {
    /// Creates results from a Python pricer.
    ///
    /// `sensitivities` accepts either a mapping or `(pillar, value)` pairs.
    /// `cashflows` accepts dictionaries with the same columns exposed by the
    /// `cashflows` DataFrame property.
    #[new]
    #[pyo3(signature = (
        evaluation_date,
        identifier,
        *,
        price = None,
        yield_to_maturity = None,
        modified_duration = None,
        z_spread = None,
        oas = None,
        fair_rate = None,
        fair_rates = None,
        sensitivities = None,
        cashflows = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        evaluation_date: &Bound<'_, PyAny>,
        identifier: String,
        price: Option<f64>,
        yield_to_maturity: Option<f64>,
        modified_duration: Option<f64>,
        z_spread: Option<f64>,
        oas: Option<f64>,
        fair_rate: Option<f64>,
        fair_rates: Option<Vec<f64>>,
        sensitivities: Option<&Bound<'_, PyAny>>,
        cashflows: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let sensitivities = sensitivities.map(extract_sensitivities).transpose()?;
        let cashflows = cashflows
            .map(|value| {
                from_py::<Vec<CashflowInput>>(value, "cashflows")
                    .map(|rows| rows.into_iter().map(Into::into).collect())
            })
            .transpose()?;
        Ok(Self {
            evaluation_date: extract_date(evaluation_date)?,
            identifier,
            price,
            yield_to_maturity,
            modified_duration,
            z_spread,
            oas,
            fair_rate,
            fair_rates,
            sensitivities,
            cashflows,
        })
    }

    /// Reference or as-of date of the evaluation.
    #[getter]
    fn evaluation_date(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.evaluation_date,
        }
    }

    /// Evaluated instrument identifier.
    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Net present value, if `"Value"` was requested.
    #[getter]
    fn price(&self) -> Option<f64> {
        self.price
    }

    /// Yield to maturity, if requested and supported by the pricer.
    #[getter]
    fn yield_to_maturity(&self) -> Option<f64> {
        self.yield_to_maturity
    }

    /// Alias for `yield_to_maturity`.
    #[getter]
    fn ytm(&self) -> Option<f64> {
        self.yield_to_maturity
    }

    /// Modified duration, if requested and supported by the pricer.
    #[getter]
    fn modified_duration(&self) -> Option<f64> {
        self.modified_duration
    }

    /// Z-spread, if requested and supported by the pricer.
    #[getter]
    fn z_spread(&self) -> Option<f64> {
        self.z_spread
    }

    /// OAS, if requested and supported by the pricer.
    #[getter]
    fn oas(&self) -> Option<f64> {
        self.oas
    }

    /// Scalar fair rate, if available; multi-segment loans use `fair_rates`.
    #[getter]
    fn fair_rate(&self) -> Option<f64> {
        self.fair_rate
    }

    /// Bootstrapped loan coupon/spread levels in contiguous segment order.
    ///
    /// Each prefix repays remaining capital at its segment's end. Previously
    /// solved rates are fixed when solving later segments. None if not supplied.
    #[getter]
    fn fair_rates(&self) -> Option<Vec<f64>> {
        self.fair_rates.clone()
    }

    /// Sensitivities as a DataFrame with columns pillar/value (duplicated
    /// pillars summed, sorted by pillar), if `"Sensitivities"` was requested.
    #[getter]
    fn sensitivities<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let Some(pairs) = &self.sensitivities else {
            return Ok(None);
        };
        let mut map = std::collections::BTreeMap::new();
        for (k, v) in pairs {
            *map.entry(k.clone()).or_insert(0.0) += v;
        }
        let (pillars, values): (Vec<String>, Vec<f64>) = map.into_iter().unzip();
        Ok(Some(dataframe(
            py,
            &[
                ("pillar", pillars.into_pyobject(py)?.into_any()),
                ("value", values.into_pyobject(py)?.into_any()),
            ],
        )?))
    }

    /// Raw sensitivities as an ordered list of `(pillar_label, exposure)` tuples.
    #[getter]
    fn raw_sensitivities(&self) -> Option<Vec<(String, f64)>> {
        self.sensitivities.clone()
    }

    /// Cashflow table as a DataFrame, if `"Cashflows"` was requested.
    #[getter]
    fn cashflows<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let Some(rows) = &self.cashflows else {
            return Ok(None);
        };
        let n = rows.len();
        let mut payment_date = Vec::with_capacity(n);
        let mut cashflow_type = Vec::with_capacity(n);
        let mut amount = Vec::with_capacity(n);
        let mut fixing = Vec::with_capacity(n);
        let mut accrual_period = Vec::with_capacity(n);
        let mut currency = Vec::with_capacity(n);
        let mut caplet_strike = Vec::with_capacity(n);
        let mut floorlet_strike = Vec::with_capacity(n);
        let mut leg_index = Vec::with_capacity(n);
        for r in rows {
            payment_date.push(r.payment_date.clone());
            cashflow_type.push(r.cashflow_type.clone());
            amount.push(r.amount);
            fixing.push(r.fixing);
            accrual_period.push(r.accrual_period);
            currency.push(r.currency.clone());
            caplet_strike.push(r.caplet_strike);
            floorlet_strike.push(r.floorlet_strike);
            leg_index.push(r.leg_index);
        }
        Ok(Some(dataframe(
            py,
            &[
                ("payment_date", payment_date.into_pyobject(py)?.into_any()),
                ("type", cashflow_type.into_pyobject(py)?.into_any()),
                ("amount", amount.into_pyobject(py)?.into_any()),
                ("fixing", fixing.into_pyobject(py)?.into_any()),
                (
                    "accrual_period",
                    accrual_period.into_pyobject(py)?.into_any(),
                ),
                ("currency", currency.into_pyobject(py)?.into_any()),
                ("caplet_strike", caplet_strike.into_pyobject(py)?.into_any()),
                (
                    "floorlet_strike",
                    floorlet_strike.into_pyobject(py)?.into_any(),
                ),
                ("leg_index", leg_index.into_pyobject(py)?.into_any()),
            ],
        )?))
    }

    fn __repr__(&self) -> String {
        format!(
            "EvaluationResults(identifier='{}', evaluation_date='{}', price={:?}, z_spread={:?}, OAS={:?}, fair_rate={:?}, sensitivities={}, cashflows={})",
            self.identifier,
            self.evaluation_date,
            self.price,
            self.z_spread,
            self.oas,
            self.fair_rate,
            self.sensitivities.is_some(),
            self.cashflows.is_some(),
        )
    }
}

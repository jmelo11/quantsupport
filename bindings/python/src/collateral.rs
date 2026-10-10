//! Python bindings for the native collateral and fixed-income curve policies.
use crate::conv::{extract_currency, extract_market_index, from_py, qs_err};
use pyo3::prelude::*;
use quantsupport::prelude::{
    Currency, FixedIncomeDiscountPolicy as QsFixedIncomePolicy, MarketIndex,
    SingleCurveCSADiscountPolicy as QsCsaPolicy,
};

/// Collateral currency and remuneration curve used by the single-curve CSA policy.
#[pyclass(name = "SingleCurveCSADiscountPolicy", frozen, from_py_object)]
#[derive(Clone)]
pub struct SingleCurveCSADiscountPolicy {
    pub(crate) collateral_currency: Currency,
    pub(crate) collateral_index: MarketIndex,
}
impl SingleCurveCSADiscountPolicy {
    pub(crate) fn build(&self) -> QsCsaPolicy {
        QsCsaPolicy::new(self.collateral_index.clone(), self.collateral_currency)
    }
}
#[pymethods]
impl SingleCurveCSADiscountPolicy {
    #[new]
    fn new(
        collateral_currency: &Bound<'_, PyAny>,
        collateral_index: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            collateral_currency: extract_currency(collateral_currency)?,
            collateral_index: extract_market_index(collateral_index)?,
        })
    }
    /// Builds a policy from collateral_currency and collateral_index.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            collateral_currency: Currency,
            collateral_index: MarketIndex,
        }
        let input: Input = from_py(obj, "CSA policy")?;
        Ok(Self {
            collateral_currency: input.collateral_currency,
            collateral_index: input.collateral_index,
        })
    }
    #[getter]
    fn collateral_currency(&self) -> crate::enums::Currency {
        self.collateral_currency.into()
    }
    #[getter]
    fn collateral_index(&self) -> crate::enums::MarketIndex {
        crate::enums::MarketIndex {
            inner: self.collateral_index.clone(),
        }
    }
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &serde_json::json!({"collateral_currency": self.collateral_currency, "collateral_index": self.collateral_index})).map_err(qs_err)
    }
    fn __repr__(&self) -> String {
        format!(
            "SingleCurveCSADiscountPolicy(collateral_currency={}, collateral_index={})",
            self.collateral_currency, self.collateral_index
        )
    }
}

/// Per-currency bond discount curves, optionally overridden by the instrument's index.
#[pyclass(name = "FixedIncomeDiscountPolicy", frozen, from_py_object)]
#[derive(Clone)]
pub struct FixedIncomeDiscountPolicy {
    risk_free_indices: Vec<(Currency, MarketIndex)>,
    prefer_instrument_index: bool,
}
impl FixedIncomeDiscountPolicy {
    pub(crate) fn build(&self) -> QsFixedIncomePolicy {
        self.risk_free_indices.iter().fold(
            QsFixedIncomePolicy::new(self.prefer_instrument_index),
            |policy, (currency, index)| policy.with_risk_free_index(*currency, index.clone()),
        )
    }
}
#[pymethods]
impl FixedIncomeDiscountPolicy {
    #[new]
    #[pyo3(signature = (risk_free_indices, prefer_instrument_index = true))]
    fn new(
        risk_free_indices: &Bound<'_, pyo3::types::PyDict>,
        prefer_instrument_index: bool,
    ) -> PyResult<Self> {
        let indices = risk_free_indices
            .iter()
            .map(|(currency, index)| {
                Ok((extract_currency(&currency)?, extract_market_index(&index)?))
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            risk_free_indices: indices,
            prefer_instrument_index,
        })
    }
}

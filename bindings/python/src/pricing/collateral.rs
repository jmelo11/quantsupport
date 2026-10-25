//! Python bindings for the native collateral and fixed-income curve policies.
use crate::common::conversion::{extract_currency, extract_market_index, from_py, qs_err};
use pyo3::prelude::*;
use quantsupport::prelude::{
    Currency, FixedIncomeDiscountPolicy as QsFixedIncomePolicy, MarketIndex,
    SingleCurveCSADiscountPolicy as QsCsaPolicy,
};

/// Collateral currency and remuneration curve used by the single-curve CSA policy.
#[pyclass(
    module = "quantsupport",
    name = "SingleCurveCSADiscountPolicy",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct SingleCurveCSADiscountPolicy {
    collateral_currency: Currency,
    collateral_index: MarketIndex,
}
impl From<QsCsaPolicy> for SingleCurveCSADiscountPolicy {
    fn from(policy: QsCsaPolicy) -> Self {
        Self {
            collateral_currency: policy.base_currency(),
            collateral_index: policy.collateral_index().clone(),
        }
    }
}

impl SingleCurveCSADiscountPolicy {
    pub fn build(&self) -> QsCsaPolicy {
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
    fn collateral_currency(&self) -> crate::common::enums::Currency {
        self.collateral_currency.into()
    }
    #[getter]
    fn collateral_index(&self) -> crate::common::enums::MarketIndex {
        crate::common::enums::MarketIndex {
            inner: self.collateral_index.clone(),
        }
    }
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &serde_json::json!({"collateral_currency": self.collateral_currency, "collateral_index": self.collateral_index})).map_err(qs_err)
    }
}

/// Select discount curves by the cashflow currency.
///
/// Parameters
/// ----------
/// discount_curve_by_currency : dict[Currency | str, MarketIndex | str]
///     Currency-to-curve mapping. Each index must identify a configured curve.
/// prefer_instrument_index : bool, optional
///     Use an instrument's discount index when present. Defaults to True.
///     Loans obtain their discount curve from the currency mapping.
///
/// Examples
/// --------
/// FixedIncomeDiscountPolicy(discount_curve_by_currency={Currency.EUR: MarketIndex.ESTR})
#[pyclass(
    module = "quantsupport",
    name = "FixedIncomeDiscountPolicy",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct FixedIncomeDiscountPolicy {
    discount_curve_by_currency: Vec<(Currency, MarketIndex)>,
    prefer_instrument_index: bool,
}
impl FixedIncomeDiscountPolicy {
    pub fn build(&self) -> QsFixedIncomePolicy {
        self.discount_curve_by_currency.iter().fold(
            QsFixedIncomePolicy::new(self.prefer_instrument_index),
            |policy, (currency, index)| policy.with_discount_curve_index(*currency, index.clone()),
        )
    }
}
#[pymethods]
impl FixedIncomeDiscountPolicy {
    /// Returns the currency-to-curve mapping and discount preference.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let indices = pyo3::types::PyDict::new(py);
        for (currency, index) in &self.discount_curve_by_currency {
            indices.set_item(
                currency.to_string(),
                pythonize::pythonize(py, index).map_err(qs_err)?,
            )?;
        }
        let result = pyo3::types::PyDict::new(py);
        result.set_item("discount_curve_by_currency", indices)?;
        result.set_item("prefer_instrument_index", self.prefer_instrument_index)?;
        Ok(result.into_any())
    }

    #[new]
    #[pyo3(signature = (discount_curve_by_currency, prefer_instrument_index = true))]
    fn new(
        discount_curve_by_currency: &Bound<'_, pyo3::types::PyDict>,
        prefer_instrument_index: bool,
    ) -> PyResult<Self> {
        let indices = discount_curve_by_currency
            .iter()
            .map(|(currency, index)| {
                Ok((extract_currency(&currency)?, extract_market_index(&index)?))
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            discount_curve_by_currency: indices,
            prefer_instrument_index,
        })
    }
}

crate::common::representation::impl_dict_protocol!(
    SingleCurveCSADiscountPolicy,
    FixedIncomeDiscountPolicy
);

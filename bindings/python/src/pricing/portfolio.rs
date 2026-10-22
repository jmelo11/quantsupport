//! Python portfolio membership with streaming native numeric aggregation.

use crate::{
    common::conversion::{extract_currency, extract_requests, qs_err},
    pricing::results::EvaluationResults,
};
use pyo3::prelude::*;
use quantsupport::prelude::{Portfolio as QsPortfolio, QSError, Request as QsRequest};

/// Ordered trade list, evaluated one member at a time against a pricing context.
/// Repeated trades count as repeated positions. Results are standard EvaluationResults.
#[pyclass(module = "quantsupport", name = "Portfolio", unsendable)]
pub struct Portfolio {
    inner: QsPortfolio<Py<PyAny>>,
}

#[pymethods]
impl Portfolio {
    #[new]
    fn new(trades: Vec<Py<PyAny>>) -> PyResult<Self> {
        Ok(Self {
            inner: QsPortfolio::new(trades).map_err(qs_err)?,
        })
    }

    /// Aggregate Value, Sensitivities and Cashflows through context.evaluate.
    /// Unsupported aggregate requests are ignored. A measure omitted by any
    /// member remains absent. value_currency identifies the common monetary PV
    /// currency used by all member pricers; cashflows keep their own currencies.
    #[pyo3(signature = (context, requests = None, *, value_currency = None))]
    fn evaluate(
        &self,
        py: Python<'_>,
        context: &Bound<'_, PyAny>,
        requests: Option<Vec<Bound<'_, PyAny>>>,
        value_currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<EvaluationResults> {
        let requests = requests.map_or_else(
            || Ok(vec![QsRequest::Value]),
            |requests| extract_requests(&requests),
        )?;
        let currency = value_currency.map(extract_currency).transpose()?;
        let result = self
            .inner
            .evaluate(&requests, |_, trade, requests| {
                let requests: Vec<crate::common::enums::Request> =
                    requests.iter().copied().map(Into::into).collect();
                let result = context
                    .call_method1("evaluate", (trade.bind(py), requests))
                    .and_then(|result| Ok(result.extract::<Py<EvaluationResults>>()?))
                    .and_then(|result| result.borrow(py).to_qs())
                    .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
                Ok((currency, result))
            })
            .map_err(qs_err)?;
        Ok(EvaluationResults::from_qs(&result))
    }

    fn __len__(&self) -> usize {
        self.inner.trades().len()
    }

    fn __repr__(&self) -> String {
        format!("Portfolio(trades={})", self.inner.trades().len())
    }
}

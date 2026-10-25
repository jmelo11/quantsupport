//! Python portfolio membership with streaming native numeric aggregation.

use crate::common::conversion::qs_err;
use pyo3::prelude::*;
use quantsupport::prelude::Portfolio as QsPortfolio;

/// Ordered trade list evaluated through PricingContext.evaluate.
/// Repeated trades count as repeated positions.
#[pyclass(module = "quantsupport", name = "Portfolio", unsendable)]
pub struct Portfolio {
    inner: QsPortfolio<Py<PyAny>>,
}

impl Portfolio {
    /// Returns native portfolio membership for dispatch to the portfolio pricer.
    pub(super) const fn native(&self) -> &QsPortfolio<Py<PyAny>> {
        &self.inner
    }
}

#[pymethods]
impl Portfolio {
    #[new]
    fn new(identifier: String, trades: Vec<Py<PyAny>>) -> PyResult<Self> {
        Ok(Self {
            inner: QsPortfolio::new(identifier, trades).map_err(qs_err)?,
        })
    }

    /// Returns the portfolio identifier carried by its aggregate results.
    #[getter]
    fn identifier(&self) -> &str {
        self.inner.identifier()
    }

    fn __len__(&self) -> usize {
        self.inner.trades().len()
    }

    fn __repr__(&self) -> String {
        format!(
            "Portfolio(identifier={:?}, trades={})",
            self.inner.identifier(),
            self.inner.trades().len()
        )
    }
}

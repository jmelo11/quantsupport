//! Temporary shocks applied to the innermost active Python pricing context.
use std::cell::RefCell;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use quantsupport::prelude::{PricingContext as QsPricingContext, Scenario as QsScenario};

use crate::context::PricingContext;
use crate::market::Scenario;

thread_local! {
    static ACTIVE_CONTEXTS: RefCell<Vec<Py<PricingContext>>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn enter_context(context: &Bound<'_, PricingContext>) -> bool {
    ACTIVE_CONTEXTS.with(|contexts| {
        let mut contexts = contexts.borrow_mut();
        let first = contexts.is_empty();
        contexts.push(context.clone().unbind());
        first
    })
}

/// Removes the current scope; returns whether the same or another context remains active.
pub(crate) fn exit_context(context: &Bound<'_, PricingContext>) -> PyResult<(bool, bool)> {
    ACTIVE_CONTEXTS.with(|contexts| {
        let mut contexts = contexts.borrow_mut();
        if let Some(current) = contexts.last() {
            if !current.bind(context.py()).is(context) {
                return Err(PyRuntimeError::new_err(
                    "Pricing contexts must exit in reverse entry order",
                ));
            }
            contexts.pop();
        }
        Ok((
            contexts
                .iter()
                .any(|entry| entry.bind(context.py()).is(context)),
            !contexts.is_empty(),
        ))
    })
}

fn active_context(py: Python<'_>) -> PyResult<Py<PricingContext>> {
    ACTIVE_CONTEXTS.with(|contexts| {
        contexts
            .borrow()
            .last()
            .map(|context| context.clone_ref(py))
            .ok_or_else(|| {
                PyRuntimeError::new_err(
                    "Scenarios requires an active PricingContext; use `with context, scenarios:`",
                )
            })
    })
}

struct SavedMarket {
    context: Py<PricingContext>,
    market: QsPricingContext,
}

/// Temporary market shocks, scoped to ``with context, scenarios:``.
///
/// Parameters
/// ----------
/// shocks : sequence of Scenario
///     Absolute or relative shocks to quote identifiers or explicit zero-rate
///     pillar labels. An index selector shocks every matching pillar.
///
/// Enter after a PricingContext. The market is rebuilt with these shocks in
/// addition to any enclosing scenarios. Exit restores the previous market,
/// including on exceptions. The same Scenarios object can be reused.
#[pyclass(module = "quantsupport", unsendable)]
pub struct Scenarios {
    shocks: Vec<QsScenario>,
    saved: Option<SavedMarket>,
}

#[pymethods]
impl Scenarios {
    #[new]
    #[pyo3(signature = (*, shocks))]
    fn new(shocks: Vec<Scenario>) -> Self {
        Self {
            shocks: shocks.into_iter().map(|shock| shock.inner).collect(),
            saved: None,
        }
    }

    #[getter]
    fn shocks(&self) -> Vec<Scenario> {
        self.shocks
            .iter()
            .cloned()
            .map(|inner| Scenario { inner })
            .collect()
    }

    fn __enter__(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        if slf.saved.is_some() {
            return Err(PyRuntimeError::new_err(
                "This Scenarios object is already active",
            ));
        }
        let context = active_context(slf.py())?;
        let market = context
            .bind(slf.py())
            .try_borrow_mut()?
            .begin_scoped_scenarios(&slf.shocks)?;
        slf.saved = Some(SavedMarket { context, market });
        Ok(slf)
    }

    #[pyo3(signature = (_exc_type = None, _exc_value = None, _traceback = None))]
    fn __exit__(
        &mut self,
        py: Python<'_>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let saved = self
            .saved
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("Scenarios is not active"))?;
        saved
            .context
            .bind(py)
            .try_borrow_mut()?
            .restore_scoped_market(saved.market);
        Ok(false)
    }
}

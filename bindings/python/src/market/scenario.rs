//! Temporary shocks applied to the innermost active Python pricing context.
use std::cell::RefCell;

use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
};
use quantsupport::{
    core::marketdatahandling::marketview::{MarketCache, MarketView},
    prelude::{PricingContext as QsPricingContext, Scenario as QsScenario, Shock as QsShock},
};

use crate::common::{conversion::qs_err, enums::ScenarioType};
use crate::pricing::context::PricingContext;

/// A market shock selecting market quote identifiers.
///
/// Parameters
/// ----------
/// target : str
///     Exact pillar identifier or underscore-separated selector. For example,
///     EURIBOR12m selects its deposit and swap quotes; FUNDINGSPREAD selects
///     every zero-rate quote of that funding-spread curve.
/// value : float
///     Additive decimal change for Absolute shocks (0.01 is 100 basis points),
///     or fractional change for Relative shocks (0.10 increases inputs by 10%).
/// shock_type : ScenarioType
///     Absolute or Relative interpretation of the value.
///
/// Immutable. Combine shocks in Scenario and enter it after a PricingContext.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct Shock {
    inner: QsShock,
}

#[pymethods]
impl Shock {
    #[new]
    #[pyo3(signature = (target, value, shock_type))]
    fn new(target: String, value: f64, shock_type: ScenarioType) -> PyResult<Self> {
        if target.is_empty() || !value.is_finite() {
            return Err(PyValueError::new_err(
                "Market shocks require a nonempty target and a finite value",
            ));
        }
        Ok(Self {
            inner: QsShock::new(target, value, shock_type.into()),
        })
    }

    /// Exact pillar identifier or underscore-separated selector.
    #[getter]
    fn target(&self) -> &str {
        self.inner.target()
    }

    /// Additive decimal change or fractional relative change.
    #[getter]
    fn value(&self) -> f64 {
        self.inner.value()
    }

    /// Interpretation of the shock value.
    #[getter]
    fn shock_type(&self) -> ScenarioType {
        self.inner.shock_type().into()
    }

    fn __repr__(&self) -> String {
        format!(
            "Shock(target={:?}, value={}, shock_type=ScenarioType.{:?})",
            self.inner.target(),
            self.inner.value(),
            self.inner.shock_type()
        )
    }
}

struct ActiveScenario {
    context_key: usize,
    scope_key: usize,
    generation: u64,
    scenario: QsScenario,
    cache: MarketCache,
}

thread_local! {
    static ACTIVE_SCENARIO: RefCell<Vec<ActiveScenario>> = const { RefCell::new(Vec::new()) };
    static ACTIVE_CONTEXTS: RefCell<Vec<Py<PricingContext>>> = const { RefCell::new(Vec::new()) };
}

pub fn enter_context(context: &Bound<'_, PricingContext>) -> bool {
    ACTIVE_CONTEXTS.with(|contexts| {
        let mut contexts = contexts.borrow_mut();
        let first = contexts.is_empty();
        contexts.push(context.clone().unbind());
        first
    })
}

/// Removes the current scope; returns whether the same or another context remains active.
pub fn exit_context(context: &Bound<'_, PricingContext>) -> PyResult<(bool, bool)> {
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
                    "Scenario requires an active PricingContext; use `with context, scenario:`",
                )
            })
    })
}

pub fn market_view<'a>(
    context: &PricingContext,
    generation: u64,
    base: &'a QsPricingContext,
) -> PyResult<MarketView<'a>> {
    let active = ACTIVE_SCENARIO.with(|stack| -> PyResult<_> {
        let mut stack = stack.borrow_mut();
        let Some(scope) = stack
            .iter_mut()
            .rev()
            .find(|scope| scope.context_key == context.key())
        else {
            return Ok(None);
        };
        if scope.generation != generation {
            scope.scenario.validate(base).map_err(qs_err)?;
            scope.cache = MarketCache::default();
            scope.generation = generation;
        }
        Ok(Some((scope.scenario.clone(), scope.cache.clone())))
    })?;
    match active {
        Some((scenario, cache)) => Ok(MarketView::new(base, scenario).with_cache(cache)),
        None => Ok(MarketView::base(base)),
    }
}

/// A combination of temporary market shocks used in ``with context, scenario:``.
///
/// Parameters
/// ----------
/// shocks : sequence of Shock
///     Typed market shocks, applied in their supplied order. Each Shock supplies
///     a target selector, value and ScenarioType.
///
/// Enter after a PricingContext. Nested scenarios compose shocks in entry order.
/// The base context retains its market inputs and constructed objects. Requested
/// curves and volatility markets have independent AD state. Leaving the scope,
/// including through an exception, discards the temporary view. Reusable.
#[pyclass(module = "quantsupport", unsendable)]
pub struct Scenario {
    inner: QsScenario,
    active: Option<(Py<PricingContext>, usize)>,
}

#[pymethods]
impl Scenario {
    #[new]
    #[pyo3(signature = (shocks))]
    fn new(shocks: Vec<Shock>) -> Self {
        Self {
            inner: QsScenario::new(shocks.into_iter().map(|shock| shock.inner).collect()),
            active: None,
        }
    }

    #[getter]
    fn shocks(&self) -> Vec<Shock> {
        self.inner
            .shocks()
            .iter()
            .cloned()
            .map(|inner| Shock { inner })
            .collect()
    }

    fn __enter__(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        if slf.active.is_some() {
            return Err(PyRuntimeError::new_err(
                "This Scenario object is already active",
            ));
        }
        let context = active_context(slf.py())?;
        let context_ref = context.bind(slf.py()).try_borrow()?;
        context_ref.require_initialized()?;
        let context_key = context_ref.key();
        let scope_key = slf.as_ptr() as usize;
        let combined = ACTIVE_SCENARIO.with(|stack| {
            let stack = stack.borrow();
            stack
                .iter()
                .rev()
                .find(|scope| scope.context_key == context_key)
                .map(|scope| scope.scenario.shocks().to_vec())
                .unwrap_or_default()
        });
        let scenario = QsScenario::new(
            combined
                .into_iter()
                .chain(slf.inner.shocks().iter().cloned())
                .collect(),
        );
        scenario
            .validate(context_ref.base_context())
            .map_err(qs_err)?;
        ACTIVE_SCENARIO.with(|stack| {
            stack.borrow_mut().push(ActiveScenario {
                context_key,
                scope_key,
                generation: context_ref.generation(),
                scenario,
                cache: MarketCache::default(),
            })
        });
        drop(context_ref);
        slf.active = Some((context, scope_key));
        Ok(slf)
    }

    #[pyo3(signature = (_exc_type = None, _exc_value = None, _traceback = None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let (_, key) = self
            .active
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Scenario is not active"))?;
        ACTIVE_SCENARIO.with(|stack| {
            let mut stack = stack.borrow_mut();
            if stack.last().is_none_or(|scope| scope.scope_key != *key) {
                return Err(PyRuntimeError::new_err(
                    "Scenario scopes must exit in reverse entry order",
                ));
            }
            stack.pop();
            Ok(())
        })?;
        self.active = None;
        Ok(false)
    }
}

crate::common::representation::impl_class_repr!(Scenario);

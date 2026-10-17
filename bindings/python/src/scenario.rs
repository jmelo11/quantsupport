//! Temporary shocks applied to the innermost active Python pricing context.
use std::cell::RefCell;

use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::PyDict,
};
use quantsupport::{
    core::marketdatahandling::marketview::{MarketCache, MarketView},
    prelude::{PricingContext as QsPricingContext, Scenario as QsScenario, Shock},
};

use crate::{
    context::PricingContext,
    conv::{extract_scenario_type, qs_err},
};

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
                    "Scenario requires an active PricingContext; use `with context, scenario:`",
                )
            })
    })
}

pub(crate) fn market_view<'a>(
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
/// shocks : sequence of dict
///     Each dictionary supplies ``target``, ``value`` and ``type``. Targets select
///     quote identifiers or explicit zero-rate pillar labels. Absolute values are
///     additive decimal changes; Relative values are fractional changes.
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
    #[pyo3(signature = (*, shocks))]
    fn new(shocks: Vec<Bound<'_, PyDict>>) -> PyResult<Self> {
        let shocks = shocks
            .into_iter()
            .map(|shock| {
                for key in shock.keys() {
                    let key: String = key.extract()?;
                    if !matches!(key.as_str(), "target" | "value" | "type") {
                        return Err(PyValueError::new_err(format!(
                            "Unknown shock field '{key}'"
                        )));
                    }
                }
                let field = |name| {
                    shock.get_item(name)?.ok_or_else(|| {
                        PyValueError::new_err(format!("Missing shock field '{name}'"))
                    })
                };
                let target: String = field("target")?.extract()?;
                let value: f64 = field("value")?.extract()?;
                let shock_type = extract_scenario_type(&field("type")?)?;
                if target.is_empty() || !value.is_finite() {
                    return Err(PyValueError::new_err(
                        "Scenario shocks require a nonempty target and a finite value",
                    ));
                }
                Ok(Shock::new(target, value, shock_type))
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: QsScenario::new(shocks),
            active: None,
        })
    }

    #[getter]
    fn shocks<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyDict>>> {
        self.inner
            .shocks()
            .iter()
            .map(|shock| {
                let fields = PyDict::new(py);
                fields.set_item("target", shock.target())?;
                fields.set_item("value", shock.value())?;
                fields.set_item("type", crate::enums::ScenarioType::from(shock.shock_type()))?;
                Ok(fields)
            })
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

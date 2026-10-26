//! In-memory scripted product specifications for the native script runtime.

use pyo3::prelude::*;
use quantsupport::prelude::{
    CodedEvent, Currency, Date, EventStream, MarketIndex, PricingModelConfiguration,
};
use serde::{Deserialize, Serialize};

use crate::common::conversion::{from_json_file, from_py, qs_err};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptEvent {
    #[serde(alias = "event_date")]
    date: Date,
    #[serde(alias = "script")]
    code: String,
}

fn default_name() -> String {
    "script".into()
}

/// Dated payoff code evaluated inside an initialized pricing context.
#[pyclass(
    module = "quantsupport",
    name = "ScriptedProduct",
    from_py_object,
    frozen
)]
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptedProduct {
    #[serde(default = "default_name", alias = "identifier")]
    name: String,
    events: Vec<ScriptEvent>,
    currency: Currency,
    #[serde(default)]
    discount_index: Option<MarketIndex>,
    #[serde(default)]
    result_variable: Option<String>,
    #[serde(default)]
    models: Vec<PricingModelConfiguration>,
}

impl ScriptedProduct {
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    pub const fn discount_index(&self) -> Option<&MarketIndex> {
        self.discount_index.as_ref()
    }

    pub fn result_variable(&self) -> Option<&str> {
        self.result_variable.as_deref()
    }

    pub fn models(&self) -> &[PricingModelConfiguration] {
        &self.models
    }

    pub fn event_stream(&self) -> PyResult<EventStream> {
        if self.events.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "a script must contain at least one event",
            ));
        }
        EventStream::try_from(
            self.events
                .iter()
                .map(|event| CodedEvent::new(event.date, event.code.clone()))
                .collect::<Vec<_>>(),
        )
        .map_err(qs_err)
    }

    fn validate(&self) -> PyResult<()> {
        self.event_stream()?;
        let mut indexes = std::collections::HashSet::new();
        for model in &self.models {
            if !indexes.insert(model.market_index()) {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "duplicate script model for '{}'",
                    model.market_index(),
                )));
            }
        }
        Ok(())
    }
}

#[pymethods]
impl ScriptedProduct {
    /// Defines script event tenors before the pricing reference date is available.
    #[classmethod]
    #[pyo3(signature=(**parameters))]
    fn from_tenors<'py>(
        cls: &Bound<'py, pyo3::types::PyType>,
        parameters: Option<&Bound<'py, pyo3::types::PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let empty = pyo3::types::PyDict::new(cls.py());
        crate::instruments::unresolved::from_tenors(cls, parameters.unwrap_or(&empty))
    }

    /// Builds a script from events [{date, code}], currency, and optional model assignments.
    #[staticmethod]
    pub fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let product: Self = from_py(obj, "scripted product")?;
        product.validate()?;
        Ok(product)
    }

    /// Loads the same script schema from a JSON file.
    #[staticmethod]
    fn from_json(path: &str) -> PyResult<Self> {
        let product: Self = from_json_file(path, "scripted product")?;
        product.validate()?;
        Ok(product)
    }

    /// Returns the script specification, including its native model schemas.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, self).map_err(qs_err)
    }

    /// Product name.
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    /// Product identifier (an alias for name).
    #[getter]
    fn identifier(&self) -> &str {
        &self.name
    }
}

crate::common::representation::impl_dict_protocol!(ScriptedProduct);

//! Shared Python representations and dictionary conversion for data objects.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::Serialize;

use super::conversion::qs_err;

/// Preserve serde's externally tagged enums while making unit variants dictionaries.
pub fn to_dict<'py, T: Serialize>(py: Python<'py>, value: &T) -> PyResult<Bound<'py, PyAny>> {
    let mut value = serde_json::to_value(value).map_err(qs_err)?;
    if let serde_json::Value::String(variant) = value {
        value = serde_json::json!({variant: null});
    }
    pythonize::pythonize(py, &value).map_err(qs_err)
}

/// Format real Python values, including Python quoting, booleans, and None.
pub fn class_repr(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    let fields = obj.call_method0("to_dict")?;
    let mut fields = fields.cast::<PyDict>()?;
    // Tagged coupon serialization stays round-trippable; repr shows its terms.
    let class_name = obj.get_type().name()?;
    let tagged = fields.get_item(&class_name)?;
    if fields.len() == 1 {
        if let Some(terms) = tagged
            .as_ref()
            .and_then(|value| value.cast::<PyDict>().ok())
        {
            fields = terms;
        }
    }
    let arguments = fields
        .iter()
        .map(|(key, value)| Ok(format!("{}={}", key.str()?, value.repr()?)))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(format!(
        "{}({})",
        obj.get_type().name()?,
        arguments.join(", ")
    ))
}

/// Give serializable data objects a consistent repr and the dict(obj) protocol.
macro_rules! impl_dict_protocol {
    ($($name:ty),+ $(,)?) => {$ (
        #[pyo3::pymethods]
        impl $name {
            fn __repr__(slf: &pyo3::Bound<'_, Self>) -> pyo3::PyResult<String> {
                crate::common::representation::class_repr(slf.as_any())
            }

            /// Keys in the serialized dictionary; supports dict(obj).
            fn keys(slf: &pyo3::Bound<'_, Self>) -> pyo3::PyResult<Vec<String>> {
                use pyo3::prelude::*;
                slf.call_method0("to_dict")?.cast::<pyo3::types::PyDict>()?.keys().extract()
            }

            fn __getitem__<'py>(
                slf: &pyo3::Bound<'py, Self>,
                key: &pyo3::Bound<'py, pyo3::PyAny>,
            ) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::PyAny>> {
                use pyo3::prelude::*;
                slf.call_method0("to_dict")?.get_item(key)
            }
        }
    )+};
}
pub(crate) use impl_dict_protocol;

/// Class-style fallback for services and utility namespaces without serialized state.
macro_rules! impl_class_repr {
    ($($name:ty),+ $(,)?) => {$ (
        #[pyo3::pymethods]
        impl $name {
            fn __repr__(slf: &pyo3::Bound<'_, Self>) -> pyo3::PyResult<String> {
                use pyo3::prelude::*;
                Ok(format!("{}()", slf.get_type().name()?))
            }
        }
    )+};
}
pub(crate) use impl_class_repr;

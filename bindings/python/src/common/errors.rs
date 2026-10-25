//! One structured error boundary shared by every Python domain.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use quantsupport::utils::errors::QSError;

/// Keep the original allocation function while guarding Python argument extraction.
/// Replacing __new__ with a Python wrapper around its descriptor is unsafe for
/// extension types: CPython rejects that descriptor after the slot changes.
#[pyclass(module = "quantsupport", name = "_ErrorConstructor")]
struct ErrorConstructor {
    owner: Py<pyo3::types::PyType>,
    allocate: pyo3::ffi::newfunc,
    operation: String,
}

#[pymethods]
impl ErrorConstructor {
    #[pyo3(signature = (cls, *args, **kwargs))]
    fn __call__<'py>(
        &self,
        py: Python<'py>,
        cls: &Bound<'py, pyo3::types::PyType>,
        args: &Bound<'py, pyo3::types::PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if !cls.is_subclass(self.owner.bind(py))? {
            return Err(context(
                py,
                invalid_type(cls.as_any(), &self.operation, None),
                Some(&self.operation),
                None,
            ));
        }
        // SAFETY: allocate was captured from owner's Py_tp_new slot. cls is
        // owner or a Python-approved subtype, and args/kwargs are live Python
        // tuples/dicts. The slot returns a new reference or sets an exception.
        let result = unsafe {
            let pointer = (self.allocate)(
                cls.as_type_ptr(),
                args.as_ptr(),
                kwargs.map_or(std::ptr::null_mut(), Bound::as_ptr),
            );
            Bound::from_owned_ptr_or_err(py, pointer)
        };
        result.map_err(|error| context(py, error, Some(&self.operation), None))
    }

    fn __traverse__(&self, visit: pyo3::PyVisit<'_>) -> Result<(), pyo3::PyTraverseError> {
        visit.call(&self.owner)
    }
}

// Capture native signatures before method guards change how Python 3.11/3.12
// inspect inherited __init__ methods.
fn install_constructors(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let inspect = py.import("inspect")?;
    let staticmethod = py.import("builtins")?.getattr("staticmethod")?;
    let mut signatures = std::collections::HashMap::new();
    for (_, value) in module.dict().iter() {
        if let Ok(cls) = value.cast::<pyo3::types::PyType>() {
            if let Ok(signature) = inspect.getattr("signature")?.call1((cls,)) {
                signatures.insert(cls.as_ptr() as usize, signature);
            }
        }
    }
    for (_, value) in module.dict().iter() {
        let Ok(cls) = value.cast::<pyo3::types::PyType>() else {
            continue;
        };
        if cls.name()?.to_string_lossy().starts_with('_')
            || cls.is_subclass(&py.get_type::<pyo3::exceptions::PyBaseException>())?
            || cls
                .getattr("__dict__")?
                .contains("_quantsupport_constructor")?
        {
            continue;
        }
        // SAFETY: cls is a live Python type. PyType_GetSlot is part of the
        // stable C API, and Py_tp_new has the newfunc signature by definition.
        let allocate =
            unsafe { pyo3::ffi::PyType_GetSlot(cls.as_type_ptr(), pyo3::ffi::Py_tp_new) };
        if allocate.is_null() {
            continue;
        }
        let signature = signatures.remove(&(cls.as_ptr() as usize));
        let constructor = Py::new(
            py,
            ErrorConstructor {
                owner: cls.clone().unbind(),
                operation: cls.name()?.to_string(),
                allocate: unsafe {
                    std::mem::transmute::<*mut std::ffi::c_void, pyo3::ffi::newfunc>(allocate)
                },
            },
        )?;
        let construct =
            support(py, "_constructor")?.call1((constructor, cls.getattr("__new__")?))?;
        cls.setattr("__new__", staticmethod.call1((construct,))?)?;
        cls.setattr("_quantsupport_constructor", true)?;
        if let Some(signature) = signature {
            cls.setattr(
                "__signature__",
                support(py, "_constructor_signature")?.call1((cls, signature))?,
            )?;
        }
    }
    Ok(())
}

/// Construct an exception using the shared Python schema and native category.
pub fn native(error: &QSError) -> PyErr {
    Python::attach(|py| {
        let (code, variant) = error.kind();
        let result = make(py, code, &error.to_string(), None, None, None, None);
        let value = result.value(py);
        let _ = value.setattr("native_type", variant);
        if let Ok(contexts) = pyo3::types::PyTuple::new(py, error.contexts()) {
            let _ = value.setattr("contexts", contexts);
        }
        if let QSError::InvalidTypeErr { expected, actual } = error.root_cause() {
            let _ = value.setattr("expected", expected);
            let _ = value.setattr("actual", actual);
        }
        if let QSError::Context { source, .. } = error {
            result.set_cause(py, Some(native(source)));
        } else if let QSError::IoErr(cause) = error {
            result.set_cause(
                py,
                Some(pyo3::exceptions::PyOSError::new_err(cause.to_string())),
            );
        } else if let Some(cause) = std::error::Error::source(error) {
            result.set_cause(
                py,
                Some(pyo3::exceptions::PyValueError::new_err(cause.to_string())),
            );
        }
        result
    })
}

fn support<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    // The exception type is shared even when a native test constructs a module
    // directly, rather than importing the public package.
    py.get_type::<crate::QuantSupportError>().getattr(name)
}

pub fn make(
    py: Python<'_>,
    code: &str,
    message: &str,
    operation: Option<&str>,
    path: Option<&str>,
    expected: Option<&str>,
    actual: Option<&str>,
) -> PyErr {
    let create = || -> PyResult<PyErr> {
        let kwargs = PyDict::new(py);
        kwargs.set_item("code", code)?;
        kwargs.set_item("operation", operation)?;
        kwargs.set_item("path", path)?;
        kwargs.set_item("expected", expected)?;
        kwargs.set_item("actual", actual)?;
        Ok(PyErr::from_value(
            support(py, "_make_error")?.call((message,), Some(&kwargs))?,
        ))
    };
    create().unwrap_or_else(|_| crate::QuantSupportError::new_err(message.to_owned()))
}

pub fn invalid_value(message: impl std::fmt::Display) -> PyErr {
    native(&QSError::InvalidValueErr(message.to_string()))
}

pub fn state_error(message: impl std::fmt::Display) -> PyErr {
    native(&QSError::EvaluationErr(message.to_string()))
}

pub fn not_found(message: impl std::fmt::Display) -> PyErr {
    native(&QSError::NotFoundErr(message.to_string()))
}

pub fn invalid_type(obj: &Bound<'_, PyAny>, expected: &str, path: Option<&str>) -> PyErr {
    let actual = obj
        .get_type()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_default();
    let error = native(&QSError::InvalidTypeErr {
        expected: expected.into(),
        actual,
    });
    context(obj.py(), error, None, path)
}

/// Add context without flattening the original category, attributes, or cause.
pub fn context(py: Python<'_>, error: PyErr, operation: Option<&str>, path: Option<&str>) -> PyErr {
    if error.is_instance_of::<crate::QuantSupportError>(py) {
        let matches = |name: &str, requested: Option<&str>| {
            requested.is_none_or(|requested| {
                error
                    .value(py)
                    .getattr(name)
                    .and_then(|v| v.extract::<String>())
                    .is_ok_and(|v| v == requested)
            })
        };
        if matches("operation", operation) && matches("path", path) {
            return error;
        }
    }
    let convert = || -> PyResult<PyErr> {
        let value = support(py, "_with_context")?.call1((error.value(py), operation, path))?;
        Ok(PyErr::from_value(value))
    };
    match convert() {
        Ok(result) => {
            result.set_cause(py, Some(error));
            result
        }
        Err(_) => error,
    }
}

pub fn initialize(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "QuantSupportError",
        module.py().get_type::<crate::QuantSupportError>(),
    )?;
    let base = module.py().get_type::<crate::QuantSupportError>();
    if let Ok(namespace) = base.getattr("_support_namespace") {
        for name in [
            "QuantSupportTypeError",
            "QuantSupportValueError",
            "QuantSupportStateError",
            "QuantSupportIOError",
            "QuantSupportNotImplementedError",
        ] {
            module.add(name, namespace.get_item(name)?)?;
        }
        install_constructors(module)?;
        namespace
            .get_item("_install_error_boundaries")?
            .call1((module.dict(),))?;
        return Ok(());
    }
    let source =
        std::ffi::CString::new(include_str!(concat!(env!("OUT_DIR"), "/_error_support.py")))
            .expect("error support contains no NUL bytes");
    module.py().run(&source, Some(&module.dict()), None)?;
    let initialized = module.getattr("_initialize")?.call1((&base,))?;
    for (name, value) in initialized.cast::<PyDict>()?.iter() {
        module.dict().set_item(name, value)?;
    }
    for name in [
        "_make_error",
        "_with_context",
        "_constructor_signature",
        "_constructor",
    ] {
        let method = module
            .py()
            .import("builtins")?
            .getattr("staticmethod")?
            .call1((module.getattr(name)?,))?;
        base.setattr(name, method)?;
    }
    base.setattr("_support_namespace", module.dict())?;
    install_constructors(module)?;
    module
        .getattr("_install_error_boundaries")?
        .call1((module.dict(),))?;
    Ok(())
}

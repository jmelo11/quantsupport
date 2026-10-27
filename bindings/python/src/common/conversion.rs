//! Conversion helpers between Python objects and quantsupport types.
//!
//! JSON-compatible Python objects (dicts, lists, strings) are converted into
//! Rust types through their serde implementations via `pythonize`.

use crate::common::errors;
use pyo3::{
    prelude::*,
    types::{PyDict, PyList, PyTuple},
};
use quantsupport::{
    prelude::{
        BusinessDayConvention, Calendar, CapFloorType, CapletFloorletType, Compounding, Currency,
        Date, DateGenerationRule, DayCounter, EuroOptionType, Frequency, Interpolator, Level,
        MarketIndex, PaymentStructure, Period, Request, Side, Strike, TimeUnit,
    },
    utils::errors::QSError,
};
use serde::de::DeserializeOwned;

/// Maps any library error into the Python `QuantSupportError` exception.
pub fn qs_err<E: std::fmt::Display + 'static>(e: E) -> PyErr {
    let value = &e as &dyn std::any::Any;
    if let Some(error) = value.downcast_ref::<QSError>() {
        errors::native(error)
    } else if value.is::<pythonize::PythonizeError>() || value.is::<serde_json::Error>() {
        errors::native(&QSError::SerializationErr(e.to_string()))
    } else if let Some(error) = value.downcast_ref::<PyErr>() {
        Python::attach(|py| errors::context(py, error.clone_ref(py), None, None))
    } else {
        errors::native(&QSError::UnexpectedErr(e.to_string()))
    }
}

/// Deserializes a JSON-compatible Python object (dict/list/str/...) into `T`.
pub fn from_py<T: DeserializeOwned>(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<T> {
    let input = serde_input(obj).map_err(|e| errors::context(obj.py(), e, Some(what), None))?;
    let mut deserializer = pythonize::Depythonizer::from_object(&input);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let mut leaf = Some(input.clone());
        for segment in error.path() {
            leaf = leaf.and_then(|value| match segment {
                serde_path_to_error::Segment::Seq { index } => value.get_item(*index).ok(),
                serde_path_to_error::Segment::Map { key } => value.get_item(key).ok(),
                serde_path_to_error::Segment::Enum { variant } => value.get_item(variant).ok(),
                serde_path_to_error::Segment::Unknown => None,
            });
        }
        let cause: PyErr = error.into_inner().into();
        let result = errors::native(&QSError::DeserializationErr(format!(
            "invalid {what}: {cause}"
        )));
        if let Some(value) = leaf {
            if let Ok(actual) = value.get_type().name() {
                let _ = result.value(obj.py()).setattr("actual", actual);
            }
        }
        result.set_cause(obj.py(), Some(cause));
        errors::context(
            obj.py(),
            result,
            None,
            (path != "." && !path.is_empty()).then_some(path.as_str()),
        )
    })
}

/// Validate dictionary inputs before PyO3 can emit an unlabelled cast error.
pub fn expect_dict<'a, 'py>(
    obj: &'a Bound<'py, PyAny>,
    what: &str,
) -> PyResult<&'a Bound<'py, PyDict>> {
    obj.cast::<PyDict>().map_err(|cause| {
        let error = errors::invalid_type(obj, "dict", None);
        error.set_cause(obj.py(), Some(cause.into()));
        errors::context(obj.py(), error, Some(what), None)
    })
}

/// Attach a field path while retaining the original Python exception type.
pub fn at_path(py: Python<'_>, path: &str, error: PyErr) -> PyErr {
    let (operation, path) = path
        .split_once('.')
        .map_or((None, path), |(op, path)| (Some(op), path));
    let nested = error
        .value(py)
        .getattr("path")
        .ok()
        .and_then(|value| value.extract::<String>().ok());
    let path = nested.filter(|value| !value.is_empty()).map_or_else(
        || path.to_owned(),
        |value| {
            if value.starts_with('[') {
                format!("{path}{value}")
            } else {
                format!("{path}.{value}")
            }
        },
    );
    errors::context(py, error, operation, Some(&path))
}

/// Recursively normalizes typed terms, dates, enums, and containers for serde.
///
/// Converts loan term wrappers through their native tagged representations,
/// allowing typed and plain serialized inputs to share the same deserializer.
///
/// # Errors
/// Propagates Python attribute/container access and serialization errors.
fn serde_input<'py>(obj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    macro_rules! enum_input {
        ($($name:ident),+ $(,)?) => { $(
            if let Ok(value) = obj.extract::<crate::common::enums::$name>() {
                return format!("{value:?}").into_pyobject(obj.py()).map(Bound::into_any).map_err(Into::into);
            }
        )+ };
    }
    enum_input!(
        Level,
        Interpolator,
        Currency,
        Side,
        Compounding,
        Frequency,
        DayCounter,
        TimeUnit,
        BusinessDayConvention,
        DateGenerationRule,
        Request,
        VolatilityType,
        SmileType,
        ScenarioType,
        OptionType,
        CapFloorType,
        CapletFloorletType,
        PaymentStructure
    );
    if let Ok(index) = obj.extract::<crate::common::enums::MarketIndex>() {
        return pythonize::pythonize(obj.py(), &index.inner).map_err(qs_err);
    }
    macro_rules! loan_terms_input {
        ($($name:ident),+ $(,)?) => { $(
            if let Ok(value) = obj.extract::<crate::instruments::loan_terms::$name>() {
                return pythonize::pythonize(obj.py(), &value.inner).map_err(qs_err);
            }
        )+ };
    }
    loan_terms_input!(LoanRepaymentStructure, PrepaymentModelConfiguration);
    if obj.is_instance_of::<crate::time::Date>()
        || obj.is_instance_of::<crate::time::Period>()
        || obj.is_instance_of::<crate::time::DateTime>()
    {
        return obj.str().map(Bound::into_any);
    }
    if let Ok(curve) = obj.extract::<crate::market::curves::ZeroRateTermStructure>() {
        return pythonize::pythonize(obj.py(), &curve.inner).map_err(qs_err);
    }
    if let Ok(curve) = obj.extract::<crate::market::curves::TenorBasedZeroRateTermStructure>() {
        return pythonize::pythonize(obj.py(), curve.inner.curve()).map_err(qs_err);
    }
    if let Ok(dictionary) = obj.cast::<PyDict>() {
        let result = PyDict::new(obj.py());
        for (key, value) in dictionary.iter() {
            result.set_item(serde_input(&key)?, serde_input(&value)?)?;
        }
        return Ok(result.into_any());
    }
    let items = if let Ok(list) = obj.cast::<PyList>() {
        Some(list.iter().collect::<Vec<_>>())
    } else {
        obj.cast::<PyTuple>()
            .ok()
            .map(|tuple| tuple.iter().collect::<Vec<_>>())
    };
    if let Some(items) = items {
        let values = items
            .iter()
            .map(serde_input)
            .collect::<PyResult<Vec<_>>>()?;
        return PyList::new(obj.py(), values).map(Bound::into_any);
    }
    Ok(obj.clone())
}

/// Builds a `pandas.DataFrame` from named columns.
pub fn dataframe<'py>(
    py: Python<'py>,
    columns: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let pandas = py.import("pandas").map_err(|_| {
        errors::native(&QSError::NotFoundErr(
            "pandas is required for DataFrame results (pip install pandas)".into(),
        ))
    })?;
    let data = PyDict::new(py);
    for (name, col) in columns {
        data.set_item(*name, col)?;
    }
    pandas.getattr("DataFrame")?.call1((data,))
}

/// Parses a plain string into a serde-deserializable enum (e.g. `"USD"` → `Currency::USD`).
pub fn parse_str_enum<T: DeserializeOwned>(s: &str, what: &str) -> PyResult<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|e| errors::native(&QSError::ParsingErr(format!("invalid {what} '{s}': {e}"))))
}

/// Defines an extractor accepting either the Python mirror enum or its
/// string form, returning the library type.
macro_rules! enum_extractor {
    ($fn_name:ident, $py:ty, $qs:ty, $what:literal) => {
        #[doc = concat!("Extracts a `", $what, "` from a mirror enum or string.")]
        pub fn $fn_name(obj: &Bound<'_, PyAny>) -> PyResult<$qs> {
            if let Ok(v) = obj.extract::<$py>() {
                return Ok(v.into());
            }
            if let Ok(s) = obj.extract::<String>() {
                return <$py>::parse(&s).map(Into::into);
            }
            Err(errors::invalid_type(
                obj,
                concat!($what, " enum or string"),
                Some(&$what.replace(' ', "_")),
            ))
        }
    };
}

enum_extractor!(
    extract_interpolator,
    crate::common::enums::Interpolator,
    Interpolator,
    "interpolator"
);

enum_extractor!(
    extract_level,
    crate::common::enums::Level,
    Level,
    "quote level"
);
enum_extractor!(
    extract_currency,
    crate::common::enums::Currency,
    Currency,
    "currency"
);
enum_extractor!(extract_side, crate::common::enums::Side, Side, "side");
enum_extractor!(
    extract_compounding,
    crate::common::enums::Compounding,
    Compounding,
    "compounding"
);
enum_extractor!(
    extract_frequency,
    crate::common::enums::Frequency,
    Frequency,
    "frequency"
);
enum_extractor!(
    extract_day_counter,
    crate::common::enums::DayCounter,
    DayCounter,
    "day counter"
);
enum_extractor!(
    extract_time_unit,
    crate::common::enums::TimeUnit,
    TimeUnit,
    "time unit"
);
enum_extractor!(
    extract_business_day_convention,
    crate::common::enums::BusinessDayConvention,
    BusinessDayConvention,
    "business day convention"
);
enum_extractor!(
    extract_option_type,
    crate::common::enums::OptionType,
    EuroOptionType,
    "option type"
);
enum_extractor!(
    extract_cap_floor_type,
    crate::common::enums::CapFloorType,
    CapFloorType,
    "cap/floor type"
);
enum_extractor!(
    extract_caplet_floorlet_type,
    crate::common::enums::CapletFloorletType,
    CapletFloorletType,
    "caplet/floorlet type"
);
enum_extractor!(
    extract_payment_structure,
    crate::common::enums::PaymentStructure,
    PaymentStructure,
    "payment structure"
);

/// Extracts a `Strike` from a float, `"ATM"`, or the native tagged schema.
pub fn extract_strike(obj: &Bound<'_, PyAny>) -> PyResult<Strike> {
    if let Ok(v) = obj.extract::<f64>() {
        return Ok(Strike::Absolute(v));
    }
    if let Ok(s) = obj.extract::<String>() {
        if s.eq_ignore_ascii_case("atm") {
            return Ok(Strike::Atm);
        }
    }
    if obj.is_instance_of::<PyDict>() {
        return from_py(obj, "strike");
    }
    Err(errors::invalid_type(
        obj,
        "float, 'ATM', or native tagged dict",
        Some("strike"),
    ))
}

/// Extracts a `MarketIndex` from the wrapper class or a plain rate-index
/// string (e.g. `"SOFR"`).
pub fn extract_market_index(obj: &Bound<'_, PyAny>) -> PyResult<MarketIndex> {
    if let Ok(v) = obj.extract::<crate::common::enums::MarketIndex>() {
        return Ok(v.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return parse_str_enum(&s, "market index");
    }
    if obj.is_instance_of::<PyDict>() {
        return from_py(obj, "market index");
    }
    Err(errors::invalid_type(
        obj,
        "MarketIndex, string, or native tagged dict",
        Some("market_index"),
    ))
}

/// Extracts a `Date` from a `Date` object, an ISO string (`"YYYY-MM-DD"`)
/// or a `datetime.date`.
pub fn extract_date(obj: &Bound<'_, PyAny>) -> PyResult<Date> {
    if let Ok(d) = obj.extract::<crate::time::Date>() {
        return Ok(d.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return Date::from_str(&s, "%Y-%m-%d").map_err(qs_err);
    }
    // datetime.date / datetime.datetime duck-typing
    if let (Ok(y), Ok(m), Ok(d)) = (
        obj.getattr("year").and_then(|v| v.extract::<i32>()),
        obj.getattr("month").and_then(|v| v.extract::<u32>()),
        obj.getattr("day").and_then(|v| v.extract::<u32>()),
    ) {
        return Date::from_str(&format!("{y:04}-{m:02}-{d:02}"), "%Y-%m-%d").map_err(qs_err);
    }
    Err(errors::invalid_type(
        obj,
        "Date, 'YYYY-MM-DD' string or datetime.date",
        None,
    ))
}

/// Extracts a `Period` from a `Period` object or a string like `"5Y"`.
pub fn extract_period(obj: &Bound<'_, PyAny>) -> PyResult<Period> {
    if let Ok(p) = obj.extract::<crate::time::Period>() {
        return Ok(p.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return parse_str_enum(&s, "period");
    }
    Err(errors::invalid_type(
        obj,
        "Period or string like '5Y'",
        None,
    ))
}

/// Extracts evaluation requests from `Request` enums or strings.
pub fn extract_requests(reqs: &[Bound<'_, PyAny>]) -> PyResult<Vec<Request>> {
    let requests = reqs
        .iter()
        .map(|obj| {
            if let Ok(r) = obj.extract::<crate::common::enums::Request>() {
                return Ok(r.into());
            }
            if let Ok(s) = obj.extract::<String>() {
                return crate::common::enums::Request::parse(&s).map(Into::into);
            }
            Err(errors::invalid_type(
                obj,
                "Request enum or string",
                Some("requests"),
            ))
        })
        .collect::<PyResult<Vec<Request>>>()?;
    Request::validate(&requests).map_err(qs_err)?;
    Ok(requests)
}

enum_extractor!(
    extract_date_generation_rule,
    crate::common::enums::DateGenerationRule,
    DateGenerationRule,
    "date generation rule"
);
/// Accepts a Python Calendar object or a native calendar name.
pub fn extract_calendar(obj: &Bound<'_, PyAny>) -> PyResult<Calendar> {
    if let Ok(calendar) = obj.extract::<crate::time::Calendar>() {
        return Ok(calendar.inner);
    }
    Calendar::try_from(obj.extract::<String>()?).map_err(qs_err)
}

enum_extractor!(
    extract_swaption_type,
    crate::common::enums::SwaptionType,
    quantsupport::prelude::SwaptionType,
    "swaption type"
);

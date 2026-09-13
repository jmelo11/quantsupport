//! Conversion helpers between Python objects and quantsupport types.
//!
//! JSON-compatible Python objects (dicts, lists, strings) are converted into
//! Rust types through their serde implementations via `pythonize`.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use quantsupport::prelude::{
    BusinessDayConvention, CapFloorType, CapletFloorletType, Compounding, Currency, Date,
    DayCounter, EuroOptionType, Frequency, MarketIndex, PaymentStructure, Period, Request,
    ScenarioType, Side, Strike, TimeUnit,
};
use serde::de::DeserializeOwned;

use crate::QuantSupportError;

/// Maps any library error into the Python `QuantSupportError` exception.
pub fn qs_err<E: std::fmt::Display>(e: E) -> PyErr {
    QuantSupportError::new_err(e.to_string())
}

/// Deserializes a JSON-compatible Python object (dict/list/str/...) into `T`.
pub fn from_py<T: DeserializeOwned>(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<T> {
    pythonize::depythonize(obj)
        .map_err(|e| QuantSupportError::new_err(format!("invalid {what}: {e}")))
}

/// Reads a JSON file and deserializes it into `T`.
pub fn from_json_file<T: DeserializeOwned>(path: &str, what: &str) -> PyResult<T> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        QuantSupportError::new_err(format!("cannot read {what} file '{path}': {e}"))
    })?;
    serde_json::from_str(&text)
        .map_err(|e| QuantSupportError::new_err(format!("invalid {what} in '{path}': {e}")))
}

/// Builds a `pandas.DataFrame` from named columns.
pub fn dataframe<'py>(
    py: Python<'py>,
    columns: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let pandas = py.import("pandas").map_err(|_| {
        QuantSupportError::new_err("pandas is required for DataFrame results (pip install pandas)")
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
        .map_err(|e| QuantSupportError::new_err(format!("invalid {what} '{s}': {e}")))
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
            Err(QuantSupportError::new_err(format!(
                concat!(
                    "invalid ",
                    $what,
                    ": expected a ",
                    $what,
                    " enum or string, got {}"
                ),
                obj.get_type().name()?
            )))
        }
    };
}

enum_extractor!(
    extract_currency,
    crate::enums::Currency,
    Currency,
    "currency"
);
enum_extractor!(extract_side, crate::enums::Side, Side, "side");
enum_extractor!(
    extract_compounding,
    crate::enums::Compounding,
    Compounding,
    "compounding"
);
enum_extractor!(
    extract_frequency,
    crate::enums::Frequency,
    Frequency,
    "frequency"
);
enum_extractor!(
    extract_day_counter,
    crate::enums::DayCounter,
    DayCounter,
    "day counter"
);
enum_extractor!(
    extract_time_unit,
    crate::enums::TimeUnit,
    TimeUnit,
    "time unit"
);
enum_extractor!(
    extract_business_day_convention,
    crate::enums::BusinessDayConvention,
    BusinessDayConvention,
    "business day convention"
);
enum_extractor!(
    extract_scenario_type,
    crate::enums::ScenarioType,
    ScenarioType,
    "scenario type"
);
enum_extractor!(
    extract_option_type,
    crate::enums::OptionType,
    EuroOptionType,
    "option type"
);
enum_extractor!(
    extract_cap_floor_type,
    crate::enums::CapFloorType,
    CapFloorType,
    "cap/floor type"
);
enum_extractor!(
    extract_caplet_floorlet_type,
    crate::enums::CapletFloorletType,
    CapletFloorletType,
    "caplet/floorlet type"
);
enum_extractor!(
    extract_payment_structure,
    crate::enums::PaymentStructure,
    PaymentStructure,
    "payment structure"
);

/// Extracts a `Strike` from a float (absolute strike) or the string `"ATM"`.
pub fn extract_strike(obj: &Bound<'_, PyAny>) -> PyResult<Strike> {
    if let Ok(v) = obj.extract::<f64>() {
        return Ok(Strike::Absolute(v));
    }
    if let Ok(s) = obj.extract::<String>() {
        if s.eq_ignore_ascii_case("atm") {
            return Ok(Strike::Atm);
        }
    }
    Err(QuantSupportError::new_err(format!(
        "invalid strike: expected a float or 'ATM', got {}",
        obj.get_type().name()?
    )))
}

/// Extracts a `MarketIndex` from the wrapper class or a plain rate-index
/// string (e.g. `"SOFR"`).
pub fn extract_market_index(obj: &Bound<'_, PyAny>) -> PyResult<MarketIndex> {
    if let Ok(v) = obj.extract::<crate::enums::MarketIndex>() {
        return Ok(v.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return parse_str_enum(&s, "market index");
    }
    Err(QuantSupportError::new_err(format!(
        "invalid market index: expected a MarketIndex or string, got {}",
        obj.get_type().name()?
    )))
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
    Err(QuantSupportError::new_err(format!(
        "invalid date: expected a Date, 'YYYY-MM-DD' string or datetime.date, got {}",
        obj.get_type().name()?
    )))
}

/// Extracts a `Period` from a `Period` object or a string like `"5Y"`.
pub fn extract_period(obj: &Bound<'_, PyAny>) -> PyResult<Period> {
    if let Ok(p) = obj.extract::<crate::time::Period>() {
        return Ok(p.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return parse_str_enum(&s, "period");
    }
    Err(QuantSupportError::new_err(format!(
        "invalid period: expected a Period or string like '5Y', got {}",
        obj.get_type().name()?
    )))
}

/// Extracts evaluation requests from `Request` enums or strings.
pub fn extract_requests(reqs: &[Bound<'_, PyAny>]) -> PyResult<Vec<Request>> {
    reqs.iter()
        .map(|obj| {
            if let Ok(r) = obj.extract::<crate::enums::Request>() {
                return Ok(r.into());
            }
            if let Ok(s) = obj.extract::<String>() {
                return crate::enums::Request::parse(&s).map(Into::into);
            }
            Err(QuantSupportError::new_err(format!(
                "invalid request: expected a Request enum or string, got {}",
                obj.get_type().name()?
            )))
        })
        .collect()
}

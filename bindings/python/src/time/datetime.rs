//! Python wrapper for the library's UTC datetime value.

use pyo3::prelude::*;
use quantsupport::prelude::DateTime as QsDateTime;

use crate::{common::conversion::qs_err, time::Date};

/// A UTC date and time, backed by the library's chrono-based DateTime.
/// Parsing accepts offset-aware RFC 3339 timestamps and preserves nanoseconds.
#[pyclass(module = "quantsupport", eq, ord, frozen, hash, from_py_object)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DateTime {
    pub(crate) inner: QsDateTime,
}

/// Accepts the native value, an RFC 3339 string, or an aware Python datetime.
pub(crate) fn extract_datetime(value: &Bound<'_, PyAny>) -> PyResult<QsDateTime> {
    if let Ok(datetime) = value.extract::<DateTime>() {
        return Ok(datetime.inner);
    }
    let text = value
        .extract::<String>()
        .or_else(|_| value.call_method0("isoformat")?.extract::<String>())?;
    text.parse().map_err(qs_err)
}

#[pymethods]
impl DateTime {
    #[new]
    #[pyo3(signature = (year, month, day, hour = 0, minute = 0, second = 0))]
    fn new(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> PyResult<Self> {
        Ok(Self {
            inner: QsDateTime::new(year, month, day, hour, minute, second).map_err(qs_err)?,
        })
    }

    /// Parses an offset-aware timestamp; fmt follows chrono's strftime syntax.
    #[staticmethod]
    #[pyo3(signature = (s, fmt = None))]
    fn parse(s: &str, fmt: Option<&str>) -> PyResult<Self> {
        let inner = fmt
            .map_or_else(|| s.parse(), |format| QsDateTime::from_str(s, format))
            .map_err(qs_err)?;
        Ok(Self { inner })
    }

    /// Combines a library Date and a UTC clock time.
    #[staticmethod]
    #[pyo3(signature = (date, hour = 0, minute = 0, second = 0))]
    fn from_date(date: &Bound<'_, PyAny>, hour: u32, minute: u32, second: u32) -> PyResult<Self> {
        Ok(Self {
            inner: QsDateTime::from_date(
                crate::common::conversion::extract_date(date)?,
                hour,
                minute,
                second,
            )
            .map_err(qs_err)?,
        })
    }

    /// Converts an aware Python datetime to the library value.
    #[staticmethod]
    fn from_datetime(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: extract_datetime(value)?,
        })
    }

    /// UTC calendar date as a library Date.
    #[getter]
    fn date(&self) -> Date {
        Date {
            inner: self.inner.date(),
        }
    }

    #[getter]
    fn year(&self) -> i32 {
        self.inner.year()
    }

    #[getter]
    fn month(&self) -> u32 {
        self.inner.month()
    }

    #[getter]
    fn day(&self) -> u32 {
        self.inner.day()
    }

    #[getter]
    fn hour(&self) -> u32 {
        self.inner.hour()
    }

    #[getter]
    fn minute(&self) -> u32 {
        self.inner.minute()
    }

    #[getter]
    fn second(&self) -> u32 {
        self.inner.second()
    }

    #[getter]
    fn nanosecond(&self) -> u32 {
        self.inner.nanosecond()
    }

    /// Formats UTC using chrono's strftime syntax, or RFC 3339 by default.
    #[pyo3(signature = (fmt = None))]
    fn to_str(&self, fmt: Option<&str>) -> String {
        fmt.map_or_else(
            || self.inner.to_string(),
            |format| self.inner.to_str(format),
        )
    }

    /// Converts to an aware Python datetime (microsecond precision).
    fn to_datetime<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        py.import("datetime")?
            .getattr("datetime")?
            .call_method1("fromisoformat", (self.inner.to_string(),))
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("DateTime('{}')", self.inner)
    }
}

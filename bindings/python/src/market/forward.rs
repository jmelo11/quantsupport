//! Forward-market assumptions exposed as a serializable Python value.

use pyo3::{pyclass, pymethods, types::PyAny, Bound, PyResult, Python};
use quantsupport::core::forwardmarket::configuration::{
    CurveRoll, FixingRoll, ForwardMarketConfiguration as NativeConfiguration, VolatilityRoll,
};

use crate::{
    common::conversion::{extract_date, extract_market_index, from_py, qs_err},
    time::Date,
};

/// Assumptions for a deterministic future pricing context.
///
/// ``curve_roll`` accepts ``ImpliedForward`` or ``ConstantTenor``;
/// ``volatility_roll`` accepts ``ConstantTenor`` or ``ForwardVariance``;
/// ``fixing_roll`` accepts ``Project`` or ``Preserve``. Quotes and historical
/// fixings belong to the source market; the result has independent horizon risk.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct ForwardMarketConfiguration {
    pub inner: NativeConfiguration,
}

#[pymethods]
impl ForwardMarketConfiguration {
    #[new]
    #[pyo3(signature = (target_date, curve_roll = "ImpliedForward", volatility_roll = "ConstantTenor", fixing_roll = "Project"))]
    fn new(
        target_date: &Bound<'_, PyAny>,
        curve_roll: &str,
        volatility_roll: &str,
        fixing_roll: &str,
    ) -> PyResult<Self> {
        let curve = match curve_roll.to_ascii_lowercase().as_str() {
            "impliedforward" => CurveRoll::ImpliedForward,
            "constanttenor" => CurveRoll::ConstantTenor,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "curve_roll must be ImpliedForward or ConstantTenor",
                ))
            }
        };
        let volatility = match volatility_roll.to_ascii_lowercase().as_str() {
            "constanttenor" => VolatilityRoll::ConstantTenor,
            "forwardvariance" => VolatilityRoll::ForwardVariance,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "volatility_roll must be ConstantTenor or ForwardVariance",
                ))
            }
        };
        let fixings = match fixing_roll.to_ascii_lowercase().as_str() {
            "project" => FixingRoll::Project,
            "preserve" => FixingRoll::Preserve,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "fixing_roll must be Project or Preserve",
                ))
            }
        };
        Ok(Self {
            inner: NativeConfiguration::new(extract_date(target_date)?)
                .with_curve_roll(curve)
                .with_volatility(volatility)
                .with_fixings(fixings),
        })
    }

    /// Restores the complete native schema, including per-index overrides and export grids.
    #[staticmethod]
    fn from_dict(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: from_py(value, "forward market configuration")?,
        })
    }

    /// Returns the serializable native configuration.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &self.inner).map_err(qs_err)
    }

    /// Horizon evaluation date.
    #[getter]
    fn target_date(&self) -> Date {
        Date {
            inner: self.inner.target_date(),
        }
    }

    /// Returns a configuration with a different curve convention for one index.
    fn with_curve_override(&self, market_index: &Bound<'_, PyAny>, roll: &str) -> PyResult<Self> {
        let roll = match roll.to_ascii_lowercase().as_str() {
            "impliedforward" => CurveRoll::ImpliedForward,
            "constanttenor" => CurveRoll::ConstantTenor,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "Expected ImpliedForward or ConstantTenor",
                ))
            }
        };
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_curve_override(extract_market_index(market_index)?, roll),
        })
    }
}

crate::common::representation::impl_dict_protocol!(ForwardMarketConfiguration);

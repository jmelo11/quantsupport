//! Reusable dated and tenor-based zero-rate term structures.

use pyo3::prelude::*;
use quantsupport::prelude::{
    Compounding, DayCounter, Frequency, InterestRatesTermStructure, Interpolator, RateDefinition,
    TenorBasedZeroRateTermStructure as QsTenorCurve, ZeroRateTermStructure as QsZeroCurve,
};

use crate::common::enums::Interpolator as PyInterpolator;

use crate::common::conversion::{
    extract_compounding, extract_date, extract_day_counter, extract_frequency,
    extract_interpolator, extract_period, from_json_file, from_py, qs_err,
};

fn rate_definition(
    day_counter: Option<&Bound<'_, PyAny>>,
    compounding: Option<&Bound<'_, PyAny>>,
    frequency: Option<&Bound<'_, PyAny>>,
) -> PyResult<RateDefinition> {
    Ok(RateDefinition::new(
        day_counter
            .map(extract_day_counter)
            .transpose()?
            .unwrap_or(DayCounter::Actual365),
        compounding
            .map(extract_compounding)
            .transpose()?
            .unwrap_or(Compounding::Continuous),
        frequency
            .map(extract_frequency)
            .transpose()?
            .unwrap_or(Frequency::Annual),
    ))
}

/// A zero-rate curve with dated pillars.
#[pyclass(
    module = "quantsupport",
    name = "ZeroRateTermStructure",
    from_py_object
)]
#[derive(Clone)]
pub struct ZeroRateTermStructure {
    pub inner: QsZeroCurve,
}

impl ZeroRateTermStructure {
    fn curve(&self) -> &QsZeroCurve {
        &self.inner
    }
}

#[pymethods]
impl ZeroRateTermStructure {
    #[new]
    #[pyo3(signature = (reference_date, dates, rates, day_counter = None, compounding = None, frequency = None, interpolator = None, enable_extrapolation = false))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        reference_date: &Bound<'_, PyAny>,
        dates: Vec<Bound<'_, PyAny>>,
        rates: Vec<f64>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        frequency: Option<&Bound<'_, PyAny>>,
        interpolator: Option<&Bound<'_, PyAny>>,
        enable_extrapolation: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: QsZeroCurve::new(
                extract_date(reference_date)?,
                dates
                    .iter()
                    .map(extract_date)
                    .collect::<PyResult<Vec<_>>>()?,
                rates,
                rate_definition(day_counter, compounding, frequency)?,
                interpolator
                    .map(extract_interpolator)
                    .transpose()?
                    .unwrap_or(Interpolator::Linear),
                enable_extrapolation,
            )
            .map_err(qs_err)?,
        })
    }

    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: from_py(obj, "zero-rate curve")?,
        })
    }

    #[staticmethod]
    fn from_json(path: &str) -> PyResult<Self> {
        Ok(Self {
            inner: from_json_file(path, "zero-rate curve")?,
        })
    }
}

/// A zero-rate curve with tenor pillars.
#[pyclass(
    module = "quantsupport",
    name = "TenorBasedZeroRateTermStructure",
    from_py_object
)]
#[derive(Clone)]
pub struct TenorBasedZeroRateTermStructure {
    pub inner: QsTenorCurve,
}

impl TenorBasedZeroRateTermStructure {
    fn curve(&self) -> &QsZeroCurve {
        self.inner.curve()
    }
}

#[pymethods]
impl TenorBasedZeroRateTermStructure {
    #[new]
    #[pyo3(signature = (reference_date, tenors, rates, day_counter = None, compounding = None, frequency = None, interpolator = None, enable_extrapolation = false))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        reference_date: &Bound<'_, PyAny>,
        tenors: Vec<Bound<'_, PyAny>>,
        rates: Vec<f64>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        frequency: Option<&Bound<'_, PyAny>>,
        interpolator: Option<&Bound<'_, PyAny>>,
        enable_extrapolation: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: QsTenorCurve::new(
                extract_date(reference_date)?,
                tenors
                    .iter()
                    .map(extract_period)
                    .collect::<PyResult<Vec<_>>>()?,
                rates,
                rate_definition(day_counter, compounding, frequency)?,
                interpolator
                    .map(extract_interpolator)
                    .transpose()?
                    .unwrap_or(Interpolator::Linear),
                enable_extrapolation,
            )
            .map_err(qs_err)?,
        })
    }

    #[getter]
    fn tenors(&self) -> Vec<crate::time::Period> {
        self.inner
            .tenors()
            .iter()
            .map(|tenor| crate::time::Period { inner: *tenor })
            .collect()
    }
}

macro_rules! curve_methods {
    ($name:ident) => {
        #[pymethods]
        impl $name {
            #[getter]
            fn reference_date(&self) -> crate::time::Date {
                crate::time::Date {
                    inner: self.curve().reference_date(),
                }
            }

            #[getter]
            fn dates(&self) -> Vec<crate::time::Date> {
                self.curve()
                    .dates()
                    .iter()
                    .map(|date| crate::time::Date { inner: *date })
                    .collect()
            }

            #[getter]
            fn rates(&self) -> Vec<f64> {
                self.curve().rates().clone()
            }

            #[getter]
            fn day_counter(&self) -> crate::common::enums::DayCounter {
                self.curve().rate_definition().day_counter().into()
            }

            #[getter]
            fn compounding(&self) -> crate::common::enums::Compounding {
                self.curve().rate_definition().compounding().into()
            }

            #[getter]
            fn frequency(&self) -> crate::common::enums::Frequency {
                self.curve().rate_definition().frequency().into()
            }

            #[getter]
            fn interpolator(&self) -> PyInterpolator {
                self.curve().interpolator().into()
            }

            #[getter]
            fn enable_extrapolation(&self) -> bool {
                self.curve().enable_extrapolation()
            }

            fn zero_rate(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
                self.curve().zero_rate(extract_date(date)?).map_err(qs_err)
            }

            fn zero_rate_from_time(&self, time: f64) -> PyResult<f64> {
                self.curve().zero_rate_from_time(time).map_err(qs_err)
            }

            fn discount_factor(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
                self.curve()
                    .discount_factor(extract_date(date)?)
                    .map_err(qs_err)
            }

            fn discount_factor_from_time(&self, time: f64) -> PyResult<f64> {
                self.curve().discount_factor_from_time(time).map_err(qs_err)
            }

            #[pyo3(signature = (start_date, end_date, compounding = None, frequency = None))]
            fn forward_rate(
                &self,
                start_date: &Bound<'_, PyAny>,
                end_date: &Bound<'_, PyAny>,
                compounding: Option<&Bound<'_, PyAny>>,
                frequency: Option<&Bound<'_, PyAny>>,
            ) -> PyResult<f64> {
                self.curve()
                    .forward_rate(
                        extract_date(start_date)?,
                        extract_date(end_date)?,
                        compounding
                            .map(extract_compounding)
                            .transpose()?
                            .unwrap_or(self.curve().rate_definition().compounding()),
                        frequency
                            .map(extract_frequency)
                            .transpose()?
                            .unwrap_or(self.curve().rate_definition().frequency()),
                    )
                    .map_err(qs_err)
            }

            fn forward_rate_from_time(&self, start: f64, end: f64) -> PyResult<f64> {
                self.curve()
                    .forward_rate_from_time(start, end)
                    .map_err(qs_err)
            }

            /// Exports the equivalent dated curve.
            #[allow(clippy::wrong_self_convention)]
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                pythonize::pythonize(py, self.curve()).map_err(qs_err)
            }
        }
    };
}

curve_methods!(ZeroRateTermStructure);
curve_methods!(TenorBasedZeroRateTermStructure);

pub fn extract_zero_curve(obj: &Bound<'_, PyAny>) -> PyResult<QsZeroCurve> {
    if let Ok(curve) = obj.extract::<ZeroRateTermStructure>() {
        return Ok(curve.inner);
    }
    if let Ok(curve) = obj.extract::<TenorBasedZeroRateTermStructure>() {
        return Ok(curve.inner.curve().clone());
    }
    from_py(obj, "zero-rate curve")
}

crate::common::representation::impl_dict_protocol!(
    ZeroRateTermStructure,
    TenorBasedZeroRateTermStructure
);

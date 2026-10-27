//! Market data and configuration wrappers.
//!
//! Typed Python constructors wrap the library's native market definitions.
//! Serialized inputs remain available through `from_dict` and `from_dict_list`.

use crate::common::conversion::{
    extract_currency, extract_date, extract_day_counter, extract_interpolator,
    extract_market_index, from_py, qs_err,
};
use pyo3::prelude::*;
use quantsupport::prelude::{
    CreditCurveConfiguration as QsCreditCurveConfiguration,
    CurveConfiguration as QsCurveConfiguration, DayCounter, DualFwd, FixingStore as QsFixingStore,
    FxRateRecord, FxStore as QsFxStore, InstrumentType, Interpolator,
    PricerConfiguration as QsPricerConfiguration, PricerImplementation, QuoteStore as QsQuoteStore,
    QuoteStoreRecords, SimulationConfiguration as QsSimulationConfiguration,
    VolatilityCubeConfiguration as QsVolatilityCubeConfiguration,
    VolatilitySurfaceConfiguration as QsVolatilitySurfaceConfiguration,
};
use serde::de::DeserializeOwned;

/// Accepts a JSON value that is either a list of `T`, a single `T`, or an
/// object wrapping a single list (e.g. `{"curve_specs": [...]}`).
fn list_from_value<T: DeserializeOwned>(value: serde_json::Value, what: &str) -> PyResult<Vec<T>> {
    let parse = |v: serde_json::Value| -> Result<Vec<T>, serde_json::Error> {
        match v {
            serde_json::Value::Array(_) => serde_json::from_value(v),
            serde_json::Value::Object(ref map)
                if map.len() == 1
                    && map.values().next().is_some_and(serde_json::Value::is_array) =>
            {
                let inner = v.as_object().unwrap().values().next().unwrap().clone();
                serde_json::from_value(inner)
            }
            _ => serde_json::from_value::<T>(v).map(|one| vec![one]),
        }
    };
    parse(value).map_err(|e| {
        crate::common::errors::native(&quantsupport::utils::errors::QSError::DeserializationErr(
            format!("invalid {what}: {e}"),
        ))
    })
}

/// Defines a Python wrapper class around a serde-deserializable library
/// configuration struct, with `from_dict` (single) and `from_dict_list` (list)
/// constructors.
macro_rules! config_wrapper {
    ($(#[$docs:meta])* $name:ident, $inner:ty, $pyname:literal, $what:literal) => {
        #[doc = concat!("Wrapper around the library's `", stringify!($inner), "`.")]
        $(#[$docs])*
        #[pyclass(module = "quantsupport", name = $pyname, from_py_object)]
        #[derive(Clone)]
        pub struct $name {
            pub inner: $inner,
        }

        #[pymethods]
        impl $name {
            /// Builds a single configuration from a dict.
            #[staticmethod]
            fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
                Ok(Self {
                    inner: from_py(obj, $what)?,
                })
            }

            /// Builds configurations from a list, single dict, or wrapped list.
            #[staticmethod]
            fn from_dict_list(obj: &Bound<'_, PyAny>) -> PyResult<Vec<Self>> {
                let value: serde_json::Value = from_py(obj, $what)?;
                Ok(list_from_value::<$inner>(value, $what)?
                    .into_iter()
                    .map(|inner| Self { inner })
                    .collect())
            }



            /// Returns the configuration as a dict.
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                crate::common::representation::to_dict(py, &self.inner)
            }

        }
    };
}

config_wrapper!(
    /// Define a curve for construction by PricingContext.
    ///
    /// Parameters
    /// ----------
    /// market_index : MarketIndex or str
    ///     Identity used to request the curve for projection or discounting.
    /// quotes : sequence of str, optional
    ///     Calibration instruments, DiscountFactor_CCY_Index_Pillar, or
    ///     ZeroRate_CCY_Index_Pillar[_Compounding[_Frequency]] identifiers.
    ///     Direct curves use one quote family; pillars are tenors or ISO dates.
    /// day_counter : DayCounter or str, optional
    ///     Curve time basis. Defaults to Actual360.
    /// interpolator : Interpolator or str, optional
    ///     Linear, LogLinear, or CubicSpline. Defaults to LogLinear.
    /// enable_extrapolation : bool, optional
    ///     Allow evaluation beyond the supplied pillars. Defaults to True.
    CurveConfiguration,
    QsCurveConfiguration,
    "CurveConfiguration",
    "curve configuration"
);

#[pymethods]
impl CurveConfiguration {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (market_index, quotes = None,
        day_counter = None, interpolator = None, enable_extrapolation = true))]
    fn new(
        market_index: &Bound<'_, PyAny>,
        quotes: Option<Vec<String>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        interpolator: Option<&Bound<'_, PyAny>>,
        enable_extrapolation: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: QsCurveConfiguration::new(
                extract_market_index(market_index)?,
                day_counter
                    .map(extract_day_counter)
                    .transpose()?
                    .unwrap_or(DayCounter::Actual360),
                interpolator
                    .map(extract_interpolator)
                    .transpose()?
                    .unwrap_or(Interpolator::LogLinear),
                enable_extrapolation,
                quotes.unwrap_or_default(),
            ),
        })
    }

    /// Define a composite curve by multiplying its base and spread discount factors.
    ///
    /// Parameters
    /// ----------
    /// market_index : MarketIndex or str
    ///     Identity used to request the resulting composite curve.
    /// base_index : MarketIndex or str
    ///     Benchmark curve configured in the same pricing context.
    /// spread_index : MarketIndex or str
    ///     Spread curve configured in the same context. The component curves
    ///     must share a reference date and day-count convention.
    #[staticmethod]
    #[pyo3(signature = (market_index, base_index, spread_index))]
    fn composite(
        market_index: &Bound<'_, PyAny>,
        base_index: &Bound<'_, PyAny>,
        spread_index: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: QsCurveConfiguration::composite(
                extract_market_index(market_index)?,
                extract_market_index(base_index)?,
                extract_market_index(spread_index)?,
            ),
        })
    }
}
config_wrapper!(
    CreditCurveConfiguration,
    QsCreditCurveConfiguration,
    "CreditCurveConfiguration",
    "credit curve configuration"
);
config_wrapper!(
    VolatilitySurfaceConfiguration,
    QsVolatilitySurfaceConfiguration,
    "VolatilitySurfaceConfiguration",
    "volatility surface configuration"
);
config_wrapper!(
    VolatilityCubeConfiguration,
    QsVolatilityCubeConfiguration,
    "VolatilityCubeConfiguration",
    "volatility cube configuration"
);
config_wrapper!(
    PricerConfiguration,
    QsPricerConfiguration,
    "PricerConfiguration",
    "pricer configuration"
);

#[pymethods]
impl PricerConfiguration {
    /// Configure the core Rust loan pricer with typed Python inputs.
    ///
    /// Parameters
    /// ----------
    /// refinancing_benchmark_index : MarketIndex or str, optional
    ///     Index benchmark for refinancing-incentive prepayment. Defaults to
    ///     None, supplying no benchmark. Its metadata sets the observation tenor
    ///     and rate convention; it excludes replacement-loan spreads and fees.
    /// rebuild_amortization_for_fair_rate : bool, optional
    ///     Defaults to False, preserving contractual principal during fair-rate
    ///     trials. True rebuilds the trial amortization from its initial all-in
    ///     rate. Neither setting changes the original contract.
    ///
    /// Returns
    /// -------
    /// PricerConfiguration
    ///     Native Loan configuration for PricingContext.pricer_configurations.
    ///     Discounting follows context.fixed_income_policy.
    #[staticmethod]
    #[pyo3(signature = (refinancing_benchmark_index = None, rebuild_amortization_for_fair_rate = false))]
    fn loan(
        refinancing_benchmark_index: Option<&Bound<'_, PyAny>>,
        rebuild_amortization_for_fair_rate: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: QsPricerConfiguration::new(
                InstrumentType::Loan,
                PricerImplementation::Loan {
                    refinancing_benchmark_index: refinancing_benchmark_index
                        .map(extract_market_index)
                        .transpose()?,
                    rebuild_amortization_for_fair_rate,
                },
                Vec::new(),
            ),
        })
    }
}

config_wrapper!(
    SimulationConfiguration,
    QsSimulationConfiguration,
    "SimulationConfiguration",
    "simulation configuration"
);

/// Market quotes with a reference date.
///
/// Created from `{"reference_date": "YYYY-MM-DD", "quotes": [{"identifier": ..., "mid": ...}]}`.
#[pyclass(module = "quantsupport", name = "QuoteStore", from_py_object)]
#[derive(Clone)]
pub struct QuoteStore {
    pub inner: QsQuoteStore,
}

#[pymethods]
impl QuoteStore {
    /// Builds a quote store from a dict.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let records: QuoteStoreRecords = from_py(obj, "quotes")?;
        Ok(Self {
            inner: QsQuoteStore::try_from(records).map_err(qs_err)?,
        })
    }

    /// Reference date.
    #[getter]
    fn reference_date(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.inner.reference_date(),
        }
    }

    /// Sorted quote identifiers.
    fn identifiers(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.inner.quotes().keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Returns one quote level by identifier (`"mid"`, `"bid"`, or `"ask"`).
    #[pyo3(signature = (identifier, level = "mid"))]
    fn value(&self, identifier: &str, level: &str) -> PyResult<f64> {
        let quote = self.inner.quote(identifier).ok_or_else(|| {
            crate::common::errors::not_found(format!("quote '{identifier}' was not found"))
        })?;
        let value = match level.to_ascii_lowercase().as_str() {
            "mid" => quote.levels().mid(),
            "bid" => quote.levels().bid(),
            "ask" => quote.levels().ask(),
            _ => {
                return Err(crate::common::errors::invalid_value(format!(
                    "invalid quote level '{level}' (expected 'mid', 'bid', or 'ask')"
                )))
            }
        };
        value.ok_or_else(|| {
            crate::common::errors::not_found(format!("quote '{identifier}' has no {level} level"))
        })
    }

    /// Returns `(mid, bid, ask)` for one quote identifier.
    fn levels(&self, identifier: &str) -> PyResult<(Option<f64>, Option<f64>, Option<f64>)> {
        let quote = self.inner.quote(identifier).ok_or_else(|| {
            crate::common::errors::not_found(format!("quote '{identifier}' was not found"))
        })?;
        Ok((
            quote.levels().mid(),
            quote.levels().bid(),
            quote.levels().ask(),
        ))
    }

    /// All quotes as a DataFrame with columns `identifier`, `mid`, `bid`,
    /// `ask`, sorted by identifier.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        type QuoteRow<'a> = (&'a String, Option<f64>, Option<f64>, Option<f64>);
        let mut rows: Vec<QuoteRow<'_>> = self
            .inner
            .quotes()
            .iter()
            .map(|(id, q)| (id, q.levels().mid(), q.levels().bid(), q.levels().ask()))
            .collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        let ids: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
        let mids: Vec<Option<f64>> = rows.iter().map(|r| r.1).collect();
        let bids: Vec<Option<f64>> = rows.iter().map(|r| r.2).collect();
        let asks: Vec<Option<f64>> = rows.iter().map(|r| r.3).collect();
        crate::common::conversion::dataframe(
            py,
            &[
                ("identifier", ids.into_pyobject(py)?.into_any()),
                ("mid", mids.into_pyobject(py)?.into_any()),
                ("bid", bids.into_pyobject(py)?.into_any()),
                ("ask", asks.into_pyobject(py)?.into_any()),
            ],
        )
    }

    fn __len__(&self) -> usize {
        self.inner.quotes().len()
    }

    fn __repr__(&self) -> String {
        format!(
            "QuoteStore(reference_date='{}', quotes={})",
            self.inner.reference_date(),
            self.inner.quotes().len()
        )
    }
}

/// Historical index fixings.
#[pyclass(module = "quantsupport", name = "FixingStore", from_py_object)]
#[derive(Clone)]
pub struct FixingStore {
    pub inner: QsFixingStore,
}

#[pymethods]
impl FixingStore {
    /// Creates an empty fixing store.
    #[new]
    fn new() -> Self {
        Self {
            inner: QsFixingStore::default(),
        }
    }

    /// Builds a fixing store from a dict.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: from_py(obj, "fixings")?,
        })
    }

    /// Adds or replaces a fixing.
    fn add(
        &mut self,
        index: &Bound<'_, PyAny>,
        date: &Bound<'_, PyAny>,
        value: f64,
    ) -> PyResult<()> {
        self.inner
            .add_fixing(&extract_market_index(index)?, extract_date(date)?, value);
        Ok(())
    }

    /// Returns one historical fixing.
    fn fixing(&self, index: &Bound<'_, PyAny>, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .fixing(&extract_market_index(index)?, extract_date(date)?)
            .map_err(qs_err)
    }

    /// All fixings for an index as a DataFrame with `date` and `value` columns.
    fn to_dataframe<'py>(
        &self,
        py: Python<'py>,
        index: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let index = extract_market_index(index)?;
        let fixings = self.inner.fixings(&index).map_err(qs_err)?;
        let dates: Vec<String> = fixings.keys().map(ToString::to_string).collect();
        let values: Vec<f64> = fixings.values().copied().collect();
        crate::common::conversion::dataframe(
            py,
            &[
                ("date", dates.into_pyobject(py)?.into_any()),
                ("value", values.into_pyobject(py)?.into_any()),
            ],
        )
    }

    fn __repr__(&self) -> String {
        "FixingStore(...)".to_string()
    }
}

/// FX spot rates. Records are `{"base": "CLP", "quote": "USD", "rate": 0.00111}`,
/// meaning 1 base = rate quote.
#[pyclass(module = "quantsupport", name = "FxStore", from_py_object)]
#[derive(Clone, Default)]
pub struct FxStore {
    pub inner: QsFxStore,
}

#[pymethods]
impl FxStore {
    /// Creates an empty FX store.
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// Builds an FX store from a list of rate records.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let records: Vec<FxRateRecord> = from_py(obj, "fx rates")?;
        Ok(Self {
            inner: QsFxStore::from_records(records),
        })
    }

    /// Adds a spot rate: 1 `base` = `rate` `quote`.
    fn add(
        &mut self,
        base: &Bound<'_, PyAny>,
        quote: &Bound<'_, PyAny>,
        rate: f64,
    ) -> PyResult<()> {
        self.inner.add_fx_rate(
            extract_currency(base)?,
            extract_currency(quote)?,
            DualFwd::from(rate),
        );
        Ok(())
    }

    /// Returns a direct, inverse, or triangulated FX rate.
    fn rate(&self, base: &Bound<'_, PyAny>, quote: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .get_fx_rate(extract_currency(base)?, extract_currency(quote)?)
            .map(|rate| rate.value())
            .map_err(qs_err)
    }

    fn __repr__(&self) -> String {
        "FxStore(...)".to_string()
    }
}

crate::common::representation::impl_dict_protocol!(
    CurveConfiguration,
    CreditCurveConfiguration,
    VolatilitySurfaceConfiguration,
    VolatilityCubeConfiguration,
    SimulationConfiguration,
    PricerConfiguration
);

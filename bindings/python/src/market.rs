//! Market data and configuration wrappers.
//!
//! Typed Python constructors wrap the library's native market definitions.
//! Serialized inputs remain available through `from_dict` and `from_json`.

use pyo3::prelude::*;
use quantsupport::prelude::{
    Compounding, CreditCurveConfiguration as QsCreditCurveConfiguration,
    CurveConfiguration as QsCurveConfiguration, CurveConstruction as QsCurveConstruction,
    CurvePillar, DayCounter, DualFwd, FixingStore as QsFixingStore, Frequency, FxRateRecord,
    FxStore as QsFxStore, Interpolator, PricerConfiguration as QsPricerConfiguration,
    QuoteStore as QsQuoteStore, QuoteStoreRecords, Scenario as QsScenario,
    ScenarioType as QsScenarioType, SimulationConfiguration as QsSimulationConfiguration,
    VolatilityCubeConfiguration as QsVolatilityCubeConfiguration,
    VolatilitySurfaceConfiguration as QsVolatilitySurfaceConfiguration,
};
use serde::de::DeserializeOwned;

use crate::conv::{
    extract_compounding, extract_currency, extract_date, extract_day_counter, extract_frequency,
    extract_market_index, extract_period, from_json_file, from_py, parse_str_enum, qs_err,
};
use crate::QuantSupportError;

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
    parse(value).map_err(|e| QuantSupportError::new_err(format!("invalid {what}: {e}")))
}

/// Defines a Python wrapper class around a serde-deserializable library
/// configuration struct, with `from_dict` (single) and `from_json` (list)
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

            /// Loads configurations from a JSON file. Accepts a list, a
            /// single object, or an object wrapping a single list (e.g.
            /// `{"curve_specs": [...]}`). Always returns a list.
            #[staticmethod]
            fn from_json(path: &str) -> PyResult<Vec<Self>> {
                let value: serde_json::Value = from_json_file(path, $what)?;
                Ok(list_from_value::<$inner>(value, $what)?
                    .into_iter()
                    .map(|inner| Self { inner })
                    .collect())
            }

            /// Returns the configuration as a dict.
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                pythonize::pythonize(py, &self.inner).map_err(|e| {
                    QuantSupportError::new_err(format!("cannot serialize {}: {e}", $what))
                })
            }

            fn __repr__(&self) -> String {
                format!(
                    "{}({})",
                    $pyname,
                    serde_json::to_string(&self.inner).unwrap_or_default()
                )
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
    ///     Calibration quote identifiers. Used with bootstrapped construction.
    /// construction : CurveConstruction, optional
    ///     Bootstrapped, explicit zero-rate, or composite construction rule.
    ///     Defaults to bootstrapped construction.
    /// day_counter : DayCounter or str, optional
    ///     Curve time basis. Defaults to Actual360.
    /// interpolator : str, optional
    ///     Linear, LogLinear, or CubicSpline. Defaults to LogLinear.
    /// enable_extrapolation : bool, optional
    ///     Allow evaluation beyond the supplied pillars. Defaults to True.
    CurveConfiguration,
    QsCurveConfiguration,
    "CurveConfiguration",
    "curve configuration"
);

/// Construction rule used by CurveConfiguration.
///
/// Use bootstrapped(), zero_rate(), or composite() to select the rule.
/// Each rule wraps the corresponding native CurveConstruction variant.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct CurveConstruction {
    pub inner: QsCurveConstruction,
}

#[pymethods]
impl CurveConstruction {
    /// Calibrate the curve to the quote identifiers in CurveConfiguration.quotes.
    #[staticmethod]
    fn bootstrapped() -> Self {
        Self {
            inner: QsCurveConstruction::Bootstrapped,
        }
    }

    /// Interpolate supplied annual zero rates.
    ///
    /// Parameters
    /// ----------
    /// pillars : sequence of Date, datetime.date, Period, or str
    ///     Ordered dates or tenors measured from the context reference date.
    /// rates : sequence of float
    ///     Decimal annual rates, one per pillar (0.01 means 1%).
    /// compounding : Compounding or str, optional
    ///     Convention for the supplied rates. Defaults to Continuous.
    /// frequency : Frequency or str, optional
    ///     Compounding frequency. Defaults to Annual.
    #[staticmethod]
    #[pyo3(signature = (*, pillars, rates, compounding = None, frequency = None))]
    fn zero_rate(
        pillars: Vec<Bound<'_, PyAny>>,
        rates: Vec<f64>,
        compounding: Option<&Bound<'_, PyAny>>,
        frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let pillars = pillars
            .iter()
            .map(|pillar| {
                extract_date(pillar)
                    .map(CurvePillar::Date)
                    .or_else(|_| extract_period(pillar).map(CurvePillar::Tenor))
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: QsCurveConstruction::ZeroRate {
                pillars,
                rates,
                compounding: compounding
                    .map(extract_compounding)
                    .transpose()?
                    .unwrap_or(Compounding::Continuous),
                frequency: frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Annual),
            },
        })
    }

    /// Multiply the discount factors of two configured curves.
    ///
    /// Parameters
    /// ----------
    /// base_index : MarketIndex or str
    ///     Benchmark curve present in the context's curve configurations.
    /// spread_index : MarketIndex or str
    ///     Spread curve present in the same configurations. Continuous zero
    ///     spreads add to the benchmark's continuous zero rates.
    #[staticmethod]
    #[pyo3(signature = (*, base_index, spread_index))]
    fn composite(base_index: &Bound<'_, PyAny>, spread_index: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: QsCurveConstruction::Composite {
                base_index: extract_market_index(base_index)?,
                spread_index: extract_market_index(spread_index)?,
            },
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "CurveConstruction({})",
            serde_json::to_string(&self.inner).unwrap_or_default()
        )
    }
}

#[pymethods]
impl CurveConfiguration {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (*, market_index, quotes = None, construction = None,
        day_counter = None, interpolator = "LogLinear", enable_extrapolation = true))]
    fn new(
        market_index: &Bound<'_, PyAny>,
        quotes: Option<Vec<String>>,
        construction: Option<CurveConstruction>,
        day_counter: Option<&Bound<'_, PyAny>>,
        interpolator: &str,
        enable_extrapolation: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: QsCurveConfiguration::new(
                extract_market_index(market_index)?,
                day_counter
                    .map(extract_day_counter)
                    .transpose()?
                    .unwrap_or(DayCounter::Actual360),
                parse_str_enum::<Interpolator>(interpolator, "interpolator")?,
                enable_extrapolation,
                quotes.unwrap_or_default(),
            )
            .with_construction(construction.map_or(QsCurveConstruction::Bootstrapped, |c| c.inner)),
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

    /// Loads a quote store from a JSON file.
    #[staticmethod]
    fn from_json(path: &str) -> PyResult<Self> {
        let records: QuoteStoreRecords = from_json_file(path, "quotes")?;
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
            QuantSupportError::new_err(format!("quote '{identifier}' was not found"))
        })?;
        let value = match level.to_ascii_lowercase().as_str() {
            "mid" => quote.levels().mid(),
            "bid" => quote.levels().bid(),
            "ask" => quote.levels().ask(),
            _ => {
                return Err(QuantSupportError::new_err(format!(
                    "invalid quote level '{level}' (expected 'mid', 'bid', or 'ask')"
                )))
            }
        };
        value.ok_or_else(|| {
            QuantSupportError::new_err(format!("quote '{identifier}' has no {level} level"))
        })
    }

    /// Returns `(mid, bid, ask)` for one quote identifier.
    fn levels(&self, identifier: &str) -> PyResult<(Option<f64>, Option<f64>, Option<f64>)> {
        let quote = self.inner.quote(identifier).ok_or_else(|| {
            QuantSupportError::new_err(format!("quote '{identifier}' was not found"))
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
        crate::conv::dataframe(
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

    /// Loads a fixing store from a JSON file.
    #[staticmethod]
    fn from_json(path: &str) -> PyResult<Self> {
        Ok(Self {
            inner: from_json_file(path, "fixings")?,
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
        crate::conv::dataframe(
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

    /// Loads FX rate records from a JSON file (a list of records).
    #[staticmethod]
    fn from_json(path: &str) -> PyResult<Self> {
        let records: Vec<FxRateRecord> = from_json_file(path, "fx rates")?;
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

/// A shock applied to one or more quotes before bootstrapping.
///
/// The target is either a full quote identifier (`"OIS_USD_SOFR_1Y"`) or a
/// partial underscore-separated selector matched against the identifier
/// segments of every quote (`"SOFR"` shocks all SOFR quotes — a parallel
/// shift; `"Swaption_USD"` shocks the USD swaption vol slide).
///
/// ```python
/// bump_all_sofr = qs.Scenario("SOFR", 0.0001, qs.ScenarioType.Absolute)
/// bump_1y_pillar = qs.Scenario("OIS_USD_SOFR_1Y", 0.0001)  # Absolute default
/// scale_vols = qs.Scenario("Swaption_USD", 0.10, "Relative")
/// ```
#[pyclass(module = "quantsupport", name = "Scenario", from_py_object)]
#[derive(Clone)]
pub struct Scenario {
    pub inner: QsScenario,
}

#[pymethods]
impl Scenario {
    #[new]
    #[pyo3(signature = (target, shock, scenario_type = None))]
    fn new(target: String, shock: f64, scenario_type: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let scenario_type = scenario_type
            .map(crate::conv::extract_scenario_type)
            .transpose()?
            .unwrap_or(QsScenarioType::Absolute);
        Ok(Self {
            inner: QsScenario::new(target, shock, scenario_type),
        })
    }

    /// Builds a scenario from a dict
    /// `{"target": ..., "shock": ..., "scenario_type": ...}`.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: from_py(obj, "scenario")?,
        })
    }

    /// Quote identifier or partial (segment) selector.
    #[getter]
    fn target(&self) -> &str {
        self.inner.target()
    }

    /// Shock size.
    #[getter]
    fn shock(&self) -> f64 {
        self.inner.shock()
    }

    /// How the shock is applied.
    #[getter]
    fn scenario_type(&self) -> crate::enums::ScenarioType {
        self.inner.scenario_type().into()
    }

    fn __repr__(&self) -> String {
        format!(
            "Scenario(target='{}', shock={}, scenario_type={:?})",
            self.inner.target(),
            self.inner.shock(),
            self.inner.scenario_type()
        )
    }
}

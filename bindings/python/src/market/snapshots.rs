//! Timestamped market archive bindings.

use pyo3::prelude::*;
use quantsupport::prelude::{
    MarketSnapshot as QsMarketSnapshot, MarketSnapshotStore as QsMarketSnapshotStore,
    SnapshotSelectionRule as QsSnapshotSelectionRule, SnapshotSelector as QsSnapshotSelector,
};

use crate::{
    common::conversion::{extract_level, from_json_file, from_py, qs_err},
    market::stores::{FixingStore, QuoteStore},
    pricing::{collateral::FixedIncomeDiscountPolicy, context::PricingContext},
    time::{datetime::extract_datetime, DateTime},
};

/// Timestamp matching policy. Previous never selects a later observation.
#[pyclass(module = "quantsupport", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SnapshotSelectionRule {
    Exact,
    Closest,
    Previous,
}

impl From<SnapshotSelectionRule> for QsSnapshotSelectionRule {
    fn from(rule: SnapshotSelectionRule) -> Self {
        match rule {
            SnapshotSelectionRule::Exact => Self::Exact,
            SnapshotSelectionRule::Closest => Self::Closest,
            SnapshotSelectionRule::Previous => Self::Previous,
        }
    }
}

impl From<QsSnapshotSelectionRule> for SnapshotSelectionRule {
    fn from(rule: QsSnapshotSelectionRule) -> Self {
        match rule {
            QsSnapshotSelectionRule::Exact => Self::Exact,
            QsSnapshotSelectionRule::Closest => Self::Closest,
            QsSnapshotSelectionRule::Previous => Self::Previous,
        }
    }
}

fn selection_rule(value: &Bound<'_, PyAny>) -> PyResult<QsSnapshotSelectionRule> {
    if let Ok(rule) = value.extract::<SnapshotSelectionRule>() {
        return Ok(rule.into());
    }
    match value.extract::<String>()?.to_lowercase().as_str() {
        "exact" => Ok(QsSnapshotSelectionRule::Exact),
        "closest" => Ok(QsSnapshotSelectionRule::Closest),
        "previous" => Ok(QsSnapshotSelectionRule::Previous),
        _ => Err(crate::common::errors::invalid_value(
            "Expected Exact, Closest, or Previous",
        )),
    }
}

/// Select a market instant and quote level. Defaults to Previous and Mid.
/// Closest may select future data; ties prefer the earlier snapshot.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct SnapshotSelector {
    inner: QsSnapshotSelector,
}

#[pymethods]
impl SnapshotSelector {
    /// Returns timestamp, selection rule, and quote level as a dictionary.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::common::representation::to_dict(py, &self.inner)
    }

    #[new]
    #[pyo3(signature = (timestamp, rule = None, level = None))]
    fn new(
        timestamp: &Bound<'_, PyAny>,
        rule: Option<&Bound<'_, PyAny>>,
        level: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: QsSnapshotSelector::new(extract_datetime(timestamp)?)
                .with_rule(rule.map(selection_rule).transpose()?.unwrap_or_default())
                .with_level(level.map(extract_level).transpose()?.unwrap_or_default()),
        })
    }

    /// Requested observation instant as a library DateTime.
    #[getter]
    fn timestamp(&self) -> DateTime {
        DateTime {
            inner: self.inner.timestamp(),
        }
    }

    #[getter]
    fn rule(&self) -> SnapshotSelectionRule {
        self.inner.rule().into()
    }

    #[getter]
    fn level(&self) -> crate::common::enums::Level {
        self.inner.level().into()
    }
}

/// Serializable base market observations and construction inputs at one instant.
/// Curves are rebuilt from configurations; runtime curves are not pickled.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct MarketSnapshot {
    inner: QsMarketSnapshot,
}

#[pymethods]
impl MarketSnapshot {
    /// Capture a context's base inputs, excluding fixings and temporary scenarios.
    /// FX spots remain side-independent; zero rates use the selected quote side.
    #[staticmethod]
    fn from_context(timestamp: &Bound<'_, PyAny>, context: &PricingContext) -> PyResult<Self> {
        Ok(Self {
            inner: QsMarketSnapshot::from_context(
                extract_datetime(timestamp)?,
                context.base_context(),
            )
            .map_err(qs_err)?,
        })
    }

    /// Restore the native serialized snapshot schema.
    #[staticmethod]
    fn from_dict(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: from_py(value, "market snapshot")?,
        })
    }

    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &self.inner).map_err(qs_err)
    }

    /// Matched observation instant as a library DateTime.
    #[getter]
    fn timestamp(&self) -> DateTime {
        DateTime {
            inner: self.inner.timestamp(),
        }
    }

    /// Business reference date used when pricing this market.
    #[getter]
    fn reference_date(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.inner.quotes().reference_date(),
        }
    }

    #[getter]
    fn quotes(&self) -> QuoteStore {
        QuoteStore {
            inner: self.inner.quotes().clone(),
        }
    }
}

/// Archive coherent markets by observation instant, with JSON persistence.
#[pyclass(module = "quantsupport")]
pub struct MarketSnapshotStore {
    inner: QsMarketSnapshotStore,
}

#[pymethods]
impl MarketSnapshotStore {
    #[new]
    fn new() -> Self {
        Self {
            inner: QsMarketSnapshotStore::new(),
        }
    }

    /// Insert a snapshot. Duplicate observation instants are errors.
    fn insert(&mut self, snapshot: MarketSnapshot) -> PyResult<()> {
        self.inner.insert(snapshot.inner).map_err(qs_err)
    }

    /// Available observation instants in ascending order, as library DateTime values.
    #[getter]
    fn timestamps(&self) -> Vec<DateTime> {
        self.inner
            .timestamps()
            .into_iter()
            .map(|inner| DateTime { inner })
            .collect()
    }

    /// Resolve one coherent market; inspect timestamp for the actual match.
    fn select(&self, selector: &SnapshotSelector) -> PyResult<MarketSnapshot> {
        Ok(MarketSnapshot {
            inner: self.inner.select(&selector.inner).map_err(qs_err)?.clone(),
        })
    }

    /// Create an independent context at the selected market's business date.
    /// Fixings are supplied separately; no publication-vintage filtering occurs.
    #[pyo3(signature = (selector, fixings = None, fixed_income_policy = None))]
    fn pricing_context(
        &self,
        selector: &SnapshotSelector,
        fixings: Option<FixingStore>,
        fixed_income_policy: Option<FixedIncomeDiscountPolicy>,
    ) -> PyResult<PricingContext> {
        let context = self
            .inner
            .pricing_context(
                &selector.inner,
                fixings.map(|store| store.inner).unwrap_or_default(),
            )
            .map_err(qs_err)?;
        Ok(PricingContext::from_snapshot_context(
            context,
            fixed_income_policy,
        ))
    }

    /// Load the timestamp-ordered JSON snapshot array.
    #[staticmethod]
    fn from_json(path: &str) -> PyResult<Self> {
        Ok(Self {
            inner: from_json_file(path, "market snapshot archive")?,
        })
    }

    /// Save all market observations and configurations, excluding runtime AD data.
    fn to_json(&self, path: &str) -> PyResult<()> {
        let json = serde_json::to_string_pretty(&self.inner).map_err(qs_err)?;
        std::fs::write(path, json).map_err(|error| {
            crate::common::errors::native(&quantsupport::utils::errors::QSError::IoErr(error))
        })
    }

    /// Load a snapshots dictionary or legacy list, rejecting duplicate timestamps.
    #[staticmethod]
    fn from_dict(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let snapshots = value.get_item("snapshots").ok();
        Ok(Self {
            inner: from_py(
                snapshots.as_ref().unwrap_or(value),
                "market snapshot archive",
            )?,
        })
    }

    /// Return a dictionary containing the snapshot list.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &serde_json::json!({"snapshots": self.inner})).map_err(qs_err)
    }
}

crate::common::representation::impl_dict_protocol!(
    SnapshotSelector,
    MarketSnapshot,
    MarketSnapshotStore
);

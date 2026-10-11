//! XVA configuration and results.
//!
//! - [`XvaConfig`] wraps the library's `XvaEngineConfig` (simulation/model
//!   setup only).
//! - [`CsaTerms`] wraps the library's per-client CSA configuration:
//!   collateral treatment plus credit/funding parameters.
//! - [`NettingSet`] groups trades under one client with its CSA terms.
//! - [`XvaResult`] exposes per-netting-set XVA values and sensitivities as
//!   pandas DataFrames.

use pyo3::prelude::*;
use quantsupport::prelude::{
    CsaTerms as QsCsaTerms, ExposureResult, NettingSet as QsNettingSet,
    XvaEngineConfig as QsXvaEngineConfig,
};

use crate::conv::{dataframe, extract_currency, extract_market_index, from_json_file, from_py};
use crate::trades::{
    BasisSwap, CrossCurrencySwap, FixFloatCrossCurrencySwap, FixedRateBond, FixedRateDeposit,
    FloatingRateNote, Swap,
};
use crate::QuantSupportError;

/// XVA engine configuration (models, paths, seed, frequency).
///
/// Credit, funding and collateral parameters are **not** part of this
/// config — they are per client, see [`CsaTerms`].
#[pyclass(name = "XvaConfig", from_py_object)]
#[derive(Clone)]
pub struct XvaConfig {
    pub inner: QsXvaEngineConfig,
}

#[pymethods]
impl XvaConfig {
    /// Builds the configuration from a dict.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: from_py(obj, "xva config")?,
        })
    }

    /// Loads the configuration from a JSON file.
    #[staticmethod]
    fn from_json(path: &str) -> PyResult<Self> {
        Ok(Self {
            inner: from_json_file(path, "xva config")?,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "XvaConfig(n_paths={}, seed={}, models={})",
            self.inner.n_paths,
            self.inner.seed,
            self.inner.model_configs.len()
        )
    }
}

/// Per-client CSA terms: collateral treatment and credit/funding parameters.
#[pyclass(name = "CsaTerms", from_py_object)]
#[derive(Clone)]
pub struct CsaTerms {
    pub inner: QsCsaTerms,
}

#[pymethods]
impl CsaTerms {
    #[new]
    #[pyo3(signature = (collateral_index, collateral_currency, credit_spread, recovery, funding_spread = 0.0, credit_index = None, funding_spread_curve = None, funding_index = None, *, variation_margin = false, counterparty_threshold = 0.0, own_threshold = 0.0, minimum_transfer_amount = 0.0))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        collateral_index: &Bound<'_, PyAny>,
        collateral_currency: &Bound<'_, PyAny>,
        credit_spread: f64,
        recovery: f64,
        funding_spread: f64,
        credit_index: Option<&Bound<'_, PyAny>>,
        funding_spread_curve: Option<&Bound<'_, PyAny>>,
        funding_index: Option<&Bound<'_, PyAny>>,
        variation_margin: bool,
        counterparty_threshold: f64,
        own_threshold: f64,
        minimum_transfer_amount: f64,
    ) -> PyResult<Self> {
        let funding_spread_curve = funding_spread_curve
            .map(crate::curves::extract_zero_curve)
            .transpose()?;
        let terms = Self {
            inner: QsCsaTerms {
                collateral_index: extract_market_index(collateral_index)?,
                collateral_currency: extract_currency(collateral_currency)?,
                variation_margin,
                counterparty_threshold,
                own_threshold,
                minimum_transfer_amount,
                credit_spread,
                recovery,
                funding_spread,
                funding_spread_curve,
                funding_index: funding_index.map(extract_market_index).transpose()?,
                credit_index: credit_index.map(extract_market_index).transpose()?,
            },
        };
        terms
            .inner
            .validate()
            .map_err(|error| QuantSupportError::new_err(error.to_string()))?;
        Ok(terms)
    }

    /// Builds CSA terms from a dict.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let terms = Self {
            inner: from_py(obj, "csa terms")?,
        };
        terms
            .inner
            .validate()
            .map_err(|error| QuantSupportError::new_err(error.to_string()))?;
        Ok(terms)
    }

    /// Loads CSA terms from a JSON file.
    #[staticmethod]
    fn from_json(path: &str) -> PyResult<Self> {
        let terms = Self {
            inner: from_json_file(path, "csa terms")?,
        };
        terms
            .inner
            .validate()
            .map_err(|error| QuantSupportError::new_err(error.to_string()))?;
        Ok(terms)
    }

    #[getter]
    fn variation_margin(&self) -> bool {
        self.inner.variation_margin
    }

    #[getter]
    fn counterparty_threshold(&self) -> f64 {
        self.inner.counterparty_threshold
    }

    #[getter]
    fn own_threshold(&self) -> f64 {
        self.inner.own_threshold
    }

    #[getter]
    fn minimum_transfer_amount(&self) -> f64 {
        self.inner.minimum_transfer_amount
    }

    #[getter]
    fn collateral_index(&self) -> crate::enums::MarketIndex {
        crate::enums::MarketIndex {
            inner: self.inner.collateral_index.clone(),
        }
    }

    #[getter]
    fn collateral_currency(&self) -> crate::enums::Currency {
        self.inner.collateral_currency.into()
    }

    #[getter]
    fn credit_spread(&self) -> f64 {
        self.inner.credit_spread
    }

    #[getter]
    fn recovery(&self) -> f64 {
        self.inner.recovery
    }

    #[getter]
    fn funding_spread(&self) -> f64 {
        self.inner.funding_spread
    }

    /// The reusable zero-rate spread curve, or `None` when absent.
    #[getter]
    fn funding_spread_curve(&self) -> Option<crate::curves::ZeroRateTermStructure> {
        self.inner
            .funding_spread_curve
            .clone()
            .map(|inner| crate::curves::ZeroRateTermStructure { inner })
    }

    #[getter]
    fn funding_index(&self) -> Option<crate::enums::MarketIndex> {
        self.inner
            .funding_index
            .clone()
            .map(|inner| crate::enums::MarketIndex { inner })
    }

    #[getter]
    fn credit_index(&self) -> Option<crate::enums::MarketIndex> {
        self.inner
            .credit_index
            .clone()
            .map(|inner| crate::enums::MarketIndex { inner })
    }

    fn __repr__(&self) -> String {
        let funding = if let Some(idx) = &self.inner.funding_index {
            format!("funding_index={idx:?}")
        } else if let Some(c) = &self.inner.funding_spread_curve {
            format!("funding_spread_curve=[{} pillars]", c.dates().len())
        } else {
            format!("funding_spread={}", self.inner.funding_spread)
        };
        format!(
            "CsaTerms(collateral_index={:?}, collateral_currency={}, credit_spread={}, recovery={}, {})",
            self.inner.collateral_index,
            self.inner.collateral_currency,
            self.inner.credit_spread,
            self.inner.recovery,
            funding,
        )
    }
}

/// A group of trades under a single netting agreement (one client),
/// carrying the client's [`CsaTerms`].
#[pyclass(name = "NettingSet")]
pub struct NettingSet {
    name: String,
    trades: Vec<Py<crate::extension::Trade>>,
    csa: QsCsaTerms,
}

impl NettingSet {
    /// Builds the library netting set (claims + CSA terms).
    pub fn build(&self, py: Python<'_>) -> PyResult<QsNettingSet> {
        let mut claims = Vec::new();
        for position in &self.trades {
            claims.extend(Self::claims(&position.borrow(py), py)?);
        }
        Ok(QsNettingSet::with_csa_terms(claims, self.csa.clone()))
    }

    fn claims(
        position: &crate::extension::Trade,
        py: Python<'_>,
    ) -> PyResult<Vec<quantsupport::prelude::ContingentClaim>> {
        let instrument = position.instrument_bound(py);
        if let Ok(swap) = instrument.extract::<Swap>() {
            swap.claims(position)
        } else if let Ok(xccy) = instrument.extract::<CrossCurrencySwap>() {
            xccy.claims(position)
        } else if let Ok(basis) = instrument.extract::<BasisSwap>() {
            basis.claims(position)
        } else if let Ok(xccy) = instrument.extract::<FixFloatCrossCurrencySwap>() {
            xccy.claims(position)
        } else if let Ok(bond) = instrument.extract::<FixedRateBond>() {
            bond.claims(position)
        } else if let Ok(frn) = instrument.extract::<FloatingRateNote>() {
            frn.claims(position)
        } else if let Ok(deposit) = instrument.extract::<FixedRateDeposit>() {
            deposit.claims(position)
        } else {
            Err(QuantSupportError::new_err(
                "unsupported instrument type in netting set",
            ))
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[pymethods]
impl NettingSet {
    #[new]
    fn new(name: String, trades: &Bound<'_, PyAny>, csa: CsaTerms) -> PyResult<Self> {
        let mut specs = Vec::new();
        for item in trades.try_iter()? {
            let item = item?;
            let position = item.extract::<Py<crate::extension::Trade>>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err(
                    "NettingSet requires Trade objects, not bare instruments",
                )
            })?;
            Self::claims(&position.borrow(item.py()), item.py())?;
            specs.push(position);
        }
        Ok(Self {
            name,
            trades: specs,
            csa: csa.inner,
        })
    }

    #[getter]
    fn get_name(&self) -> &str {
        &self.name
    }

    fn __len__(&self) -> usize {
        self.trades.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "NettingSet(name='{}', trades={})",
            self.name,
            self.trades.len()
        )
    }
}

/// Per-netting-set exposure profile (EPE / ENE / EE by date).
#[pyclass(name = "ExposureProfile", from_py_object)]
#[derive(Clone)]
pub struct ExposureProfile {
    #[pyo3(get)]
    pub netting_set: String,
    pub dates: Vec<String>,
    pub epe: Vec<f64>,
    pub ene: Vec<f64>,
    pub ee: Vec<f64>,
}

#[pymethods]
impl ExposureProfile {
    /// Exposure profile as a DataFrame with columns date/epe/ene/ee.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        dataframe(
            py,
            &[
                ("date", self.dates.clone().into_pyobject(py)?.into_any()),
                ("epe", self.epe.clone().into_pyobject(py)?.into_any()),
                ("ene", self.ene.clone().into_pyobject(py)?.into_any()),
                ("ee", self.ee.clone().into_pyobject(py)?.into_any()),
            ],
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "ExposureProfile(netting_set='{}', dates={})",
            self.netting_set,
            self.dates.len()
        )
    }
}

/// Result of an XVA engine run.
#[pyclass(name = "XvaResult")]
pub struct XvaResult {
    xva_values: Vec<(String, String, f64)>,
    sensitivities: Vec<(String, f64)>,
    exposures: Vec<ExposureProfile>,
}

impl XvaResult {
    pub fn from_qs(res: &ExposureResult) -> Self {
        let exposures = res
            .cubes
            .iter()
            .map(|cube| ExposureProfile {
                netting_set: cube.trade_id.clone(),
                dates: cube.dates.iter().map(ToString::to_string).collect(),
                epe: cube.epe(),
                ene: cube.ene(),
                ee: cube.ee(),
            })
            .collect();
        let xva_values = res
            .xva_values
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|v| (v.netting_set.clone(), v.measure.clone(), v.value))
            .collect();
        let mut sensitivities: Vec<(String, f64)> = res.sensitivities.clone().unwrap_or_default();
        sensitivities.sort_by(|a, b| a.0.cmp(&b.0));
        Self {
            xva_values,
            sensitivities,
            exposures,
        }
    }
}

#[pymethods]
impl XvaResult {
    /// XVA values as a DataFrame with columns netting_set/measure/value.
    #[getter]
    fn xva_values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut ns = Vec::with_capacity(self.xva_values.len());
        let mut measure = Vec::with_capacity(self.xva_values.len());
        let mut value = Vec::with_capacity(self.xva_values.len());
        for (n, m, v) in &self.xva_values {
            ns.push(n.clone());
            measure.push(m.clone());
            value.push(*v);
        }
        dataframe(
            py,
            &[
                ("netting_set", ns.into_pyobject(py)?.into_any()),
                ("measure", measure.into_pyobject(py)?.into_any()),
                ("value", value.into_pyobject(py)?.into_any()),
            ],
        )
    }

    /// Sensitivities as a DataFrame with columns parameter/value.
    /// Aggregator parameters are prefixed with the netting-set name
    /// (e.g. `"clientA.CVA.credit_spread"`).
    #[getter]
    fn sensitivities<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let (labels, values): (Vec<String>, Vec<f64>) = self.sensitivities.iter().cloned().unzip();
        dataframe(
            py,
            &[
                ("parameter", labels.into_pyobject(py)?.into_any()),
                ("value", values.into_pyobject(py)?.into_any()),
            ],
        )
    }

    /// Per-netting-set exposure profiles.
    #[getter]
    fn exposures(&self) -> Vec<ExposureProfile> {
        self.exposures.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "XvaResult(xva_values={}, exposures={})",
            self.xva_values.len(),
            self.exposures.len()
        )
    }
}

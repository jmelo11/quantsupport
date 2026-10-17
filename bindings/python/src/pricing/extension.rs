//! Python-native extension points for instruments, trades, and pricers.
//!
//! The Rust library deliberately separates product data (`Instrument` / `Trade`),
//! pricing logic (`Pricer`), market state (`PricingContext`), and runtime dispatch
//! (`Evaluator`).  The classes in this module preserve those same boundaries for
//! Python implementations.  Registration is structural: inheriting from these
//! base classes is convenient, but any Python type and any object with a callable
//! `evaluate` method can be registered.

use pyo3::exceptions::PyNotImplementedError;
use pyo3::prelude::*;
use pyo3::types::PyType;
use quantsupport::prelude::Request as QsRequest;

use crate::pricing::context::PricingContext;
use crate::common::conversion::extract_requests;
use crate::pricing::results::EvaluationResults;
use crate::time::Date;
use crate::QuantSupportError;

fn python_type_name(value: &Bound<'_, PyAny>) -> String {
    value
        .get_type()
        .name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "<unknown>".to_string())
}

/// Optional base class for Python market-data providers.
///
/// A provider may expose whichever query methods its pricers need (for
/// example `curve`, `fixing`, or `fx_rate`). The only common contract is an
/// `evaluation_date`. `PricingContext` is the full built-in provider, while
/// ordinary Python objects can be used with the standalone `Evaluator`.
#[pyclass(
    module = "quantsupport",
    name = "MarketDataProvider",
    subclass,
    dict,
    unsendable
)]
pub struct MarketDataProvider;

#[pymethods]
impl MarketDataProvider {
    #[new]
    fn new() -> Self {
        Self
    }

    /// Reference date supplied by a concrete provider.
    #[getter]
    fn evaluation_date(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(
            "MarketDataProvider.evaluation_date must be implemented",
        ))
    }
}

/// Optional base class for user-defined financial products.
///
/// Subclasses may add arbitrary Python attributes and may override
/// `identifier`. Plain Python classes can also be registered directly.
#[pyclass(
    module = "quantsupport",
    name = "Instrument",
    subclass,
    dict,
    unsendable
)]
pub struct Instrument {
    identifier: String,
}

impl Instrument {
    pub fn new(identifier: String) -> Self {
        Self { identifier }
    }
}

#[pymethods]
impl Instrument {
    #[new]
    #[pyo3(signature = (identifier, *_args, **_kwargs))]
    fn new_py(
        identifier: String,
        _args: &Bound<'_, pyo3::types::PyTuple>,
        _kwargs: Option<&Bound<'_, pyo3::types::PyDict>>,
    ) -> Self {
        Self::new(identifier)
    }

    #[pyo3(signature = (identifier, *_args, **_kwargs))]
    fn __init__(
        &mut self,
        identifier: String,
        _args: &Bound<'_, pyo3::types::PyTuple>,
        _kwargs: Option<&Bound<'_, pyo3::types::PyDict>>,
    ) {
        self.identifier = identifier;
    }

    /// Stable product identifier used in evaluation results and diagnostics.
    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!("Instrument(identifier='{}')", self.identifier)
    }
}

/// Generic position in an instrument.
///
/// This mirrors the Rust `Trade` contract while remaining subclassable so a
/// Python trade can carry any product-specific state it needs.
#[pyclass(module = "quantsupport", name = "Trade", subclass, dict, unsendable)]
pub struct Trade {
    instrument: Py<PyAny>,
    trade_date: quantsupport::prelude::Date,
    side: quantsupport::prelude::Side,
    notional: Option<f64>,
    domestic_notional: Option<f64>,
    foreign_notional: Option<f64>,
    num_contracts: f64,
    pricing_anchor: Option<quantsupport::prelude::BondPricingAnchor>,
    settlement_date: Option<quantsupport::prelude::Date>,
    spread_day_counter: Option<quantsupport::prelude::DayCounter>,
}

impl Trade {
    pub const fn trade_date(&self) -> quantsupport::prelude::Date {
        self.trade_date
    }

    pub const fn side(&self) -> quantsupport::prelude::Side {
        self.side
    }

    pub const fn notional(&self) -> Option<f64> {
        self.notional
    }

    pub const fn domestic_notional(&self) -> Option<f64> {
        self.domestic_notional
    }

    pub const fn foreign_notional(&self) -> Option<f64> {
        self.foreign_notional
    }

    pub const fn num_contracts(&self) -> f64 {
        self.num_contracts
    }

    pub const fn pricing_anchor(&self) -> Option<quantsupport::prelude::BondPricingAnchor> {
        self.pricing_anchor
    }

    pub const fn settlement_date(&self) -> Option<quantsupport::prelude::Date> {
        self.settlement_date
    }

    pub const fn spread_day_counter(&self) -> Option<quantsupport::prelude::DayCounter> {
        self.spread_day_counter
    }

    pub fn instrument_bound<'py>(&self, py: Python<'py>) -> &Bound<'py, PyAny> {
        self.instrument.bind(py)
    }

    pub fn bond_anchor(&self) -> PyResult<quantsupport::prelude::BondPricingAnchor> {
        self.pricing_anchor().ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("bond Trade requires pricing_anchor")
        })
    }
}

#[pymethods]
impl Trade {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (instrument, trade_date, side = None, *, notional = None,
        domestic_notional = None, foreign_notional = None, num_contracts = 1.0,
        pricing_anchor = None, settlement_date = None, spread_day_counter = None))]
    fn new(
        instrument: Py<PyAny>,
        trade_date: &Bound<'_, PyAny>,
        side: Option<&Bound<'_, PyAny>>,
        notional: Option<f64>,
        domestic_notional: Option<f64>,
        foreign_notional: Option<f64>,
        num_contracts: f64,
        pricing_anchor: Option<&Bound<'_, PyAny>>,
        settlement_date: Option<&Bound<'_, PyAny>>,
        spread_day_counter: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        for amount in [
            notional,
            domestic_notional,
            foreign_notional,
            Some(num_contracts),
        ]
        .into_iter()
        .flatten()
        {
            if !amount.is_finite() || amount < 0.0 {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "position amounts must be finite and non-negative; use side for direction",
                ));
            }
        }
        Ok(Self {
            instrument,
            trade_date: crate::common::conversion::extract_date(trade_date)?,
            side: side
                .map(crate::common::conversion::extract_side)
                .transpose()?
                .unwrap_or(quantsupport::prelude::Side::LongReceive),
            notional,
            domestic_notional,
            foreign_notional,
            num_contracts,
            pricing_anchor: pricing_anchor
                .map(crate::pricing::bond_anchor::BondPricingAnchorConfig::extract)
                .transpose()?,
            settlement_date: settlement_date.map(crate::common::conversion::extract_date).transpose()?,
            spread_day_counter: spread_day_counter
                .map(crate::common::conversion::extract_day_counter)
                .transpose()?,
        })
    }

    /// Instrument held by this position.
    #[getter]
    fn instrument(&self, py: Python<'_>) -> Py<PyAny> {
        self.instrument.clone_ref(py)
    }

    /// Execution date.
    #[getter(trade_date)]
    fn trade_date_py(&self) -> Date {
        Date {
            inner: self.trade_date,
        }
    }

    /// Pay/receive direction.
    #[getter(side)]
    fn side_py(&self) -> crate::common::enums::Side {
        self.side.into()
    }

    /// Position amount; None uses the instrument's contractual amount.
    #[getter(notional)]
    fn notional_py(&self) -> Option<f64> {
        self.notional
    }

    #[getter(domestic_notional)]
    fn domestic_notional_py(&self) -> Option<f64> {
        self.domestic_notional
    }

    #[getter(foreign_notional)]
    fn foreign_notional_py(&self) -> Option<f64> {
        self.foreign_notional
    }

    #[getter(settlement_date)]
    fn settlement_date_py(&self) -> Option<Date> {
        self.settlement_date.map(|inner| Date { inner })
    }

    #[getter(spread_day_counter)]
    fn spread_day_counter_py(&self) -> Option<crate::common::enums::DayCounter> {
        self.spread_day_counter.map(Into::into)
    }

    #[getter(num_contracts)]
    fn num_contracts_py(&self) -> f64 {
        self.num_contracts
    }

    #[getter(pricing_anchor)]
    fn pricing_anchor_py(&self) -> Option<crate::pricing::bond_anchor::BondPricingAnchorConfig> {
        self.pricing_anchor
            .map(crate::pricing::bond_anchor::BondPricingAnchorConfig::from)
    }

    /// Returns another position in the same instrument with a different bond anchor.
    fn with_pricing_anchor(&self, py: Python<'_>, anchor: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            instrument: self.instrument.clone_ref(py),
            trade_date: self.trade_date,
            side: self.side,
            notional: self.notional,
            domestic_notional: self.domestic_notional,
            foreign_notional: self.foreign_notional,
            num_contracts: self.num_contracts,
            pricing_anchor: Some(crate::pricing::bond_anchor::BondPricingAnchorConfig::extract(
                anchor,
            )?),
            settlement_date: self.settlement_date,
            spread_day_counter: self.spread_day_counter,
        })
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let instrument = self.instrument.bind(py).repr()?.extract::<String>()?;
        Ok(format!(
            "Trade(instrument={instrument}, trade_date='{}', side={:?})",
            self.trade_date, self.side
        ))
    }
}

/// Optional base class for Python pricing implementations.
///
/// Override `evaluate(trade, requests, context)` and return an
/// `EvaluationResults`. The `context` is a market-data provider (normally an
/// initialized `PricingContext`), so the implementation can resolve curves,
/// volatility objects, fixings, FX, or quotes through the same boundary used
/// by built-in pricers.
#[pyclass(module = "quantsupport", name = "Pricer", subclass, dict, unsendable)]
pub struct Pricer {
    discount_policy: Option<Py<PyAny>>,
}

#[pymethods]
impl Pricer {
    #[new]
    fn new() -> Self {
        Self {
            discount_policy: None,
        }
    }

    /// Evaluates a trade. Subclasses must override this method.
    fn evaluate(
        &self,
        _trade: &Bound<'_, PyAny>,
        _requests: &Bound<'_, PyAny>,
        _context: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(
            "Pricer.evaluate(trade, requests, context) must be implemented",
        ))
    }

    /// Describes required market data, if the implementation uses an explicit
    /// request object. Python pricers may also query the context directly.
    fn market_data_request(&self, _trade: &Bound<'_, PyAny>) -> Option<Py<PyAny>> {
        None
    }

    /// Installs a custom discount policy object for the Python pricer.
    fn set_discount_policy(&mut self, policy: Py<PyAny>) {
        self.discount_policy = Some(policy);
    }

    /// Currently installed custom discount policy object.
    #[getter]
    fn discount_policy(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.discount_policy.as_ref().map(|p| p.clone_ref(py))
    }
}

struct Registration {
    trade_type: Py<PyType>,
    pricer: Py<PyAny>,
}

/// Shared implementation used by standalone evaluators and pricing contexts.
#[derive(Default)]
pub struct PricerRegistry {
    registrations: Vec<Registration>,
}

impl PricerRegistry {
    /// Registers or replaces the pricer associated with an exact Python type.
    pub fn register(
        &mut self,
        py: Python<'_>,
        trade_type: &Bound<'_, PyType>,
        pricer: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let evaluate = pricer.getattr("evaluate").map_err(|_| {
            QuantSupportError::new_err(format!(
                "pricer of type '{}' must define evaluate(trade, requests, context)",
                python_type_name(pricer)
            ))
        })?;
        if !evaluate.is_callable() {
            return Err(QuantSupportError::new_err(
                "pricer.evaluate must be callable",
            ));
        }

        if let Some(existing) = self
            .registrations
            .iter_mut()
            .find(|entry| entry.trade_type.bind(py).is(trade_type))
        {
            existing.pricer = pricer.clone().unbind();
            return Ok(());
        }
        self.registrations.push(Registration {
            trade_type: trade_type.clone().unbind(),
            pricer: pricer.clone().unbind(),
        });
        Ok(())
    }

    /// Removes the registration for an exact Python type.
    pub fn unregister(&mut self, py: Python<'_>, trade_type: &Bound<'_, PyType>) -> bool {
        let before = self.registrations.len();
        self.registrations
            .retain(|entry| !entry.trade_type.bind(py).is(trade_type));
        before != self.registrations.len()
    }

    /// Resolves the closest registered type in the trade's method-resolution
    /// order, then falls back to virtual/structural `isinstance` matches.
    pub fn resolve(&self, py: Python<'_>, trade: &Bound<'_, PyAny>) -> PyResult<Option<Py<PyAny>>> {
        let actual_type = trade.get_type();
        let mro = actual_type.getattr("__mro__")?;
        for class in mro.try_iter()? {
            let class = class?;
            if let Some(entry) = self
                .registrations
                .iter()
                .rev()
                .find(|entry| entry.trade_type.bind(py).as_any().is(&class))
            {
                return Ok(Some(entry.pricer.clone_ref(py)));
            }
        }

        // Preserve Python's ABC/virtual-subclass behavior for types that do
        // not appear in `__mro__`. The newest such registration wins.
        for entry in self.registrations.iter().rev() {
            if trade.is_instance(entry.trade_type.bind(py))? {
                return Ok(Some(entry.pricer.clone_ref(py)));
            }
        }
        Ok(None)
    }

    /// Returns the registered pricer for a trade, if any.
    pub fn pricer_for(
        &self,
        py: Python<'_>,
        trade: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.resolve(py, trade)
    }

    /// Number of exact type registrations.
    pub const fn len(&self) -> usize {
        self.registrations.len()
    }
}

fn requests_for_python(requests: &[QsRequest]) -> Vec<crate::common::enums::Request> {
    requests
        .iter()
        .map(|request| match request {
            QsRequest::Value => crate::common::enums::Request::Value,
            QsRequest::YieldToMaturity => crate::common::enums::Request::YieldToMaturity,
            QsRequest::ModifiedDuration => crate::common::enums::Request::ModifiedDuration,
            QsRequest::Sensitivities => crate::common::enums::Request::Sensitivities,
            QsRequest::ZSpread => crate::common::enums::Request::ZSpread,
            QsRequest::OAS => crate::common::enums::Request::OAS,
            QsRequest::Cashflows => crate::common::enums::Request::Cashflows,
            QsRequest::FairRate => crate::common::enums::Request::FairRate,
        })
        .collect()
}

/// Calls a resolved Python pricer and validates its result.
pub fn call_pricer(
    py: Python<'_>,
    pricer: &Py<PyAny>,
    trade: &Bound<'_, PyAny>,
    requests: &[QsRequest],
    context: &Bound<'_, PyAny>,
) -> PyResult<Py<EvaluationResults>> {
    let result = pricer
        .bind(py)
        .call_method1("evaluate", (trade, requests_for_python(requests), context))?;
    result.extract::<Py<EvaluationResults>>().map_err(|_| {
        QuantSupportError::new_err(format!(
            "{}.evaluate must return EvaluationResults, got {}",
            python_type_name(pricer.bind(py)),
            python_type_name(&result)
        ))
    })
}

/// Runtime dispatcher for heterogeneous Python trade collections.
///
/// Registrations are keyed by Python trade type, matching the role of the
/// Rust `Evaluator`'s `TypeId -> ErasedPricer` map.
#[pyclass(module = "quantsupport", name = "Evaluator", unsendable)]
#[derive(Default)]
pub struct Evaluator {
    registry: PricerRegistry,
}

#[pymethods]
impl Evaluator {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// Associates a Python trade class with a pricer object.
    fn register(
        &mut self,
        py: Python<'_>,
        trade_type: &Bound<'_, PyType>,
        pricer: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.registry.register(py, trade_type, pricer)
    }

    /// Removes the registration for an exact Python trade class.
    fn unregister(&mut self, py: Python<'_>, trade_type: &Bound<'_, PyType>) -> bool {
        self.registry.unregister(py, trade_type)
    }

    /// Returns the pricer selected for a trade, or `None`.
    fn pricer_for(&self, py: Python<'_>, trade: &Bound<'_, PyAny>) -> PyResult<Option<Py<PyAny>>> {
        self.registry.pricer_for(py, trade)
    }

    /// Evaluates a registered trade against a market-data provider.
    #[pyo3(signature = (trade, context, requests = None))]
    fn evaluate(
        &self,
        py: Python<'_>,
        trade: &Bound<'_, PyAny>,
        context: &Bound<'_, PyAny>,
        requests: Option<Vec<Bound<'_, PyAny>>>,
    ) -> PyResult<Py<EvaluationResults>> {
        if let Ok(context_ref) = context.extract::<PyRef<'_, PricingContext>>() {
            context_ref.require_initialized()?;
        }
        let requests = requests.map_or_else(
            || Ok(vec![QsRequest::Value]),
            |objects| extract_requests(&objects),
        )?;
        let pricer = self.registry.resolve(py, trade)?.ok_or_else(|| {
            QuantSupportError::new_err(format!(
                "no pricer registered for trade type '{}'",
                python_type_name(trade)
            ))
        })?;
        call_pricer(py, &pricer, trade, &requests, context)
    }

    fn __len__(&self) -> usize {
        self.registry.len()
    }

    fn __repr__(&self) -> String {
        format!("Evaluator(registrations={})", self.registry.len())
    }
}

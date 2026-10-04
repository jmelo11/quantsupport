//! The Python `PricingContext` — a context manager wrapping the Rust
//! [`quantsupport::prelude::PricingContext`].
//!
//! All market data and configurations are passed as typed objects
//! ([`crate::market`]); the base-curve definition lives in a dedicated
//! [`DiscountingConfig`]. Entering the `with` block bootstraps curves /
//! volatility surfaces / cubes / simulations and starts the AD tape;
//! leaving it stops the tape and releases all constructed market data.

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::PyType;
use quantsupport::prelude::{
    ADForward, BackwardFourierEvolutionBondPricer, BasisSwap as QsBasisSwap, BasisSwapTrade,
    BlackEuropeanOptionPricer, BondPricer, BondPricingAnchor, CdsPricer, ClosedFormBlackCapPricer,
    ClosedFormBlackCapletPricer, ConstructedElementStore, Currency, DiscountedCashflowPricer,
    DualFwd, FixFloatCrossCurrencySwap as QsFixFloatXccy, FixFloatCrossCurrencySwapTrade,
    FixedRateDeposit as QsFixedRateDeposit, FixedRateDepositTrade, FloatFloatCrossCurrencySwap,
    FloatFloatCrossCurrencySwapTrade, FloatingRateNote as QsFloatingRateNote,
    FloatingRateNoteTrade, FxEuropeanOptionPricer, FxForwardPricer, InstrumentType, MarketIndex,
    DayCounter, FixedIncomeDiscountPolicy, Level, LgmMarketModel, LgmRateModel, ModelConfiguration,
    Pricer, PricerImplementation, PricingContext as QsPricingContext, RateFuturesPricer,
    Scalar, ScriptEngine,
    Swap as QsSwap, SwapTrade, Tape, TreeBasedBondPricer, XvaEngine,
};

use crate::conv::{extract_currency, extract_market_index, extract_requests, qs_err};
use crate::explore::{DiscountCurve, Simulation, VolatilityCube, VolatilitySurface};
use crate::extension::{call_pricer, PricerRegistry};
use crate::market::{
    CurveConfiguration, FixingStore, FxStore, PricerConfiguration, QuoteStore, Scenario,
    SimulationConfiguration, VolatilityCubeConfiguration, VolatilitySurfaceConfiguration,
};
use crate::results::EvaluationResults;
use crate::scripting::ScriptedProduct;
use crate::time::Date;
use crate::trades::{
    BasisSwap, CapFloorPy, CapletFloorletPy, CreditDefaultSwapPy, CrossCurrencySwap, EquityOption,
    FixFloatCrossCurrencySwap, FixedRateBond, FixedRateDeposit, FloatingRateNote, FxForwardPy,
    FxOptionPy, RateFuturesPy, SteppedCouponBond, Swap,
};
use crate::xva::{NettingSet, XvaConfig, XvaResult};
use crate::QuantSupportError;

/// Base-curve (risk-free discounting) configuration of the context:
/// the reporting currency and the remuneration index whose curve anchors
/// collateral-adjusted discounting.
#[pyclass(name = "DiscountingConfig", from_py_object)]
#[derive(Clone)]
pub struct DiscountingConfig {
    pub currency: Currency,
    pub index: MarketIndex,
}

/// Market observable used to anchor native straight or callable bond pricing.
///
/// Use one of the named constructors; clean and dirty prices are quoted per
/// the bond's units. Price anchors imply Z-spread for straight bonds and OAS
/// for bonds with an issuer-call schedule.
#[pyclass(name = "BondPricingAnchor", frozen, from_py_object)]
#[derive(Clone, Copy)]
pub struct BondPricingAnchorConfig {
    inner: BondPricingAnchor,
}

#[pymethods]
impl BondPricingAnchorConfig {
    /// Anchors the pricer to a clean price.
    #[staticmethod]
    fn clean_price(value: f64) -> Self {
        Self {
            inner: BondPricingAnchor::CleanPrice(value),
        }
    }

    /// Anchors the pricer to a dirty price.
    #[staticmethod]
    fn dirty_price(value: f64) -> Self {
        Self {
            inner: BondPricingAnchor::DirtyPrice(value),
        }
    }

    /// Anchors a straight bond to a continuously compounded Z-spread.
    #[staticmethod]
    fn z_spread(value: f64) -> Self {
        Self {
            inner: BondPricingAnchor::ZSpread(value),
        }
    }

    /// Anchors an issuer-call bond to an OAS.
    #[staticmethod]
    #[pyo3(name = "OAS")]
    fn oas(value: f64) -> Self {
        Self {
            inner: BondPricingAnchor::OAS(value),
        }
    }
}

#[pymethods]
impl DiscountingConfig {
    #[new]
    fn new(currency: &Bound<'_, PyAny>, index: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            currency: extract_currency(currency)?,
            index: extract_market_index(index)?,
        })
    }

    /// Builds the configuration from a dict `{"currency": ..., "index": ...}`.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        #[derive(serde::Deserialize)]
        struct Input {
            currency: Currency,
            index: MarketIndex,
        }
        let input: Input = crate::conv::from_py(obj, "discounting config")?;
        Ok(Self {
            currency: input.currency,
            index: input.index,
        })
    }

    /// Base currency.
    #[getter]
    fn currency(&self) -> crate::enums::Currency {
        self.currency.into()
    }

    /// Base remuneration index.
    #[getter]
    fn index(&self) -> crate::enums::MarketIndex {
        crate::enums::MarketIndex {
            inner: self.index.clone(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "DiscountingConfig(currency={}, index={:?})",
            self.currency, self.index
        )
    }
}

/// Market data context for pricing. Use as a context manager:
///
/// ```python
/// quotes = qs.QuoteStore.from_json("quotes.json")
/// curves = qs.CurveConfiguration.from_json("curve_specs.json")
/// discounting = qs.DiscountingConfig(currency="USD", index="SOFR")
///
/// with qs.PricingContext(quotes=quotes, curves=curves, discounting=discounting) as ctx:
///     results = ctx.evaluate(swap, requests=["Value", "Sensitivities"])
/// ```
#[pyclass(name = "PricingContext", unsendable, subclass, dict)]
pub struct PricingContext {
    inner: QsPricingContext,
    initialized: bool,
    python_pricers: PricerRegistry,
}

impl PricingContext {
    pub(crate) fn require_initialized(&self) -> PyResult<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(QuantSupportError::new_err(
                "PricingContext is not initialized; use it inside a `with` block \
                 or call initialize() first",
            ))
        }
    }
}

#[pymethods]
impl PricingContext {
    #[new]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    #[pyo3(signature = (
        quotes,
        curves,
        fixings = None,
        fx = None,
        volatility_surfaces = None,
        volatility_cubes = None,
        pricer_configurations = None,
        simulations = None,
        discounting = None,
        scenarios = None,
    ))]
    fn new(
        quotes: QuoteStore,
        curves: Vec<CurveConfiguration>,
        fixings: Option<FixingStore>,
        fx: Option<FxStore>,
        volatility_surfaces: Option<Vec<VolatilitySurfaceConfiguration>>,
        volatility_cubes: Option<Vec<VolatilityCubeConfiguration>>,
        pricer_configurations: Option<Vec<PricerConfiguration>>,
        simulations: Option<Vec<SimulationConfiguration>>,
        discounting: Option<DiscountingConfig>,
        scenarios: Option<Vec<Scenario>>,
    ) -> PyResult<Self> {
        let mut ctx = QsPricingContext::new()
            .with_quote_store(quotes.inner)
            .with_curve_configurations(curves.into_iter().map(|c| c.inner).collect());

        if let Some(f) = fixings {
            ctx = ctx.with_fixing_store(f.inner);
        }
        if let Some(f) = fx {
            ctx = ctx.with_fx_store(f.inner);
        }
        if let Some(v) = volatility_surfaces {
            ctx = ctx
                .with_volatility_surface_configurations(v.into_iter().map(|c| c.inner).collect());
        }
        if let Some(v) = volatility_cubes {
            ctx = ctx.with_volatility_cube_configurations(v.into_iter().map(|c| c.inner).collect());
        }
        if let Some(configurations) = pricer_configurations {
            ctx = ctx.with_pricer_configurations(
                configurations
                    .into_iter()
                    .map(|configuration| configuration.inner)
                    .collect(),
            );
        }
        if let Some(s) = simulations {
            ctx = ctx.with_simulation_configurations(s.into_iter().map(|c| c.inner).collect());
        }
        if let Some(d) = discounting {
            ctx = ctx.with_base_currency(d.currency).with_base_index(d.index);
        }
        if let Some(s) = scenarios {
            ctx = ctx.with_scenarios(s.into_iter().map(|s| s.inner).collect());
        }

        Ok(Self {
            inner: ctx,
            initialized: false,
            python_pricers: PricerRegistry::default(),
        })
    }

    /// Bootstraps curves and builds volatility surfaces/cubes and simulations.
    ///
    /// Called automatically by `__enter__`; only needed when not using a
    /// `with` block.
    fn initialize(&mut self) -> PyResult<()> {
        if !self.initialized {
            self.inner.initialize().map_err(qs_err)?;
            self.initialized = true;
        }
        Ok(())
    }

    /// Reference date of the underlying quote store.
    #[getter]
    fn reference_date(&self) -> Date {
        Date {
            inner: self.inner.quote_store().reference_date(),
        }
    }

    /// Alias for `reference_date`, matching the `MarketDataProvider` contract.
    #[getter]
    fn evaluation_date(&self) -> Date {
        Date {
            inner: self.inner.evaluation_date(),
        }
    }

    /// The quote store the context was built from.
    #[getter]
    fn quotes(&self) -> QuoteStore {
        QuoteStore {
            inner: self.inner.quote_store().clone(),
        }
    }

    /// Historical fixings available to built-in and Python pricers.
    #[getter]
    fn fixings(&self) -> FixingStore {
        FixingStore {
            inner: self.inner.fixing_store().clone(),
        }
    }

    /// FX spot store available to built-in and Python pricers.
    #[getter]
    fn fx(&self) -> FxStore {
        FxStore {
            inner: self.inner.fx_store().clone(),
        }
    }

    /// Reporting/base currency used by the context.
    #[getter]
    fn base_currency(&self) -> crate::enums::Currency {
        self.inner.base_currency().into()
    }

    /// Base collateral/remuneration index used by the context.
    #[getter]
    fn base_index(&self) -> crate::enums::MarketIndex {
        crate::enums::MarketIndex {
            inner: self.inner.base_index().clone(),
        }
    }

    /// The original curve configurations (bootstrap inputs).
    #[getter]
    fn curve_configurations(&self) -> Vec<CurveConfiguration> {
        self.inner
            .curve_configurations()
            .iter()
            .map(|c| CurveConfiguration { inner: c.clone() })
            .collect()
    }

    /// The original volatility surface configurations.
    #[getter]
    fn volatility_surface_configurations(&self) -> Vec<VolatilitySurfaceConfiguration> {
        self.inner
            .volatility_surface_configurations()
            .iter()
            .map(|c| VolatilitySurfaceConfiguration { inner: c.clone() })
            .collect()
    }

    /// The original volatility cube configurations.
    #[getter]
    fn volatility_cube_configurations(&self) -> Vec<VolatilityCubeConfiguration> {
        self.inner
            .volatility_cube_configurations()
            .iter()
            .map(|c| VolatilityCubeConfiguration { inner: c.clone() })
            .collect()
    }

    /// The original simulation configurations.
    #[getter]
    fn simulation_configurations(&self) -> Vec<SimulationConfiguration> {
        self.inner
            .simulation_configurations()
            .iter()
            .map(|c| SimulationConfiguration { inner: c.clone() })
            .collect()
    }

    /// Instrument-level pricer selections and typed model assignments.
    #[getter]
    fn pricer_configurations(&self) -> Vec<PricerConfiguration> {
        self.inner
            .pricer_configurations()
            .iter()
            .map(|configuration| PricerConfiguration {
                inner: configuration.clone(),
            })
            .collect()
    }

    /// All bootstrapped discount curves, sorted by market index.
    fn curves(&self) -> PyResult<Vec<DiscountCurve>> {
        self.require_initialized()?;
        let mut curves: Vec<_> = self
            .inner
            .constructed_elements()
            .discount_curves()
            .values()
            .map(|e| DiscountCurve { element: e.clone() })
            .collect();
        curves.sort_by_key(|c| c.element.market_index().to_string());
        Ok(curves)
    }

    /// A bootstrapped discount curve by market index.
    fn curve(&self, index: &Bound<'_, PyAny>) -> PyResult<DiscountCurve> {
        self.require_initialized()?;
        let idx = extract_market_index(index)?;
        self.inner
            .constructed_elements()
            .discount_curve(&idx)
            .map(|e| DiscountCurve { element: e.clone() })
            .ok_or_else(|| {
                QuantSupportError::new_err(format!("no bootstrapped curve for index '{idx}'"))
            })
    }

    /// All constructed volatility surfaces, sorted by market index.
    fn volatility_surfaces(&self) -> PyResult<Vec<VolatilitySurface>> {
        self.require_initialized()?;
        let mut surfaces: Vec<_> = self
            .inner
            .constructed_elements()
            .volatility_surfaces()
            .values()
            .map(|e| VolatilitySurface { element: e.clone() })
            .collect();
        surfaces.sort_by_key(|s| s.element.market_index().to_string());
        Ok(surfaces)
    }

    /// A constructed volatility surface by market index.
    fn volatility_surface(&self, index: &Bound<'_, PyAny>) -> PyResult<VolatilitySurface> {
        self.require_initialized()?;
        let idx = extract_market_index(index)?;
        self.inner
            .constructed_elements()
            .volatility_surface(&idx)
            .map(|e| VolatilitySurface { element: e.clone() })
            .ok_or_else(|| {
                QuantSupportError::new_err(format!("no volatility surface for index '{idx}'"))
            })
    }

    /// All constructed volatility cubes, sorted by market index.
    fn volatility_cubes(&self) -> PyResult<Vec<VolatilityCube>> {
        self.require_initialized()?;
        let mut cubes: Vec<_> = self
            .inner
            .constructed_elements()
            .volatility_cubes()
            .values()
            .map(|e| VolatilityCube { element: e.clone() })
            .collect();
        cubes.sort_by_key(|c| c.element.market_index().to_string());
        Ok(cubes)
    }

    /// A constructed volatility cube by market index.
    fn volatility_cube(&self, index: &Bound<'_, PyAny>) -> PyResult<VolatilityCube> {
        self.require_initialized()?;
        let idx = extract_market_index(index)?;
        self.inner
            .constructed_elements()
            .volatility_cube(&idx)
            .map(|e| VolatilityCube { element: e.clone() })
            .ok_or_else(|| {
                QuantSupportError::new_err(format!("no volatility cube for index '{idx}'"))
            })
    }

    /// All constructed Monte Carlo simulations, sorted by market index.
    fn simulations(&self) -> PyResult<Vec<Simulation>> {
        self.require_initialized()?;
        let mut sims: Vec<_> = self
            .inner
            .constructed_elements()
            .simulations()
            .values()
            .map(|e| Simulation { element: e.clone() })
            .collect();
        sims.sort_by_key(|s| s.element.market_index().to_string());
        Ok(sims)
    }

    /// A constructed Monte Carlo simulation by market index.
    fn simulation(&self, index: &Bound<'_, PyAny>) -> PyResult<Simulation> {
        self.require_initialized()?;
        let idx = extract_market_index(index)?;
        self.inner
            .constructed_elements()
            .simulations()
            .get(&idx)
            .map(|e| Simulation { element: e.clone() })
            .ok_or_else(|| QuantSupportError::new_err(format!("no simulation for index '{idx}'")))
    }

    /// Registers (or replaces) a Python pricer for a Python trade class.
    ///
    /// The most specific exact type is selected first; registered base classes
    /// also handle their subclasses. Python registrations take precedence over
    /// built-in dispatch, which makes built-in trade classes overridable too.
    fn register_pricer(
        &mut self,
        py: Python<'_>,
        trade_type: &Bound<'_, PyType>,
        pricer: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.python_pricers.register(py, trade_type, pricer)
    }

    /// Removes the Python pricer registered for an exact trade class.
    fn unregister_pricer(&mut self, py: Python<'_>, trade_type: &Bound<'_, PyType>) -> bool {
        self.python_pricers.unregister(py, trade_type)
    }

    /// Returns the Python pricer selected for a trade, or `None` when built-in
    /// dispatch will be used.
    fn pricer_for(&self, py: Python<'_>, trade: &Bound<'_, PyAny>) -> PyResult<Option<Py<PyAny>>> {
        self.python_pricers.pricer_for(py, trade)
    }

    fn __enter__(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        slf.initialize()?;
        Tape::<ADForward>::start_recording_fwd();
        Ok(slf)
    }

    #[pyo3(signature = (_exc_type = None, _exc_value = None, _traceback = None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        Tape::<ADForward>::stop_recording_fwd();
        Tape::<ADForward>::rewind_to_init_fwd();
        *self.inner.constructed_elements_mut() = ConstructedElementStore::default();
        self.initialized = false;
        false // never suppress exceptions
    }

    /// Evaluates dated payoff code with native LGM models and bootstrapped context curves.
    #[pyo3(signature = (script, n_paths = 1000, seed = 42))]
    fn evaluate_script(
        &self,
        script: &Bound<'_, PyAny>,
        n_paths: usize,
        seed: u64,
    ) -> PyResult<HashMap<String, f64>> {
        self.require_initialized()?;
        if n_paths == 0 {
            return Err(pyo3::exceptions::PyValueError::new_err("n_paths must be positive"));
        }
        let script = script.extract::<ScriptedProduct>()
            .or_else(|_| ScriptedProduct::from_dict(script))?;
        let discount_index = match &script.discount_index {
            Some(index) => index.clone(),
            None if script.currency == self.inner.base_currency() => self.inner.base_index().clone(),
            None => return Err(QuantSupportError::new_err(
                "scripts outside the context base currency require an explicit discount_index",
            )),
        };
        let mut assignments = HashMap::new();
        for configuration in self.inner.pricer_configurations() {
            for index in self.inner.constructed_elements().discount_curves().keys() {
                if let Some(model) = configuration.model(index) {
                    if matches!(model, ModelConfiguration::Lgm { .. } | ModelConfiguration::HullWhite { .. }) {
                        assignments.insert(index.clone(), model.clone());
                    }
                }
            }
        }
        for configuration in self.inner.simulation_configurations() {
            if matches!(configuration.model(), ModelConfiguration::Lgm { .. } | ModelConfiguration::HullWhite { .. }) {
                assignments.insert(configuration.market_index().clone(), configuration.model().clone());
            }
        }
        for configuration in &script.models {
            assignments.insert(configuration.market_index().clone(), configuration.model().clone());
        }
        if !assignments.contains_key(&discount_index) {
            return Err(QuantSupportError::new_err(format!(
                "no rate model configured for script discount index '{discount_index}'; supply script.models or context model configurations",
            )));
        }
        let mut assignments: Vec<_> = assignments.into_iter().collect();
        assignments.sort_by_key(|(index, _)| index.to_string());
        let elements = assignments.iter().map(|(index, _)| {
            self.inner.constructed_elements().discount_curve(index).ok_or_else(|| {
                QuantSupportError::new_err(format!("no bootstrapped curve for script model '{index}'"))
            })
        }).collect::<PyResult<Vec<_>>>()?;
        let curves: Vec<_> = elements.iter().map(|element| element.curve()).collect();
        let reference_date = self.inner.evaluation_date();
        let day_counter = self.inner.constructed_elements().discount_curve(&discount_index)
            .ok_or_else(|| QuantSupportError::new_err(format!("no script discount curve '{discount_index}'")))?
            .curve().day_counter().unwrap_or(DayCounter::Actual365);
        let mut market_model = LgmMarketModel::new(
            script.currency, discount_index.clone(), reference_date, day_counter,
        ).with_n_paths(n_paths).with_seed(seed);
        for (((index, configuration), element), curve) in assignments.iter().zip(&elements).zip(&curves) {
            let configuration = match configuration {
                ModelConfiguration::HullWhite { alpha, parameter_source } => ModelConfiguration::Lgm {
                    lambda: *alpha, parameter_source: parameter_source.clone(),
                },
                other => other.clone(),
            };
            let ModelConfiguration::Lgm { lambda, .. } = &configuration else {
                return Err(QuantSupportError::new_err(format!(
                    "script rate model '{index}' must be Lgm or HullWhite",
                )));
            };
            if !lambda.is_finite() || *lambda < 0.0 {
                return Err(QuantSupportError::new_err("rate model mean reversion must be finite and non-negative"));
            }
            let plain_curve = element.to_f64_term_structure(day_counter).map_err(qs_err)?;
            let resolved = LgmRateModel::from_configuration(
                &configuration, &plain_curve, self.inner.constructed_elements(),
                self.inner.quote_store(), Level::Mid,
            ).map_err(qs_err)?;
            let schedule = resolved.sigma_schedule().iter().map(|(time, sigma)| {
                (*time, DualFwd::scalar(*sigma))
            }).collect();
            let rate_model = LgmRateModel::new_piecewise(
                DualFwd::scalar(*lambda), schedule, &**curve,
            ).map_err(qs_err)?;
            market_model.add_curve_model(index.clone(), rate_model);
        }
        ScriptEngine::new(
            script.event_stream()?, reference_date, script.currency, discount_index,
        ).map_err(qs_err)?.evaluate(&mut market_model, script.result_variable.as_deref()).map_err(qs_err)
    }

    /// Evaluates a trade.
    ///
    /// `requests` is a list of [`crate::enums::Request`] values (or their
    /// string names). Defaults to `[Request.Value]`.
    #[pyo3(signature = (trade, requests = None, bond_anchor = None))]
    fn evaluate<'py>(
        slf: PyRef<'py, Self>,
        py: Python<'py>,
        trade: &Bound<'py, PyAny>,
        requests: Option<Vec<Bound<'py, PyAny>>>,
        bond_anchor: Option<BondPricingAnchorConfig>,
    ) -> PyResult<Py<EvaluationResults>> {
        slf.require_initialized()?;
        let reqs = match requests {
            Some(objs) => extract_requests(&objs)?,
            None => vec![quantsupport::prelude::Request::Value],
        };

        if let Some(pricer) = slf.python_pricers.resolve(py, trade)? {
            let context = slf.into_pyobject(py)?;
            return call_pricer(py, &pricer, trade, &reqs, context.as_any());
        }

        if let Ok(swap) = trade.extract::<Swap>() {
            let t = swap.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<QsSwap<DualFwd>, SwapTrade<DualFwd>>::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(xccy) = trade.extract::<CrossCurrencySwap>() {
            let t = xccy.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<
                FloatFloatCrossCurrencySwap<DualFwd>,
                FloatFloatCrossCurrencySwapTrade<DualFwd>,
            >::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(basis) = trade.extract::<BasisSwap>() {
            let t = basis.build_trade_dual()?;
            let pricer =
                DiscountedCashflowPricer::<QsBasisSwap<DualFwd>, BasisSwapTrade<DualFwd>>::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(xccy) = trade.extract::<FixFloatCrossCurrencySwap>() {
            let t = xccy.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<
                QsFixFloatXccy<DualFwd>,
                FixFloatCrossCurrencySwapTrade<DualFwd>,
            >::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(bond) = trade.extract::<FixedRateBond>() {
            let t = bond.build_trade_dual()?;
            let anchor = bond_anchor.map_or(BondPricingAnchor::ZSpread(0.0), |configuration| {
                configuration.inner
            });
            let mut pricer = BondPricer::new(anchor);
            pricer.set_discount_policy(Box::new(
                FixedIncomeDiscountPolicy::new(true).with_risk_free_index(
                    slf.inner.base_currency(), slf.inner.base_index().clone(),
                ),
            ));
            let res = pricer
                .evaluate(&t, &reqs, &slf.inner)
                .map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(bond) = trade.extract::<SteppedCouponBond>() {
            let t = bond.build_trade_dual()?;
            let default_anchor = if bond.call_schedule.is_empty() {
                BondPricingAnchor::ZSpread(0.0)
            } else {
                BondPricingAnchor::OAS(0.0)
            };
            let anchor = bond_anchor.map_or(default_anchor, |configuration| {
                configuration.inner
            });
            let res = match slf
                .inner
                .pricer_configuration(InstrumentType::SteppedCouponBond)
                .map(|configuration| configuration.pricer())
            {
                Some(PricerImplementation::BackwardFourierEvolution { .. }) => {
                    let mut pricer = BackwardFourierEvolutionBondPricer::new(anchor);
                    pricer.set_discount_policy(Box::new(
                        FixedIncomeDiscountPolicy::new(false).with_risk_free_index(
                            slf.inner.base_currency(), slf.inner.base_index().clone(),
                        ),
                    ));
                    pricer.evaluate(&t, &reqs, &slf.inner)
                }
                Some(PricerImplementation::TreeBased { .. }) => {
                    let mut pricer = TreeBasedBondPricer::new(anchor);
                    pricer.set_discount_policy(Box::new(
                        FixedIncomeDiscountPolicy::new(false).with_risk_free_index(
                            slf.inner.base_currency(), slf.inner.base_index().clone(),
                        ),
                    ));
                    pricer.evaluate(&t, &reqs, &slf.inner)
                }
                Some(other) => Err(quantsupport::prelude::QSError::InvalidValueErr(format!(
                    "Configured pricer {other:?} cannot price a stepped-coupon bond"
                ))),
                None => Err(quantsupport::prelude::QSError::NotFoundErr(
                    "SteppedCouponBond requires a pricer configuration".into(),
                )),
            }
            .map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(frn) = trade.extract::<FloatingRateNote>() {
            let t = frn.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<
                QsFloatingRateNote<DualFwd>,
                FloatingRateNoteTrade<DualFwd>,
            >::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(deposit) = trade.extract::<FixedRateDeposit>() {
            let t = deposit.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<
                QsFixedRateDeposit<DualFwd>,
                FixedRateDepositTrade<DualFwd>,
            >::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(fwd) = trade.extract::<FxForwardPy>() {
            let t = fwd.build_trade()?;
            let pricer = FxForwardPricer::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(opt) = trade.extract::<FxOptionPy>() {
            let t = opt.build_trade()?;
            let pricer = FxEuropeanOptionPricer::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(opt) = trade.extract::<EquityOption>() {
            let t = opt.build_trade()?;
            let pricer = BlackEuropeanOptionPricer::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(cds) = trade.extract::<CreditDefaultSwapPy>() {
            let t = cds.build_trade()?;
            let pricer = CdsPricer::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(cap) = trade.extract::<CapFloorPy>() {
            let t = cap.build_trade()?;
            let pricer = ClosedFormBlackCapPricer::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(caplet) = trade.extract::<CapletFloorletPy>() {
            let t = caplet.build_trade()?;
            let pricer = ClosedFormBlackCapletPricer::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(futures) = trade.extract::<RateFuturesPy>() {
            let t = futures.build_trade()?;
            let pricer = RateFuturesPricer::new();
            let res = pricer.evaluate(&t, &reqs, &slf.inner).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }

        Err(QuantSupportError::new_err(format!(
            "no pricer registered for Python trade type '{}' and it is not a supported built-in trade; \
             call context.register_pricer(TradeType, pricer)",
            trade.get_type().name()?
        )))
    }

    /// Runs the XVA engine.
    ///
    /// - `config`: an [`XvaConfig`] (models, paths, seed, frequency).
    /// - `netting_sets`: list of [`NettingSet`], each carrying its client's
    ///   [`crate::xva::CsaTerms`] (collateral treatment + credit/funding).
    #[allow(clippy::needless_pass_by_value)]
    fn run_xva(&self, config: XvaConfig, netting_sets: Vec<NettingSet>) -> PyResult<XvaResult> {
        self.require_initialized()?;

        let mut sets = HashMap::new();
        for ns in &netting_sets {
            if sets.contains_key(ns.name()) {
                return Err(QuantSupportError::new_err(format!(
                    "duplicate netting set name '{}'",
                    ns.name()
                )));
            }
            sets.insert(ns.name().to_string(), ns.build()?);
        }

        let mut engine = XvaEngine::new(&self.inner, config.inner).map_err(qs_err)?;
        let result = engine.run(&mut sets).map_err(qs_err)?;
        Ok(XvaResult::from_qs(&result))
    }
}

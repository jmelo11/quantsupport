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
use quantsupport::prelude::{
    ADForward, BasisSwap as QsBasisSwap, BasisSwapTrade, BlackEuropeanOptionPricer, CdsPricer,
    ClosedFormBlackCapPricer, ClosedFormBlackCapletPricer, ConstructedElementStore, Currency,
    DiscountedCashflowPricer, DualFwd, FixFloatCrossCurrencySwap as QsFixFloatXccy,
    FixFloatCrossCurrencySwapTrade, FixedRateBond as QsFixedRateBond, FixedRateBondTrade,
    FixedRateDeposit as QsFixedRateDeposit, FixedRateDepositTrade, FloatFloatCrossCurrencySwap,
    FloatFloatCrossCurrencySwapTrade, FloatingRateNote as QsFloatingRateNote,
    FloatingRateNoteTrade, FxEuropeanOptionPricer, FxForwardPricer, MarketIndex, Pricer,
    PricingContext as QsPricingContext, RateFuturesPricer, Swap as QsSwap, SwapTrade, Tape,
    XvaEngine,
};

use crate::conv::{extract_currency, extract_market_index, extract_requests, qs_err};
use crate::explore::{DiscountCurve, Simulation, VolatilityCube, VolatilitySurface};
use crate::market::{
    CurveConfiguration, FixingStore, FxStore, QuoteStore, Scenario, SimulationConfiguration,
    VolatilityCubeConfiguration, VolatilitySurfaceConfiguration,
};
use crate::results::EvaluationResults;
use crate::time::Date;
use crate::trades::{
    BasisSwap, CapFloorPy, CapletFloorletPy, CreditDefaultSwapPy, CrossCurrencySwap, EquityOption,
    FixFloatCrossCurrencySwap, FixedRateBond, FixedRateDeposit, FloatingRateNote, FxForwardPy,
    FxOptionPy, RateFuturesPy, Swap,
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
#[pyclass(name = "PricingContext", unsendable)]
pub struct PricingContext {
    inner: QsPricingContext,
    initialized: bool,
}

impl PricingContext {
    fn require_initialized(&self) -> PyResult<()> {
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

    /// The quote store the context was built from.
    #[getter]
    fn quotes(&self) -> QuoteStore {
        QuoteStore {
            inner: self.inner.quote_store().clone(),
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

    /// Evaluates a trade.
    ///
    /// `requests` is a list of [`crate::enums::Request`] values (or their
    /// string names). Defaults to `[Request.Value]`.
    #[pyo3(signature = (trade, requests = None))]
    fn evaluate(
        &self,
        trade: &Bound<'_, PyAny>,
        requests: Option<Vec<Bound<'_, PyAny>>>,
    ) -> PyResult<EvaluationResults> {
        self.require_initialized()?;
        let reqs = match requests {
            Some(objs) => extract_requests(&objs)?,
            None => vec![quantsupport::prelude::Request::Value],
        };

        if let Ok(swap) = trade.extract::<Swap>() {
            let t = swap.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<QsSwap<DualFwd>, SwapTrade<DualFwd>>::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(xccy) = trade.extract::<CrossCurrencySwap>() {
            let t = xccy.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<
                FloatFloatCrossCurrencySwap<DualFwd>,
                FloatFloatCrossCurrencySwapTrade<DualFwd>,
            >::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(basis) = trade.extract::<BasisSwap>() {
            let t = basis.build_trade_dual()?;
            let pricer =
                DiscountedCashflowPricer::<QsBasisSwap<DualFwd>, BasisSwapTrade<DualFwd>>::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(xccy) = trade.extract::<FixFloatCrossCurrencySwap>() {
            let t = xccy.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<
                QsFixFloatXccy<DualFwd>,
                FixFloatCrossCurrencySwapTrade<DualFwd>,
            >::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(bond) = trade.extract::<FixedRateBond>() {
            let t = bond.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<
                QsFixedRateBond<DualFwd>,
                FixedRateBondTrade<DualFwd>,
            >::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(frn) = trade.extract::<FloatingRateNote>() {
            let t = frn.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<
                QsFloatingRateNote<DualFwd>,
                FloatingRateNoteTrade<DualFwd>,
            >::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(deposit) = trade.extract::<FixedRateDeposit>() {
            let t = deposit.build_trade_dual()?;
            let pricer = DiscountedCashflowPricer::<
                QsFixedRateDeposit<DualFwd>,
                FixedRateDepositTrade<DualFwd>,
            >::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(fwd) = trade.extract::<FxForwardPy>() {
            let t = fwd.build_trade()?;
            let pricer = FxForwardPricer::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(opt) = trade.extract::<FxOptionPy>() {
            let t = opt.build_trade()?;
            let pricer = FxEuropeanOptionPricer::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(opt) = trade.extract::<EquityOption>() {
            let t = opt.build_trade()?;
            let pricer = BlackEuropeanOptionPricer::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(cds) = trade.extract::<CreditDefaultSwapPy>() {
            let t = cds.build_trade()?;
            let pricer = CdsPricer::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(cap) = trade.extract::<CapFloorPy>() {
            let t = cap.build_trade()?;
            let pricer = ClosedFormBlackCapPricer::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(caplet) = trade.extract::<CapletFloorletPy>() {
            let t = caplet.build_trade()?;
            let pricer = ClosedFormBlackCapletPricer::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }
        if let Ok(futures) = trade.extract::<RateFuturesPy>() {
            let t = futures.build_trade()?;
            let pricer = RateFuturesPricer::new();
            let res = pricer.evaluate(&t, &reqs, &self.inner).map_err(qs_err)?;
            return Ok(EvaluationResults::from_qs(&res));
        }

        Err(QuantSupportError::new_err(
            "unsupported trade type (expected Swap, CrossCurrencySwap, BasisSwap, \
             FixFloatCrossCurrencySwap, FixedRateBond, FloatingRateNote, FixedRateDeposit, \
             FxForward, FxOption, EquityOption, CreditDefaultSwap, CapFloor, CapletFloorlet \
             or RateFutures)",
        ))
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

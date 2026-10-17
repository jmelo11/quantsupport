//! The Python `PricingContext` — a context manager wrapping the Rust
//! [`quantsupport::prelude::PricingContext`].
//!
//! All market data and configurations are passed as typed objects
//! ([`crate::market::stores`]); the base-curve definition lives in a dedicated
//! [`SingleCurveCSADiscountPolicy`]. Entering the `with` block enables request-driven market construction;
//! leaving it stops the tape and releases all constructed market data.

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::PyType;
use quantsupport::core::marketdatahandling::marketview::MarketView;
use quantsupport::prelude::{
    ADForward, BackwardFourierEvolutionBondPricer, BasisSwap as QsBasisSwap, BasisSwapTrade,
    BlackEuropeanOptionPricer, BondPricer, CalibrationSource, CdsPricer, ClosedFormBlackCapPricer,
    ClosedFormBlackCapletPricer, ConstructedElementRequest, ConstructedElementStore, DayCounter,
    DiscountedCashflowPricer, DualFwd, FixFloatCrossCurrencySwap as QsFixFloatXccy,
    FixFloatCrossCurrencySwapTrade, FixedIncomeDiscountPolicy,
    FixedRateDeposit as QsFixedRateDeposit, FixedRateDepositTrade, FloatFloatCrossCurrencySwap,
    FloatFloatCrossCurrencySwapTrade, FloatingRateNote as QsFloatingRateNote,
    FloatingRateNoteTrade, FxEuropeanOptionPricer, FxForwardPricer, InstrumentType, Level,
    LgmMarketModel, LgmRateModel, LoanPricer, MarketData, MarketDataProvider, MarketDataRequest,
    ModelConfiguration, ParameterSource, Pricer, PricerImplementation,
    PricingContext as QsPricingContext, QuoteSelector, QuoteStore as QsQuoteStore,
    RateFuturesPricer, Scalar, ScriptEngine, Swap as QsSwap, SwapTrade, Tape, TreeBasedBondPricer,
    XvaEngine,
};

use crate::pricing::collateral::{
    FixedIncomeDiscountPolicy as PyFixedIncomeDiscountPolicy, SingleCurveCSADiscountPolicy,
};
use crate::common::conversion::{extract_level, extract_market_index, extract_requests, qs_err};
use crate::market::views::{DiscountCurve, Simulation, VolatilityCube, VolatilitySurface};
use crate::pricing::extension::{call_pricer, PricerRegistry};
use crate::market::stores::{
    CreditCurveConfiguration, CurveConfiguration, FixingStore, FxStore, PricerConfiguration,
    QuoteStore, SimulationConfiguration, VolatilityCubeConfiguration,
    VolatilitySurfaceConfiguration,
};
use crate::pricing::results::EvaluationResults;
use crate::market::scenario::{enter_context, exit_context};
use crate::instruments::scripting::ScriptedProduct;
use crate::time::Date;
use crate::instruments::products::{
    BasisSwap, CapFloorPy, CapletFloorletPy, CreditDefaultSwapPy, CrossCurrencySwap, EquityOption,
    FixFloatCrossCurrencySwap, FixedRateBond, FixedRateDeposit, FloatingRateNote, FxForwardPy,
    FxOptionPy, RateFuturesPy, SteppedCouponBond, Swap,
};
use crate::xva::{NettingSet, XvaConfig, XvaResult};
use crate::QuantSupportError;

/// Market inputs and policies used to value trades.
///
/// Assign public properties to change inputs: curves, curve_configurations,
/// credit_curve_configurations, fixings, quotes, fx, quote_level, csa_policy,
/// fixed_income_policy, volatility_surface_configurations,
/// volatility_cube_configurations, simulation_configurations, and
/// pricer_configurations. Reassigning market inputs invalidates constructed data;
/// construction follows subsequent market-data requests.
/// Collection getters return copies; assign the updated collection back.
/// Use ``with context, scenario:`` for temporary ``Scenario`` shocks;
/// the base market remains available when the scenario block exits.
///
/// Parameters
/// ----------
/// quotes : QuoteStore
///     Market observations and reference date.
/// curves : sequence of CurveConfiguration
///     Calibration inputs, explicit zero rates, and composite curves.
/// fixings : FixingStore, optional
///     Published historical index observations.
/// fx : FxStore, optional
///     Spot exchange rates.
/// volatility_surfaces : sequence of VolatilitySurfaceConfiguration, optional
///     Surface construction inputs.
/// volatility_cubes : sequence of VolatilityCubeConfiguration, optional
///     Cube construction inputs.
/// pricer_configurations : sequence of PricerConfiguration, optional
///     Instrument pricing models and implementations.
/// simulations : sequence of SimulationConfiguration, optional
///     Simulation inputs.
/// csa_policy : SingleCurveCSADiscountPolicy, optional
///     Collateral currency and curve used for market construction.
/// fixed_income_policy : FixedIncomeDiscountPolicy, optional
///     Currency-to-curve discount policy. Defaults to the context base curve.
/// quote_level : Level or str, optional
///     Quote level used for calibration. Defaults to Mid.
/// credit_curves : sequence of CreditCurveConfiguration, optional
///     Survival curve construction inputs.
///
/// Examples
/// --------
///
/// ```python
/// quotes = qs.QuoteStore.from_json("quotes.json")
/// curves = qs.CurveConfiguration.from_json("curve_specs.json")
/// csa_policy = qs.SingleCurveCSADiscountPolicy(collateral_currency="USD", collateral_index="SOFR")
/// trade = qs.Trade(swap, quotes.reference_date)
///
/// with qs.PricingContext(quotes=quotes, curves=curves, csa_policy=csa_policy) as ctx:
///     results = ctx.evaluate(trade, requests=["Value", "Sensitivities"])
/// ```
#[pyclass(
    module = "quantsupport",
    name = "PricingContext",
    unsendable,
    subclass,
    dict
)]
pub struct PricingContext {
    inner: QsPricingContext,
    initialized: bool,
    fully_constructed: bool,
    generation: u64,
    python_pricers: PricerRegistry,
    fixed_income_policy: Option<PyFixedIncomeDiscountPolicy>,
}

impl PricingContext {
    pub fn from_snapshot_context(
        inner: QsPricingContext,
        fixed_income_policy: Option<PyFixedIncomeDiscountPolicy>,
    ) -> Self {
        Self {
            inner,
            initialized: false,
            fully_constructed: false,
            generation: 0,
            python_pricers: PricerRegistry::default(),
            fixed_income_policy,
        }
    }

    pub fn key(&self) -> usize {
        std::ptr::from_ref(self) as usize
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn base_context(&self) -> &QsPricingContext {
        &self.inner
    }

    pub fn market_view(&self) -> PyResult<MarketView<'_>> {
        self.require_initialized()?;
        crate::market::scenario::market_view(self, self.generation, &self.inner)
    }

    fn request_elements(&self, elements: Vec<ConstructedElementRequest>) -> PyResult<MarketData> {
        self.market_view()?
            .handle_request(
                &MarketDataRequest::default().with_constructed_elements_request(elements),
            )
            .map_err(qs_err)
    }

    /// Rebuilds initialized market data after a public input is reassigned.
    fn update_market_inputs(
        &mut self,
        update: impl FnOnce(QsPricingContext) -> QsPricingContext,
    ) -> PyResult<()> {
        let rebuild = self.initialized;
        self.initialized = false;
        self.fully_constructed = false;
        self.generation += 1;
        self.inner = update(std::mem::take(&mut self.inner))
            .with_constructed_elements(ConstructedElementStore::default());
        if rebuild {
            self.inner.validate().map_err(qs_err)?;
            self.initialized = true;
        }
        Ok(())
    }

    /// Builds the configured fixed-income policy, using the context base curve
    /// when no explicit currency mapping was supplied.
    pub fn build_loan_discount_policy(&self) -> FixedIncomeDiscountPolicy {
        self.bond_discount_policy(false)
    }

    /// Builds an explicit fixed-income policy or a default backed by the base curve.
    ///
    /// `prefer_instrument_index` affects only the default policy; an explicit
    /// context policy supplies its own preference and currency/index mappings.
    fn bond_discount_policy(&self, prefer_instrument_index: bool) -> FixedIncomeDiscountPolicy {
        self.fixed_income_policy.as_ref().map_or_else(
            || {
                FixedIncomeDiscountPolicy::new(prefer_instrument_index).with_risk_free_index(
                    self.inner.base_currency(),
                    self.inner.base_index().clone(),
                )
            },
            PyFixedIncomeDiscountPolicy::build,
        )
    }
    /// Checks that curves and other configured market elements are initialized.
    ///
    /// # Errors
    /// Returns `QuantSupportError` until `initialize()` or context entry succeeds.
    pub fn require_initialized(&self) -> PyResult<()> {
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
        csa_policy = None,
        fixed_income_policy = None,
        quote_level = None,
        credit_curves = None,
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
        csa_policy: Option<SingleCurveCSADiscountPolicy>,
        fixed_income_policy: Option<PyFixedIncomeDiscountPolicy>,
        quote_level: Option<&Bound<'_, PyAny>>,
        credit_curves: Option<Vec<CreditCurveConfiguration>>,
    ) -> PyResult<Self> {
        let mut ctx = QsPricingContext::new()
            .with_quote_level(
                quote_level
                    .map(extract_level)
                    .transpose()?
                    .unwrap_or(Level::Mid),
            )
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
        if let Some(d) = csa_policy {
            ctx = ctx.with_csa_policy(&d.build());
        }
        if let Some(configurations) = credit_curves {
            ctx = ctx.with_credit_curve_configurations(
                configurations
                    .into_iter()
                    .map(|configuration| configuration.inner)
                    .collect(),
            );
        }

        Ok(Self {
            inner: ctx,
            initialized: false,
            fully_constructed: false,
            generation: 0,
            python_pricers: PricerRegistry::default(),
            fixed_income_policy,
        })
    }

    /// Constructs curves, volatility surfaces/cubes, and simulations.
    ///
    /// Explicitly constructs every configured object. Context entry enables
    /// lazy construction of requested market objects instead.
    fn initialize(&mut self) -> PyResult<()> {
        if !self.fully_constructed {
            self.initialized = false;
            self.inner.initialize().map_err(qs_err)?;
            self.fully_constructed = true;
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

    /// Selected observation instant as a DateTime, or None after inputs change.
    #[getter]
    fn snapshot_timestamp(&self) -> Option<crate::time::DateTime> {
        self.inner.snapshot_timestamp().map(|inner| crate::time::DateTime { inner })
    }

    /// Quote level used for construction and calibration; defaults to `Level.Mid`.
    #[getter]
    fn quote_level(&self) -> crate::common::enums::Level {
        self.inner.quote_level().into()
    }

    /// Snapshot of quote inputs in the active base or scenario market view.
    #[getter]
    fn quotes(&self) -> PyResult<QuoteStore> {
        if !self.initialized {
            return Ok(QuoteStore {
                inner: self.inner.quote_store().clone(),
            });
        }
        let view = self.market_view()?;
        let mut quotes = QsQuoteStore::new(self.inner.evaluation_date());
        for identifier in self.inner.quote_store().quotes().keys() {
            if let Some(quote) = view.select(identifier) {
                quotes.add_quote(quote);
            }
        }
        Ok(QuoteStore { inner: quotes })
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

    /// Base collateral currency used for curve construction.
    #[getter]
    fn base_currency(&self) -> crate::common::enums::Currency {
        self.inner.base_currency().into()
    }

    /// Base collateral/remuneration index used by the context.
    #[getter]
    fn base_index(&self) -> crate::common::enums::MarketIndex {
        crate::common::enums::MarketIndex {
            inner: self.inner.base_index().clone(),
        }
    }

    /// Default collateral policy used for curve construction and derivative valuation.
    #[getter]
    fn csa_policy(&self) -> SingleCurveCSADiscountPolicy {
        quantsupport::prelude::SingleCurveCSADiscountPolicy::new(
            self.inner.base_index().clone(),
            self.inner.base_currency(),
        )
        .into()
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

    /// The original credit curve configurations (bootstrap inputs).
    #[getter]
    fn credit_curve_configurations(&self) -> Vec<CreditCurveConfiguration> {
        self.inner
            .credit_curve_configurations()
            .iter()
            .map(|configuration| CreditCurveConfiguration {
                inner: configuration.clone(),
            })
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

    /// Fixed-income discount policy, or None for the context base-curve policy.
    #[getter]
    fn fixed_income_policy(&self) -> Option<PyFixedIncomeDiscountPolicy> {
        self.fixed_income_policy.clone()
    }

    #[setter]
    fn set_fixed_income_policy(&mut self, policy: Option<PyFixedIncomeDiscountPolicy>) {
        self.fixed_income_policy = policy;
    }

    #[setter]
    fn set_quote_level(&mut self, level: &Bound<'_, PyAny>) -> PyResult<()> {
        let level = extract_level(level)?;
        self.update_market_inputs(|context| context.with_quote_level(level))
    }

    #[setter]
    fn set_csa_policy(&mut self, policy: SingleCurveCSADiscountPolicy) -> PyResult<()> {
        self.update_market_inputs(|context| context.with_csa_policy(&policy.build()))
    }

    #[setter]
    fn set_quotes(&mut self, value: QuoteStore) -> PyResult<()> {
        self.update_market_inputs(|context| context.with_quote_store(value.inner))
    }

    #[setter]
    fn set_fixings(&mut self, value: FixingStore) -> PyResult<()> {
        self.update_market_inputs(|context| context.with_fixing_store(value.inner))
    }

    #[setter]
    fn set_fx(&mut self, value: FxStore) -> PyResult<()> {
        self.update_market_inputs(|context| context.with_fx_store(value.inner))
    }

    #[setter]
    fn set_curve_configurations(&mut self, value: Vec<CurveConfiguration>) -> PyResult<()> {
        self.update_market_inputs(|context| {
            context.with_curve_configurations(
                value
                    .into_iter()
                    .map(|configuration| configuration.inner)
                    .collect(),
            )
        })
    }

    #[setter]
    fn set_credit_curve_configurations(
        &mut self,
        value: Vec<CreditCurveConfiguration>,
    ) -> PyResult<()> {
        self.update_market_inputs(|context| {
            context.with_credit_curve_configurations(
                value
                    .into_iter()
                    .map(|configuration| configuration.inner)
                    .collect(),
            )
        })
    }

    #[setter]
    fn set_volatility_surface_configurations(
        &mut self,
        value: Vec<VolatilitySurfaceConfiguration>,
    ) -> PyResult<()> {
        self.update_market_inputs(|context| {
            context.with_volatility_surface_configurations(
                value
                    .into_iter()
                    .map(|configuration| configuration.inner)
                    .collect(),
            )
        })
    }

    #[setter]
    fn set_volatility_cube_configurations(
        &mut self,
        value: Vec<VolatilityCubeConfiguration>,
    ) -> PyResult<()> {
        self.update_market_inputs(|context| {
            context.with_volatility_cube_configurations(
                value
                    .into_iter()
                    .map(|configuration| configuration.inner)
                    .collect(),
            )
        })
    }

    #[setter]
    fn set_simulation_configurations(
        &mut self,
        value: Vec<SimulationConfiguration>,
    ) -> PyResult<()> {
        self.update_market_inputs(|context| {
            context.with_simulation_configurations(
                value
                    .into_iter()
                    .map(|configuration| configuration.inner)
                    .collect(),
            )
        })
    }

    #[setter]
    fn set_pricer_configurations(&mut self, value: Vec<PricerConfiguration>) -> PyResult<()> {
        self.update_market_inputs(|context| {
            context.with_pricer_configurations(
                value
                    .into_iter()
                    .map(|configuration| configuration.inner)
                    .collect(),
            )
        })
    }

    /// Curve construction inputs. Assign a new collection to rebuild the market.
    #[getter]
    fn curves(&self) -> Vec<CurveConfiguration> {
        self.curve_configurations()
    }

    #[setter]
    fn set_curves(&mut self, value: Vec<CurveConfiguration>) -> PyResult<()> {
        self.set_curve_configurations(value)
    }

    /// Resolves all configured discount curves in the active market view.
    fn discount_curves(&self) -> PyResult<Vec<DiscountCurve>> {
        let requests = self
            .inner
            .curve_configurations()
            .iter()
            .map(|configuration| ConstructedElementRequest::DiscountCurve {
                market_index: configuration.market_index().clone(),
            })
            .collect();
        let market = self.request_elements(requests)?;
        let mut elements: Vec<_> = market
            .constructed_elements()
            .discount_curves()
            .values()
            .map(|element| DiscountCurve {
                element: element.clone(),
            })
            .collect();
        elements.sort_by_key(|item| item.element.market_index().to_string());
        Ok(elements)
    }

    /// Resolves one DiscountCurve in the active market view.
    fn curve(&self, index: &Bound<'_, PyAny>) -> PyResult<DiscountCurve> {
        let idx = extract_market_index(index)?;
        let market = self.request_elements(vec![ConstructedElementRequest::DiscountCurve {
            market_index: idx.clone(),
        }])?;
        market
            .constructed_elements()
            .discount_curve(&idx)
            .map(|element| DiscountCurve {
                element: element.clone(),
            })
            .ok_or_else(|| QuantSupportError::new_err(format!("No DiscountCurve for '{idx}'")))
    }

    /// Resolves all configured volatility surfaces in the active market view.
    fn volatility_surfaces(&self) -> PyResult<Vec<VolatilitySurface>> {
        let requests = self
            .inner
            .volatility_surface_configurations()
            .iter()
            .map(
                |configuration| ConstructedElementRequest::VolatilitySurface {
                    market_index: configuration.market_index().clone(),
                },
            )
            .collect();
        let market = self.request_elements(requests)?;
        let mut elements: Vec<_> = market
            .constructed_elements()
            .volatility_surfaces()
            .values()
            .map(|element| VolatilitySurface {
                element: element.clone(),
            })
            .collect();
        elements.sort_by_key(|item| item.element.market_index().to_string());
        Ok(elements)
    }

    /// Resolves one VolatilitySurface in the active market view.
    fn volatility_surface(&self, index: &Bound<'_, PyAny>) -> PyResult<VolatilitySurface> {
        let idx = extract_market_index(index)?;
        let market = self.request_elements(vec![ConstructedElementRequest::VolatilitySurface {
            market_index: idx.clone(),
        }])?;
        market
            .constructed_elements()
            .volatility_surface(&idx)
            .map(|element| VolatilitySurface {
                element: element.clone(),
            })
            .ok_or_else(|| QuantSupportError::new_err(format!("No VolatilitySurface for '{idx}'")))
    }

    /// Resolves all configured volatility cubes in the active market view.
    fn volatility_cubes(&self) -> PyResult<Vec<VolatilityCube>> {
        let requests = self
            .inner
            .volatility_cube_configurations()
            .iter()
            .map(|configuration| ConstructedElementRequest::VolatilityCube {
                market_index: configuration.market_index().clone(),
            })
            .collect();
        let market = self.request_elements(requests)?;
        let mut elements: Vec<_> = market
            .constructed_elements()
            .volatility_cubes()
            .values()
            .map(|element| VolatilityCube {
                element: element.clone(),
            })
            .collect();
        elements.sort_by_key(|item| item.element.market_index().to_string());
        Ok(elements)
    }

    /// Resolves one VolatilityCube in the active market view.
    fn volatility_cube(&self, index: &Bound<'_, PyAny>) -> PyResult<VolatilityCube> {
        let idx = extract_market_index(index)?;
        let market = self.request_elements(vec![ConstructedElementRequest::VolatilityCube {
            market_index: idx.clone(),
        }])?;
        market
            .constructed_elements()
            .volatility_cube(&idx)
            .map(|element| VolatilityCube {
                element: element.clone(),
            })
            .ok_or_else(|| QuantSupportError::new_err(format!("No VolatilityCube for '{idx}'")))
    }

    /// Resolves all configured simulations in the active market view.
    fn simulations(&self) -> PyResult<Vec<Simulation>> {
        let requests = self
            .inner
            .simulation_configurations()
            .iter()
            .map(|configuration| ConstructedElementRequest::Simulation {
                market_index: configuration.market_index().clone(),
            })
            .collect();
        let market = self.request_elements(requests)?;
        let mut elements: Vec<_> = market
            .constructed_elements()
            .simulations()
            .values()
            .map(|element| Simulation {
                element: element.clone(),
            })
            .collect();
        elements.sort_by_key(|item| item.element.market_index().to_string());
        Ok(elements)
    }

    /// Resolves one Simulation in the active market view.
    fn simulation(&self, index: &Bound<'_, PyAny>) -> PyResult<Simulation> {
        let idx = extract_market_index(index)?;
        let market = self.request_elements(vec![ConstructedElementRequest::Simulation {
            market_index: idx.clone(),
        }])?;
        market
            .constructed_elements()
            .simulations()
            .get(&idx)
            .map(|element| Simulation {
                element: element.clone(),
            })
            .ok_or_else(|| QuantSupportError::new_err(format!("No Simulation for '{idx}'")))
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

    fn __enter__(slf: Bound<'_, Self>) -> PyResult<Bound<'_, Self>> {
        {
            let mut context = slf.try_borrow_mut()?;
            context.inner.validate().map_err(qs_err)?;
            context.initialized = true;
        }
        if enter_context(&slf) {
            Tape::<ADForward>::start_recording_fwd();
        }
        Ok(slf)
    }

    #[pyo3(signature = (_exc_type = None, _exc_value = None, _traceback = None))]
    fn __exit__(
        slf: Bound<'_, Self>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let (same_context_remains, another_context_remains) = exit_context(&slf)?;
        if !same_context_remains {
            if !another_context_remains {
                Tape::<ADForward>::stop_recording_fwd();
                Tape::<ADForward>::rewind_to_init_fwd();
            }
            let mut context = slf.try_borrow_mut()?;
            *context.inner.constructed_elements_mut() = ConstructedElementStore::default();
            context.initialized = false;
            context.fully_constructed = false;
            context.generation += 1;
        }
        Ok(false)
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
            return Err(pyo3::exceptions::PyValueError::new_err(
                "n_paths must be positive",
            ));
        }
        let script = script
            .extract::<ScriptedProduct>()
            .or_else(|_| ScriptedProduct::from_dict(script))?;
        let discount_index = match script.discount_index() {
            Some(index) => index.clone(),
            None if script.currency() == self.inner.base_currency() => {
                self.inner.base_index().clone()
            }
            None => {
                return Err(QuantSupportError::new_err(
                    "scripts outside the context base currency require an explicit discount_index",
                ));
            }
        };
        let mut assignments = HashMap::new();
        for configuration in self.inner.pricer_configurations() {
            for definition in self.inner.curve_configurations() {
                let index = definition.market_index();
                if let Some(model) = configuration.model(index) {
                    if matches!(
                        model,
                        ModelConfiguration::Lgm { .. } | ModelConfiguration::HullWhite { .. }
                    ) {
                        assignments.insert(index.clone(), model.clone());
                    }
                }
            }
        }
        for configuration in self.inner.simulation_configurations() {
            if matches!(
                configuration.model(),
                ModelConfiguration::Lgm { .. } | ModelConfiguration::HullWhite { .. }
            ) {
                assignments.insert(
                    configuration.market_index().clone(),
                    configuration.model().clone(),
                );
            }
        }
        for configuration in script.models() {
            assignments.insert(
                configuration.market_index().clone(),
                configuration.model().clone(),
            );
        }
        if !assignments.contains_key(&discount_index) {
            return Err(QuantSupportError::new_err(format!(
                "no rate model configured for script discount index '{discount_index}'; supply script.models or context model configurations",
            )));
        }
        let mut assignments: Vec<_> = assignments.into_iter().collect();
        assignments.sort_by_key(|(index, _)| index.to_string());
        let view = self.market_view()?;
        let mut dependencies: Vec<_> = assignments
            .iter()
            .map(|(index, _)| ConstructedElementRequest::DiscountCurve {
                market_index: index.clone(),
            })
            .collect();
        for (_, model) in &assignments {
            if let ModelConfiguration::Lgm {
                parameter_source: ParameterSource::Calibrated(configuration),
                ..
            }
            | ModelConfiguration::HullWhite {
                parameter_source: ParameterSource::Calibrated(configuration),
                ..
            } = model
            {
                dependencies.push(match configuration.source() {
                    CalibrationSource::Surface { market_index } => {
                        ConstructedElementRequest::VolatilitySurface {
                            market_index: market_index.clone(),
                        }
                    }
                    CalibrationSource::Cube { market_index } => {
                        ConstructedElementRequest::VolatilityCube {
                            market_index: market_index.clone(),
                        }
                    }
                });
            }
        }
        let market = view
            .handle_request(
                &MarketDataRequest::default().with_constructed_elements_request(dependencies),
            )
            .map_err(qs_err)?;
        let store = market.constructed_elements();
        let elements = assignments
            .iter()
            .map(|(index, _)| {
                store.discount_curve(index).ok_or_else(|| {
                    QuantSupportError::new_err(format!(
                        "no bootstrapped curve for script model '{index}'"
                    ))
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        let curves: Vec<_> = elements.iter().map(|element| element.curve()).collect();
        let reference_date = self.inner.evaluation_date();
        let day_counter = store
            .discount_curve(&discount_index)
            .ok_or_else(|| {
                QuantSupportError::new_err(format!("no script discount curve '{discount_index}'"))
            })?
            .curve()
            .day_counter()
            .unwrap_or(DayCounter::Actual365);
        let mut market_model = LgmMarketModel::new(
            script.currency(),
            discount_index.clone(),
            reference_date,
            day_counter,
        )
        .with_n_paths(n_paths)
        .with_seed(seed);
        for (((index, configuration), element), curve) in
            assignments.iter().zip(&elements).zip(&curves)
        {
            let configuration = match configuration {
                ModelConfiguration::HullWhite {
                    alpha,
                    parameter_source,
                } => ModelConfiguration::Lgm {
                    lambda: *alpha,
                    parameter_source: parameter_source.clone(),
                },
                other => other.clone(),
            };
            let ModelConfiguration::Lgm { lambda, .. } = &configuration else {
                return Err(QuantSupportError::new_err(format!(
                    "script rate model '{index}' must be Lgm or HullWhite",
                )));
            };
            if !lambda.is_finite() || *lambda < 0.0 {
                return Err(QuantSupportError::new_err(
                    "rate model mean reversion must be finite and non-negative",
                ));
            }
            let plain_curve = element.to_f64_term_structure(day_counter).map_err(qs_err)?;
            let resolved = LgmRateModel::from_configuration(
                &configuration,
                &plain_curve,
                store,
                &view,
                self.inner.quote_level(),
            )
            .map_err(qs_err)?;
            let schedule = resolved
                .sigma_schedule()
                .iter()
                .map(|(time, sigma)| (*time, DualFwd::scalar(*sigma)))
                .collect();
            let rate_model =
                LgmRateModel::new_piecewise(DualFwd::scalar(*lambda), schedule, &**curve)
                    .map_err(qs_err)?;
            market_model.add_curve_model(index.clone(), rate_model);
        }
        ScriptEngine::new(
            script.event_stream()?,
            reference_date,
            script.currency(),
            discount_index,
        )
        .map_err(qs_err)?
        .evaluate(&mut market_model, script.result_variable())
        .map_err(qs_err)
    }

    /// Evaluates a trade.
    ///
    /// `requests` is a list of [`crate::common::enums::Request`] values (or their
    /// string names). Defaults to `[Request.Value]`.
    #[pyo3(signature = (trade, requests = None))]
    fn evaluate<'py>(
        slf: PyRef<'py, Self>,
        py: Python<'py>,
        trade: &Bound<'py, PyAny>,
        requests: Option<Vec<Bound<'py, PyAny>>>,
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

        let market = slf.market_view()?;

        // Native pricers start a fresh tape and attach their market pillars.
        // Build contractual amounts/rates without tape nodes: recording them
        // here would leave pointers into storage reused by that fresh tape,
        // making fixed contract terms appear sensitive to unrelated quotes.
        Tape::<ADForward>::stop_recording_fwd();

        let position = trade
            .extract::<PyRef<'_, crate::pricing::extension::Trade>>()
            .map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err(
                "native pricing requires Trade(instrument, trade_date, side), not a bare instrument"
            )
            })?;
        let instrument = position.instrument_bound(py);

        if let Ok(loan) = instrument.extract::<crate::instruments::loan::Loan>() {
            let trade = loan.build_native_loan_trade(&position)?;
            let mut pricer = LoanPricer::new();
            pricer.set_discount_policy(Box::new(slf.build_loan_discount_policy()));
            let result = pricer.evaluate(&trade, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&result));
        }

        if let Ok(swap) = instrument.extract::<Swap>() {
            let t = swap.build_trade_dual(&position)?;
            let mut pricer = DiscountedCashflowPricer::<QsSwap<DualFwd>, SwapTrade<DualFwd>>::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(xccy) = instrument.extract::<CrossCurrencySwap>() {
            let t = xccy.build_trade_dual(&position)?;
            let mut pricer = DiscountedCashflowPricer::<
                FloatFloatCrossCurrencySwap<DualFwd>,
                FloatFloatCrossCurrencySwapTrade<DualFwd>,
            >::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(basis) = instrument.extract::<BasisSwap>() {
            let t = basis.build_trade_dual(&position)?;
            let mut pricer =
                DiscountedCashflowPricer::<QsBasisSwap<DualFwd>, BasisSwapTrade<DualFwd>>::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(xccy) = instrument.extract::<FixFloatCrossCurrencySwap>() {
            let t = xccy.build_trade_dual(&position)?;
            let mut pricer = DiscountedCashflowPricer::<
                QsFixFloatXccy<DualFwd>,
                FixFloatCrossCurrencySwapTrade<DualFwd>,
            >::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(bond) = instrument.extract::<FixedRateBond>() {
            let t = bond.build_trade_dual(&position)?;
            let mut pricer = BondPricer::new();
            pricer.set_discount_policy(Box::new(slf.bond_discount_policy(true)));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(bond) = instrument.extract::<SteppedCouponBond>() {
            let t = bond.build_trade_dual(&position)?;
            let res = match slf
                .inner
                .pricer_configuration(InstrumentType::SteppedCouponBond)
                .map(|configuration| configuration.pricer())
            {
                Some(PricerImplementation::BackwardFourierEvolution { .. }) => {
                    let mut pricer = BackwardFourierEvolutionBondPricer::new();
                    pricer.set_discount_policy(Box::new(slf.bond_discount_policy(false)));
                    pricer.evaluate(&t, &reqs, &market)
                }
                Some(PricerImplementation::TreeBased { .. }) => {
                    let mut pricer = TreeBasedBondPricer::new();
                    pricer.set_discount_policy(Box::new(slf.bond_discount_policy(false)));
                    pricer.evaluate(&t, &reqs, &market)
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
        if let Ok(frn) = instrument.extract::<FloatingRateNote>() {
            let t = frn.build_trade_dual(&position)?;
            let mut pricer = DiscountedCashflowPricer::<
                QsFloatingRateNote<DualFwd>,
                FloatingRateNoteTrade<DualFwd>,
            >::new();
            pricer.set_discount_policy(Box::new(slf.bond_discount_policy(true)));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(deposit) = instrument.extract::<FixedRateDeposit>() {
            let t = deposit.build_trade_dual(&position)?;
            let mut pricer = DiscountedCashflowPricer::<
                QsFixedRateDeposit<DualFwd>,
                FixedRateDepositTrade<DualFwd>,
            >::new();
            pricer.set_discount_policy(Box::new(slf.bond_discount_policy(true)));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(fwd) = instrument.extract::<FxForwardPy>() {
            let t = fwd.build_trade(&position)?;
            let mut pricer = FxForwardPricer::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(opt) = instrument.extract::<FxOptionPy>() {
            let t = opt.build_trade(&position)?;
            let mut pricer = FxEuropeanOptionPricer::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(fwd) = instrument.extract::<crate::instruments::products::EquityForward>() {
            let trade = fwd.build_trade(&position)?;
            let mut pricer = quantsupport::prelude::EquityForwardPricer::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let result = pricer.evaluate(&trade, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&result));
        }
        if let Ok(option) = instrument.extract::<crate::instruments::products::EuropeanSwaption>() {
            let configuration = slf
                .inner
                .pricer_configuration(InstrumentType::EuropeanSwaption)
                .ok_or_else(|| {
                    QuantSupportError::new_err("EuropeanSwaption requires a pricer configuration")
                })?;
            let mut pricer =
                quantsupport::prelude::ClosedFormHullWhiteSwaptionPricer::from_configuration(
                    configuration,
                    &option.market_index,
                )
                .map_err(qs_err)?;
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let trade = option.build_trade_dual(&position)?;
            let result = pricer.evaluate(&trade, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&result));
        }
        if let Ok(opt) = instrument.extract::<EquityOption>() {
            let t = opt.build_trade(&position)?;
            let mut pricer = BlackEuropeanOptionPricer::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(cds) = instrument.extract::<CreditDefaultSwapPy>() {
            let t = cds.build_trade(&position)?;
            let mut pricer = CdsPricer::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(cap) = instrument.extract::<CapFloorPy>() {
            let t = cap.build_trade(&position)?;
            let mut pricer = ClosedFormBlackCapPricer::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(caplet) = instrument.extract::<CapletFloorletPy>() {
            let t = caplet.build_trade(&position)?;
            let mut pricer = ClosedFormBlackCapletPricer::new();
            pricer.set_discount_policy(Box::new(slf.inner.csa_policy()));
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }
        if let Ok(futures) = instrument.extract::<RateFuturesPy>() {
            let t = futures.build_trade(&position)?;
            let pricer = RateFuturesPricer::new();
            let res = pricer.evaluate(&t, &reqs, &market).map_err(qs_err)?;
            return Py::new(py, EvaluationResults::from_qs(&res));
        }

        Err(QuantSupportError::new_err(format!(
            "no native pricer for instrument type '{}'; \
             call context.register_pricer(TradeType, pricer)",
            instrument.get_type().name()?
        )))
    }

    /// Runs the XVA engine.
    ///
    /// - `config`: an [`XvaConfig`] (models, paths, seed, frequency).
    /// - `netting_sets`: list of [`NettingSet`], each carrying its client's
    ///   [`crate::xva::CsaTerms`] (collateral treatment + credit/funding).
    #[allow(clippy::needless_pass_by_value)]
    fn run_xva(
        &self,
        py: Python<'_>,
        config: XvaConfig,
        netting_sets: Vec<Py<NettingSet>>,
    ) -> PyResult<XvaResult> {
        self.require_initialized()?;

        let mut sets = HashMap::new();
        for ns in &netting_sets {
            let ns = ns.borrow(py);
            if sets.contains_key(ns.name()) {
                return Err(QuantSupportError::new_err(format!(
                    "duplicate netting set name '{}'",
                    ns.name()
                )));
            }
            sets.insert(ns.name().to_string(), ns.build(py)?);
        }

        let mut engine =
            XvaEngine::with_market_provider(&self.inner, &self.market_view()?, config.inner)
                .map_err(qs_err)?;
        let result = engine.run(&mut sets).map_err(qs_err)?;
        Ok(XvaResult::from_qs(&result))
    }
}

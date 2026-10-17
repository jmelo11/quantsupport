"""Positions, valuation contexts, pricing policies, extension points, and results."""
from datetime import date as PythonDate
from typing import Any, ClassVar, Mapping, Sequence, final, overload
from typing_extensions import Self, disjoint_base
from pandas import DataFrame
from ..time import Date, _DateLike
from ..market import CreditCurveConfiguration, Currency, CurveConfiguration, DiscountCurve, FixingStore, FxStore, Level, MarketIndex, QuoteStore, Simulation, SimulationConfiguration, VolatilityCube, VolatilityCubeConfiguration, VolatilitySurface, VolatilitySurfaceConfiguration, _CurrencyLike, _IndexLike
from ..instruments import Instrument, Side
from ..risk import XvaResult
__all__ = ['Request', 'PricerConfiguration', 'SingleCurveCSADiscountPolicy', 'FixedIncomeDiscountPolicy', 'BondPricingAnchor', 'PricingContext', 'LoanPricer', 'Trade', 'Pricer', 'MarketDataProvider', 'Evaluator', 'EvaluationResults']

@final
class Request:
    """Evaluation request."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Request:
        """Parses a request from its name."""
        ...
    Value: ClassVar[Request]
    YieldToMaturity: ClassVar[Request]
    ModifiedDuration: ClassVar[Request]
    Sensitivities: ClassVar[Request]
    ZSpread: ClassVar[Request]
    OAS: ClassVar[Request]
    Cashflows: ClassVar[Request]
    FairRate: ClassVar[Request]

@final
class PricerConfiguration:
    """Wrapper around the library's `QsPricerConfiguration`."""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> PricerConfiguration:
        """Builds a single configuration from a dict."""
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[PricerConfiguration]:
        """Builds configurations from a list, single dict, or wrapped list."""
        ...

    @staticmethod
    def from_json(path: str) -> list[PricerConfiguration]:
        """Loads configurations from a JSON file. Accepts a list, a
single object, or an object wrapping a single list (e.g.
`{"curve_specs": [...]}`). Always returns a list."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns the configuration as a dict."""
        ...

@final
class SingleCurveCSADiscountPolicy:
    """Collateral currency and remuneration curve used by the single-curve CSA policy."""

    def __new__(cls, collateral_currency: _CurrencyLike, collateral_index: _IndexLike) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> SingleCurveCSADiscountPolicy:
        """Builds a policy from collateral_currency and collateral_index."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return the native JSON-compatible representation."""
        ...

    @property
    def collateral_currency(self) -> Any:
        """Read-only currency in which collateral is posted."""
        ...

    @property
    def collateral_index(self) -> Any:
        """Read-only market index of the collateral remuneration curve."""
        ...

@final
class FixedIncomeDiscountPolicy:
    """Select discount curves by the cashflow currency.

Parameters
----------
risk_free_indices : dict[Currency | str, MarketIndex | str]
    Currency-to-curve mapping. Each index must identify a configured curve.
prefer_instrument_index : bool, optional
    Use an instrument's discount index when present. Defaults to True.
    Loans obtain their discount curve from the currency mapping.

Examples
--------
FixedIncomeDiscountPolicy(risk_free_indices={Currency.EUR: MarketIndex.ESTR})"""

    def __new__(cls, risk_free_indices: Mapping[_CurrencyLike, _IndexLike], prefer_instrument_index: bool=...) -> Self:
        """Map each cashflow currency to its discount curve's market index."""
        ...

@final
class BondPricingAnchor:
    """Market observable used to anchor native straight or callable bond pricing.

Use one of the named constructors; clean and dirty prices are quoted per
the bond's units. Price anchors imply Z-spread for straight bonds and OAS
for bonds with an issuer-call schedule."""

    @staticmethod
    def clean_price(value: float) -> Any:
        """Anchors the trade to a clean price."""
        ...

    @staticmethod
    def dirty_price(value: float) -> Any:
        """Anchors the trade to a dirty price."""
        ...

    @staticmethod
    def z_spread(value: float) -> Any:
        """Anchors a straight bond to a continuously compounded Z-spread."""
        ...

    @staticmethod
    def OAS(value: float) -> Any:
        """Anchors an issuer-call bond to an OAS."""
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> BondPricingAnchor:
        """Loads an anchor such as {"CleanPrice": 99.5} or {"OAS": 0.0075}."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serializes the native anchor enum."""
        ...

@disjoint_base
class PricingContext:
    """Market inputs and policies used to value trades.

Assign public properties to change inputs: curves, curve_configurations,
credit_curve_configurations, fixings, quotes, fx, quote_level, csa_policy,
fixed_income_policy, volatility_surface_configurations,
volatility_cube_configurations, simulation_configurations, and
pricer_configurations. Reassigning market inputs invalidates constructed data;
construction follows subsequent market-data requests.
Collection getters return copies; assign the updated collection back.
Use ``with context, scenario:`` for temporary ``Scenario`` shocks;
the base market remains available when the scenario block exits.

Parameters
----------
quotes : QuoteStore
    Market observations and reference date.
curves : sequence of CurveConfiguration
    Calibration inputs, explicit zero rates, and composite curves.
fixings : FixingStore, optional
    Published historical index observations.
fx : FxStore, optional
    Spot exchange rates.
volatility_surfaces : sequence of VolatilitySurfaceConfiguration, optional
    Surface construction inputs.
volatility_cubes : sequence of VolatilityCubeConfiguration, optional
    Cube construction inputs.
pricer_configurations : sequence of PricerConfiguration, optional
    Instrument pricing models and implementations.
simulations : sequence of SimulationConfiguration, optional
    Simulation inputs.
csa_policy : SingleCurveCSADiscountPolicy, optional
    Collateral currency and curve used for market construction.
fixed_income_policy : FixedIncomeDiscountPolicy, optional
    Currency-to-curve discount policy. Defaults to the context base curve.
quote_level : Level or str, optional
    Quote level used for calibration. Defaults to Mid.
credit_curves : sequence of CreditCurveConfiguration, optional
    Survival curve construction inputs.

Examples
--------

.. code-block:: python

   quotes = qs.QuoteStore.from_json("quotes.json")
   curves = qs.CurveConfiguration.from_json("curve_specs.json")
   policy = qs.SingleCurveCSADiscountPolicy("USD", "SOFR")
   trade = qs.Trade(swap, quotes.reference_date)

   with qs.PricingContext(quotes=quotes, curves=curves, csa_policy=policy) as ctx:
       results = ctx.evaluate(trade, requests=["Value", "Sensitivities"])
"""

    def __new__(cls, quotes: QuoteStore, curves: Sequence[CurveConfiguration], fixings: FixingStore | None=..., fx: FxStore | None=..., volatility_surfaces: Sequence[VolatilitySurfaceConfiguration] | None=..., volatility_cubes: Sequence[VolatilityCubeConfiguration] | None=..., pricer_configurations: Sequence[PricerConfiguration] | None=..., simulations: Sequence[SimulationConfiguration] | None=..., csa_policy: SingleCurveCSADiscountPolicy | None=..., fixed_income_policy: FixedIncomeDiscountPolicy | None=..., quote_level: Level | str | None=..., credit_curves: Sequence[CreditCurveConfiguration] | None=...) -> Self:
        ...

    def initialize(self) -> None:
        """Constructs curves, volatility surfaces/cubes, and simulations.

Explicitly constructs every configured object. Context entry enables
lazy construction of requested market objects instead."""
        ...

    def discount_curves(self) -> list[DiscountCurve]:
        """Resolves all configured discount curves in the active market view."""
        ...

    @property
    def curves(self) -> list[CurveConfiguration]:
        """Curve construction inputs. Assign a new collection to rebuild the market."""
        ...

    @curves.setter
    def curves(self, value: Sequence[CurveConfiguration]) -> None:
        """Curve construction inputs. Assign a new collection to rebuild the market."""
        ...

    def curve(self, index: _IndexLike) -> DiscountCurve:
        """Resolves one DiscountCurve in the active market view."""
        ...

    def volatility_surfaces(self) -> list[VolatilitySurface]:
        """Resolves all configured volatility surfaces in the active market view."""
        ...

    def volatility_surface(self, index: _IndexLike) -> VolatilitySurface:
        """Resolves one VolatilitySurface in the active market view."""
        ...

    def volatility_cubes(self) -> list[VolatilityCube]:
        """Resolves all configured volatility cubes in the active market view."""
        ...

    def volatility_cube(self, index: _IndexLike) -> VolatilityCube:
        """Resolves one VolatilityCube in the active market view."""
        ...

    def simulations(self) -> list[Simulation]:
        """Resolves all configured simulations in the active market view."""
        ...

    def simulation(self, index: _IndexLike) -> Simulation:
        """Resolves one Simulation in the active market view."""
        ...

    def register_pricer(self, trade_type: Any, pricer: Any) -> Any:
        """Registers (or replaces) a Python pricer for a Python trade class.

The most specific exact type is selected first; registered base classes
also handle their subclasses. Python registrations take precedence over
built-in dispatch, which makes built-in trade classes overridable too."""
        ...

    def unregister_pricer(self, trade_type: Any) -> Any:
        """Removes the Python pricer registered for an exact trade class."""
        ...

    def pricer_for(self, trade: Trade) -> Any:
        """Returns the Python pricer selected for a trade, or `None` when built-in
dispatch will be used."""
        ...

    def __enter__(self) -> Self:
        ...

    def __exit__(self, _exc_type: Any | None=..., _exc_value: Any | None=..., _traceback: Any | None=...) -> bool:
        ...

    def evaluate_script(self, script: str, n_paths: int=..., seed: int=...) -> Any:
        """Evaluates dated payoff code with native LGM models and bootstrapped context curves."""
        ...

    def evaluate(self, trade: Trade, requests: Sequence[Request | str] | None=...) -> EvaluationResults:
        """Evaluates a trade.

``requests`` contains :class:`Request` values or their string names.
Defaults to ``[Request.Value]``."""
        ...

    def run_xva(self, config: Any, netting_sets: Any) -> XvaResult:
        """Runs the XVA engine.

``config`` is a :class:`quantsupport.risk.XvaConfig` defining models,
paths, seed, and frequency. ``netting_sets`` contains
:class:`quantsupport.risk.NettingSet` objects, each carrying its client's
:class:`quantsupport.risk.CsaTerms` for collateral, credit, and funding."""
        ...

    @property
    def curve_configurations(self) -> list[CurveConfiguration]:
        """The original curve configurations (bootstrap inputs)."""
        ...

    @curve_configurations.setter
    def curve_configurations(self, value: Sequence[CurveConfiguration]) -> None:
        """The original curve configurations (bootstrap inputs)."""
        ...

    @property
    def simulation_configurations(self) -> list[SimulationConfiguration]:
        """The original simulation configurations."""
        ...

    @simulation_configurations.setter
    def simulation_configurations(self, value: Sequence[SimulationConfiguration]) -> None:
        """The original simulation configurations."""
        ...

    @property
    def reference_date(self) -> Date:
        """Reference date of the underlying quote store."""
        ...

    @property
    def quotes(self) -> QuoteStore:
        """Snapshot of quote inputs in the active base or scenario market view."""
        ...

    @quotes.setter
    def quotes(self, value: QuoteStore) -> None:
        """Snapshot of quote inputs in the active base or scenario market view."""
        ...

    @property
    def csa_policy(self) -> SingleCurveCSADiscountPolicy:
        """Default collateral policy used for curve construction and derivative valuation."""
        ...

    @csa_policy.setter
    def csa_policy(self, value: SingleCurveCSADiscountPolicy) -> None:
        """Default collateral policy used for curve construction and derivative valuation."""
        ...

    @property
    def volatility_surface_configurations(self) -> list[VolatilitySurfaceConfiguration]:
        """The original volatility surface configurations."""
        ...

    @volatility_surface_configurations.setter
    def volatility_surface_configurations(self, value: Sequence[VolatilitySurfaceConfiguration]) -> None:
        """The original volatility surface configurations."""
        ...

    @property
    def volatility_cube_configurations(self) -> list[VolatilityCubeConfiguration]:
        """The original volatility cube configurations."""
        ...

    @volatility_cube_configurations.setter
    def volatility_cube_configurations(self, value: Sequence[VolatilityCubeConfiguration]) -> None:
        """The original volatility cube configurations."""
        ...

    @property
    def credit_curve_configurations(self) -> list[CreditCurveConfiguration]:
        """The original credit curve configurations (bootstrap inputs)."""
        ...

    @credit_curve_configurations.setter
    def credit_curve_configurations(self, value: Sequence[CreditCurveConfiguration]) -> None:
        """The original credit curve configurations (bootstrap inputs)."""
        ...

    @property
    def evaluation_date(self) -> Date:
        """Alias for `reference_date`, matching the `MarketDataProvider` contract."""
        ...

    @property
    def base_currency(self) -> Currency:
        """Base collateral currency used for curve construction."""
        ...

    @property
    def quote_level(self) -> Level:
        """Quote level used for construction and calibration; defaults to `Level.Mid`."""
        ...

    @quote_level.setter
    def quote_level(self, value: Level | str) -> None:
        """Quote level used for construction and calibration; defaults to `Level.Mid`."""
        ...

    @property
    def pricer_configurations(self) -> list[PricerConfiguration]:
        """Instrument-level pricer selections and typed model assignments."""
        ...

    @pricer_configurations.setter
    def pricer_configurations(self, value: Sequence[PricerConfiguration]) -> None:
        """Instrument-level pricer selections and typed model assignments."""
        ...

    @property
    def base_index(self) -> MarketIndex:
        """Base collateral/remuneration index used by the context."""
        ...

    @property
    def fixings(self) -> FixingStore:
        """Historical fixings available to built-in and Python pricers."""
        ...

    @fixings.setter
    def fixings(self, value: FixingStore) -> None:
        """Historical fixings available to built-in and Python pricers."""
        ...

    @property
    def fx(self) -> FxStore:
        """FX spot store available to built-in and Python pricers."""
        ...

    @fx.setter
    def fx(self, value: FxStore) -> None:
        """FX spot store available to built-in and Python pricers."""
        ...

    @property
    def fixed_income_policy(self) -> FixedIncomeDiscountPolicy | None:
        """Fixed-income discount policy, or None for the context base-curve policy."""
        ...

    @fixed_income_policy.setter
    def fixed_income_policy(self, value: FixedIncomeDiscountPolicy | None) -> None:
        """Fixed-income discount policy, or None for the context base-curve policy."""
        ...

@final
class LoanPricer:
    """Signed receipt PV, expected cashflows, raw pillar sensitivities, and fair rates."""

    def __new__(cls, *, refinancing_benchmark_index: _IndexLike | None=..., discount_policy: FixedIncomeDiscountPolicy | None=..., rebuild_amortization_for_fair_rate: bool=...) -> Self:
        """Configure refinancing/discount overrides and whether FairRate trials rebuild amortization."""
        ...

    def evaluate(self, trade: Trade, context: PricingContext, requests: Sequence[Request | str] | None=...) -> EvaluationResults:
        """Evaluate Trade(Loan(...)) in an initialized context; default to Value, excluding the advance."""
        ...

@disjoint_base
class Trade:
    """Generic position in an instrument.

This mirrors the Rust `Trade` contract while remaining subclassable so a
Python trade can carry any product-specific state it needs."""

    def __new__(cls, instrument: Instrument, trade_date: _DateLike, side: Side | str | None=..., *, notional: float | None=..., domestic_notional: float | None=..., foreign_notional: float | None=..., num_contracts: float=..., pricing_anchor: Any | None=..., settlement_date: _DateLike | None=..., spread_day_counter: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    def with_pricing_anchor(self, anchor: Any) -> Any:
        """Returns another position in the same instrument with a different bond anchor."""
        ...

    @property
    def settlement_date(self) -> Any:
        """Read-only bond settlement-date override, or None when unspecified."""
        ...

    @property
    def instrument(self) -> Instrument:
        """Instrument held by this position."""
        ...

    @property
    def num_contracts(self) -> float | None:
        """Read-only futures position size in contracts; defaults to 1.0."""
        ...

    @property
    def pricing_anchor(self) -> Any:
        """Read-only bond price or spread anchor, or None when unspecified."""
        ...

    @property
    def spread_day_counter(self) -> Any:
        """Read-only day-count override for bond spread calculations, or None."""
        ...

    @property
    def side(self) -> Side:
        """Pay/receive direction."""
        ...

    @property
    def notional(self) -> float | None:
        """Position amount; None uses the instrument's contractual amount."""
        ...

    @property
    def foreign_notional(self) -> float | None:
        """Read-only foreign-currency leg amount override; None keeps the contract amount."""
        ...

    @property
    def trade_date(self) -> Date:
        """Execution date."""
        ...

    @property
    def domestic_notional(self) -> float | None:
        """Read-only domestic-currency leg amount override; None keeps the contract amount."""
        ...

@disjoint_base
class Pricer:
    """Optional base class for Python pricing implementations.

Override `evaluate(trade, requests, context)` and return an
`EvaluationResults`. The `context` is a market-data provider (normally an
initialized `PricingContext`), so the implementation can resolve curves,
volatility objects, fixings, FX, or quotes through the same boundary used
by built-in pricers."""

    def __new__(cls) -> Self:
        ...

    def evaluate(self, _trade: Any, _requests: Any, _context: Any) -> EvaluationResults:
        """Evaluates a trade. Subclasses must override this method."""
        ...

    def market_data_request(self, _trade: Any) -> Any:
        """Describes required market data, if the implementation uses an explicit
request object. Python pricers may also query the context directly."""
        ...

    def set_discount_policy(self, policy: Any) -> Any:
        """Installs a custom discount policy object for the Python pricer."""
        ...

    @property
    def discount_policy(self) -> Any:
        """Currently installed custom discount policy object."""
        ...

@disjoint_base
class MarketDataProvider:
    """Optional base class for Python market-data providers.

A provider may expose whichever query methods its pricers need (for
example `curve`, `fixing`, or `fx_rate`). The only common contract is an
`evaluation_date`. `PricingContext` is the full built-in provider, while
ordinary Python objects can be used with the standalone `Evaluator`."""

    def __new__(cls) -> Self:
        ...

    @property
    def evaluation_date(self) -> Date:
        """Reference date supplied by a concrete provider."""
        ...

@final
class Evaluator:
    """Runtime dispatcher for heterogeneous Python trade collections.

Registrations are keyed by Python trade type, matching the role of the
Rust `Evaluator`'s `TypeId -> ErasedPricer` map."""

    def __repr__(self) -> str:
        ...

    def __len__(self) -> int:
        ...

    def register(self, trade_type: Any, pricer: Any) -> Any:
        """Associates a Python trade class with a pricer object."""
        ...

    def unregister(self, trade_type: Any) -> Any:
        """Removes the registration for an exact Python trade class."""
        ...

    def pricer_for(self, trade: Trade) -> Any:
        """Returns the pricer selected for a trade, or `None`."""
        ...

    def evaluate(self, trade: Trade, context: Any, requests: Sequence[Request | str] | None=...) -> Any:
        """Evaluates a registered trade against a market-data provider."""
        ...

@disjoint_base
class EvaluationResults:
    """Results of evaluating a trade."""

    def __new__(cls, evaluation_date: _DateLike, identifier: str, *, price: Any | None=..., yield_to_maturity: Any | None=..., modified_duration: Any | None=..., z_spread: Any | None=..., oas: Any | None=..., fair_rate: Any | None=..., sensitivities: Any | None=..., cashflows: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @property
    def z_spread(self) -> Any:
        """Z-spread, if requested and supported by the pricer."""
        ...

    @property
    def identifier(self) -> str:
        """Evaluated instrument identifier."""
        ...

    @property
    def evaluation_date(self) -> Date:
        """Reference or as-of date of the evaluation."""
        ...

    @property
    def yield_to_maturity(self) -> Any:
        """Yield to maturity, if requested and supported by the pricer."""
        ...

    @property
    def fair_rate(self) -> float | None:
        """Fair (par) rate, if `"FairRate"` was requested."""
        ...

    @property
    def ytm(self) -> Any:
        """Alias for `yield_to_maturity`."""
        ...

    @property
    def oas(self) -> Any:
        """OAS, if requested and supported by the pricer."""
        ...

    @property
    def modified_duration(self) -> Any:
        """Modified duration, if requested and supported by the pricer."""
        ...

    @property
    def sensitivities(self) -> DataFrame | None:
        """Sensitivities as a DataFrame with columns pillar/value (duplicated
pillars summed, sorted by pillar), if `"Sensitivities"` was requested."""
        ...

    @property
    def cashflows(self) -> DataFrame | None:
        """Cashflow table as a DataFrame, if `"Cashflows"` was requested."""
        ...

    @property
    def price(self) -> float | None:
        """Net present value, if `"Value"` was requested."""
        ...

    @property
    def raw_sensitivities(self) -> Any:
        """Raw sensitivities as an ordered list of `(pillar_label, exposure)` tuples."""
        ...

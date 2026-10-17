"""Positions, valuation contexts, pricing policies, extension points, and results."""
from datetime import date as PythonDate
from typing import Any, ClassVar, Mapping, Sequence, final, overload
from typing_extensions import Self, disjoint_base
from pandas import DataFrame
from ..time import Date, DateTime, _DateLike
from ..market import CreditCurveConfiguration, Currency, CurveConfiguration, DiscountCurve, FixingStore, FxStore, Level, MarketIndex, QuoteStore, Simulation, SimulationConfiguration, VolatilityCube, VolatilityCubeConfiguration, VolatilitySurface, VolatilitySurfaceConfiguration, _CurrencyLike, _IndexLike
from ..instruments import Instrument, ScriptedProduct, Side
from ..risk import XvaResult
__all__ = ['Request', 'PricerConfiguration', 'SingleCurveCSADiscountPolicy', 'FixedIncomeDiscountPolicy', 'BondPricingAnchor',
           'PricingContext', 'LoanPricer', 'Trade', 'Pricer', 'MarketDataProvider', 'Evaluator', 'EvaluationResults']


@final
class Request:
    """Evaluation request."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Request:
        """Parse an evaluation request by its case-insensitive variant name.

Parameters
----------
s : str
    One of ``Value``, ``YieldToMaturity``, ``ModifiedDuration``,
    ``Sensitivities``, ``ZSpread``, ``OAS``, ``Cashflows``, or ``FairRate``.
    Variant names are case-insensitive. Additional exact aliases are ``npv``,
    ``ytm``, ``fair_rate``, ``modified_duration``, ``z_spread``, and ``zspread``.

Returns
-------
Request
    Corresponding request; support depends on the selected pricer.

Raises
------
QuantSupportError
    If the name is not a recognized request.
"""
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
    """Select a native pricing implementation and its index-keyed models.

Load configurations through from_dict(), from_dict_list(), or from_json().
They select an engine for an instrument family; they are not Python-pricer
registrations. Native compatibility checks run when the context is validated.
"""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> PricerConfiguration:
        """Deserialize one instrument-level pricing configuration.

Parameters
----------
obj : mapping
    Native configuration with required ``instrument_type`` and ``pricer``
    fields. ``models`` defaults to an empty list; each supplied entry requires
    ``market_index`` and a native tagged ``model``. Unit pricer variants use
    strings, for example ``"Bond"``; data-carrying variants use tagged mappings,
    for example ``{"TreeBased": {}}``. Unknown configuration fields are rejected.

Returns
-------
PricerConfiguration
    Deserialized configuration. Loading does not itself validate the selected
    instrument/pricer pairing or numerical grids.

Raises
------
QuantSupportError
    If the configuration cannot be deserialized.
"""
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[PricerConfiguration]:
        """Deserialize one or more native pricing configurations.

Parameters
----------
obj : mapping or list of mappings
    One configuration in the from_dict() schema, a list of configurations,
    or a one-key mapping whose sole value is that list, for example
    ``{"pricers": [...]}``. The wrapper key is arbitrary; it must be the only
    key. Runtime also accepts the list form despite the retained annotation.

Returns
-------
list of PricerConfiguration
    Configurations in input order, even for a single input configuration.

Raises
------
QuantSupportError
    If the container or any contained configuration cannot be deserialized.
"""
        ...

    @staticmethod
    def from_json(path: str) -> list[PricerConfiguration]:
        """Load instrument-level pricing configurations from a JSON file.

Parameters
----------
path : str
    Filesystem path, relative to the current working directory or absolute.
    The UTF-8 file must contain one from_dict() configuration, a list of them,
    or an object with exactly one key whose value is such a list. This is a
    file path, not inline JSON text.

Returns
-------
list of PricerConfiguration
    Configurations in file order; a single object still produces a list.

Raises
------
QuantSupportError
    If the file cannot be read or its JSON/configuration schema is invalid.
"""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the native pricing configuration.

Returns
-------
dict[str, Any]
    Native instrument_type, pricer, and models fields, including serde enum
    tags. Suitable as input to from_dict(), not a display-only description.

Raises
------
QuantSupportError
    If serialization to Python objects fails.
"""
        ...


@final
class SingleCurveCSADiscountPolicy:
    """Collateral currency and remuneration curve for a single-curve CSA.

Parameters
----------
collateral_currency : Currency or str
    Currency in which collateral is posted, as a typed currency or its ISO
    alphabetic code, for example ``Currency.USD`` or ``"USD"``.
collateral_index : MarketIndex or str
    Collateral remuneration index, as a typed index or its serialized name,
    for example ``MarketIndex.SOFR`` or ``"SOFR"``. The context must be able
    to construct the associated discount curve when it is requested.

Notes
-----
Both arguments are required. The resulting policy is immutable.
"""

    def __new__(cls, collateral_currency: _CurrencyLike, collateral_index: _IndexLike) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> SingleCurveCSADiscountPolicy:
        """Deserialize a single-curve collateral policy.

Parameters
----------
obj : mapping
    Native serialized policy with required ``collateral_currency`` and
    ``collateral_index`` fields. Currency and index values must use their
    native serde representation, not arbitrary display strings. Unknown
    fields are rejected.

Returns
-------
SingleCurveCSADiscountPolicy
    Immutable collateral policy.

Raises
------
QuantSupportError
    If the mapping cannot be deserialized as a collateral policy.
"""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the collateral policy.

Returns
-------
dict[str, Any]
    Native collateral_currency and collateral_index fields, suitable for
    from_dict(). Data-carrying index variants retain their serde tags.

Raises
------
QuantSupportError
    If serialization to Python objects fails.
"""
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

    def __new__(cls, risk_free_indices: Mapping[_CurrencyLike, _IndexLike], prefer_instrument_index: bool = ...) -> Self:
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
        """Anchor the trade to a clean price, excluding accrued interest.

Parameters
----------
value : float
    Clean price in the bond's quotation units, not a decimal interest rate.
    Use the same price convention as the instrument being valued.

Returns
-------
BondPricingAnchor
    Clean-price anchor; spread inversion is performed by the bond pricer.
"""
        ...

    @staticmethod
    def dirty_price(value: float) -> Any:
        """Anchor the trade to a dirty price, including accrued interest.

Parameters
----------
value : float
    Dirty price in the bond's quotation units, using the same price
    convention as the instrument being valued.

Returns
-------
BondPricingAnchor
    Dirty-price anchor; spread inversion is performed by the bond pricer.
"""
        ...

    @staticmethod
    def z_spread(value: float) -> Any:
        """Anchor a straight bond to a continuously compounded Z-spread.

Parameters
----------
value : float
    Spread in decimal annual rate units; 0.01 represents 100 basis points.

Returns
-------
BondPricingAnchor
    Z-spread anchor for straight-bond pricing.
"""
        ...

    @staticmethod
    def OAS(value: float) -> Any:
        """Anchor an issuer-call bond to an option-adjusted spread.

Parameters
----------
value : float
    Option-adjusted spread in decimal annual rate units; 0.01 represents
    100 basis points.

Returns
-------
BondPricingAnchor
    OAS anchor for pricing a bond with an issuer-call schedule.
"""
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> BondPricingAnchor:
        """Deserialize and validate a bond price or spread anchor.

Parameters
----------
obj : mapping
    Native externally tagged anchor, such as ``{"CleanPrice": 99.5}``,
    ``{"DirtyPrice": 100.0}``, ``{"ZSpread": 0.01}``, or ``{"OAS": 0.0075}``.
    Prices use the bond's quotation units; spreads are decimal annual rates.

Returns
-------
BondPricingAnchor
    Validated native anchor.

Raises
------
QuantSupportError
    If deserialization or native anchor validation fails.
"""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the native bond-anchor enum.

Returns
-------
dict[str, Any]
    Single tagged price/spread value, for example ``{"OAS": 0.0075}``.
    Serialization does not perform the extraction-time validation of from_dict().

Raises
------
QuantSupportError
    If serialization to Python objects fails.
"""
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
    Typed market observations, copied into the context. The store's reference
    date determines the context evaluation date.
curves : sequence of CurveConfiguration
    Typed bootstrap, explicit zero-rate, or composite-curve configurations.
    These are construction inputs, not already constructed DiscountCurve views.
fixings : FixingStore, optional
    Published historical index observations. Defaults to None, retaining the
    native context's empty fixing store.
fx : FxStore, optional
    Spot exchange rates. Defaults to None, retaining the native empty FX store.
volatility_surfaces : sequence of VolatilitySurfaceConfiguration, optional
    Surface construction inputs. None, the default, configures no surfaces.
volatility_cubes : sequence of VolatilityCubeConfiguration, optional
    Cube construction inputs. None, the default, configures no cubes.
pricer_configurations : sequence of PricerConfiguration, optional
    Instrument pricing models and implementations. None, the default, adds no
    instrument-specific configurations; pricers requiring one fail at evaluation.
simulations : sequence of SimulationConfiguration, optional
    Simulation construction inputs. None, the default, configures no simulations.
csa_policy : SingleCurveCSADiscountPolicy, optional
    Collateral currency and remuneration curve used for market construction and
    derivative discounting. None, the default, keeps USD collateral with SOFR
    remuneration; it does not infer a policy from the supplied curve list.
fixed_income_policy : FixedIncomeDiscountPolicy, optional
    Currency-to-curve discount policy. None, the default, maps the context base
    currency to its base curve. Straight bonds, notes, and deposits may prefer
    their instrument discount index; loans use the currency mapping.
quote_level : Level or str, optional
    Calibration quote level: Mid, Bid, or Ask, as an enum or case-insensitive
    variant name. None, the default, selects Level.Mid.
credit_curves : sequence of CreditCurveConfiguration, optional
    Survival-curve construction inputs. None, the default, configures no credit
    curves.

Notes
-----
Construction stores inputs without initializing the market. Enter the context
for lazy construction, or call initialize() to construct all configured objects.
The context is thread-confined. Scenario stacks and AD recording are local to
the current thread; enter and exit scopes on that same thread in reverse order.
Context exit releases constructed market data when its last matching scope ends.
Do not rely on AD-backed market views outside the scope that created them.

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

    def __new__(cls, quotes: QuoteStore, curves: Sequence[CurveConfiguration], fixings: FixingStore | None = ..., fx: FxStore | None = ..., volatility_surfaces: Sequence[VolatilitySurfaceConfiguration] | None = ..., volatility_cubes: Sequence[VolatilityCubeConfiguration] | None = ..., pricer_configurations: Sequence[PricerConfiguration] | None = ..., simulations: Sequence[SimulationConfiguration] | None = ..., csa_policy: SingleCurveCSADiscountPolicy | None = ..., fixed_income_policy: FixedIncomeDiscountPolicy | None = ..., quote_level: Level | str | None = ..., credit_curves: Sequence[CreditCurveConfiguration] | None = ...) -> Self:
        ...

    def initialize(self) -> None:
        """Constructs curves, volatility surfaces/cubes, and simulations.

Explicitly constructs every configured object. Context entry enables
lazy construction of requested market objects instead. Repeated calls do
nothing until market inputs are reassigned or the context is exited.

Raises
------
QuantSupportError
    If validation, calibration, or market construction fails.
"""
        ...

    def discount_curves(self) -> list[DiscountCurve]:
        """Resolve all configured discount curves in the active market view.

Returns
-------
list of DiscountCurve
    Curves sorted by market-index display name, constructed lazily if needed.
    Within a scenario these are the shocked views, not base-market replacements.

Raises
------
QuantSupportError
    If the context is uninitialized or curve construction fails.
"""
        ...

    @property
    def curves(self) -> list[CurveConfiguration]:
        """Curve construction inputs. Assign a new collection to rebuild the market."""
        ...

    @curves.setter
    def curves(self, value: Sequence[CurveConfiguration]) -> None:
        """Replace curve inputs and invalidate constructed market data.

        Parameters
        ----------
        value : Sequence[CurveConfiguration]
            New curve definitions; equivalent to assigning curve_configurations.
        """
        ...

    def curve(self, index: _IndexLike) -> DiscountCurve:
        """Resolve one discount curve in the active base or scenario market.

Parameters
----------
index : MarketIndex or str
    Curve's configured market index, as a typed index or native serialized name.

Returns
-------
DiscountCurve
    Requested curve view, constructed on demand. Use AD-backed views within
    the context or scenario lifetime that produced them.

Raises
------
QuantSupportError
    If the context is uninitialized, the index is invalid, construction fails,
    or no discount curve can be resolved for the index.
"""
        ...

    def volatility_surfaces(self) -> list[VolatilitySurface]:
        """Resolve all configured volatility surfaces in the active market view.

Returns
-------
list of VolatilitySurface
    Surfaces sorted by market-index display name, constructed lazily if needed.
    An empty configuration yields an empty list. Scenario scopes return shocked
    views; use AD-backed data within the originating scope.

Raises
------
QuantSupportError
    If the context is uninitialized or market construction fails.
"""
        ...

    def volatility_surface(self, index: _IndexLike) -> VolatilitySurface:
        """Resolve one volatility surface in the active market view.

Parameters
----------
index : MarketIndex or str
    Surface's configured market index, as a typed index or serialized name.

Returns
-------
VolatilitySurface
    Surface constructed on demand from the active base or scenario inputs.
    Use its AD-backed data within the originating context or scenario scope.

Raises
------
QuantSupportError
    If the context is uninitialized, index conversion or construction fails,
    or no surface is available for the index.
"""
        ...

    def volatility_cubes(self) -> list[VolatilityCube]:
        """Resolve all configured volatility cubes in the active market view.

Returns
-------
list of VolatilityCube
    Cubes sorted by market-index display name, constructed lazily if needed.
    An empty configuration yields an empty list. Scenario scopes return shocked
    views; use AD-backed data within the originating scope.

Raises
------
QuantSupportError
    If the context is uninitialized or market construction fails.
"""
        ...

    def volatility_cube(self, index: _IndexLike) -> VolatilityCube:
        """Resolve one volatility cube in the active market view.

Parameters
----------
index : MarketIndex or str
    Cube's configured market index, as a typed index or serialized name.

Returns
-------
VolatilityCube
    Cube constructed on demand from the active base or scenario inputs.
    Use its AD-backed data within the originating context or scenario scope.

Raises
------
QuantSupportError
    If the context is uninitialized, index conversion or construction fails,
    or no cube is available for the index.
"""
        ...

    def simulations(self) -> list[Simulation]:
        """Resolve all configured simulations in the active market view.

Returns
-------
list of Simulation
    Simulations sorted by market-index display name, constructed on demand
    from active inputs. An empty configuration yields an empty list.

Raises
------
QuantSupportError
    If the context is uninitialized or market construction fails.
"""
        ...

    def simulation(self, index: _IndexLike) -> Simulation:
        """Resolve one simulation in the active market view.

Parameters
----------
index : MarketIndex or str
    Simulation's configured market index, as a typed index or serialized name.

Returns
-------
Simulation
    Requested simulation, constructed on demand from active market inputs.

Raises
------
QuantSupportError
    If the context is uninitialized, index conversion or construction fails,
    or no simulation is available for the index.
"""
        ...

    def register_pricer(self, trade_type: Any, pricer: Any) -> Any:
        """Registers (or replaces) a Python pricer for a Python trade class.

The most specific exact type is selected first; registered base classes
also handle their subclasses. Python registrations take precedence over
built-in dispatch, which makes built-in trade classes overridable too.

Parameters
----------
trade_type : type
    Python class of positions to handle, not an instrument instance. Exact
    types and then registered bases in the trade's MRO are considered first;
    virtual-subclass matches fall back to the newest matching registration.
pricer : object
    Object exposing callable ``evaluate(trade, requests, context)`` and returning
    EvaluationResults. Inheritance from Pricer is not required. Replaces any
    existing registration for the exact same trade_type.

Returns
-------
None
    Registration is stored on this context only.

Raises
------
QuantSupportError
    If pricer has no callable evaluate attribute.
"""
        ...

    def unregister_pricer(self, trade_type: Any) -> Any:
        """Remove the Python pricer registered for an exact trade class.

Parameters
----------
trade_type : type
    Exact Python class whose registration is removed. Registrations for its
    bases and subclasses are left unchanged.

Returns
-------
bool
    True if a registration was removed; False if none existed for that class.
"""
        ...

    def pricer_for(self, trade: Trade) -> Any:
        """Returns the Python pricer selected for a trade, or `None` when built-in
dispatch will be used.

Parameters
----------
trade : Trade
    Position instance whose Python type, MRO, and virtual-subclass matches are
    checked against this context's registrations.

Returns
-------
object or None
    Closest registered Python pricer. None means no Python override, not that
    native pricing of this instrument is necessarily supported.
"""
        ...

    def __enter__(self) -> Self:
        """Activate this thread's context scope and validate market inputs.

Returns
-------
PricingContext
    This context, ready for request-driven market construction. The first
    active context on the thread starts forward AD recording.

Raises
------
QuantSupportError
    If native context validation fails.
"""
        ...

    def __exit__(self, _exc_type: Any | None = ..., _exc_value: Any | None = ..., _traceback: Any | None = ...) -> bool:
        """Leave a context scope without suppressing an exception.

Parameters
----------
_exc_type : type or None, optional
    Exception class supplied by the with statement; defaults to None. Ignored.
_exc_value : BaseException or None, optional
    Exception instance supplied by the with statement; defaults to None. Ignored.
_traceback : traceback or None, optional
    Exception traceback supplied by the with statement; defaults to None. Ignored.

Returns
-------
bool
    Always False. After this context's last matching scope exits, its constructed
    data are cleared and it becomes uninitialized. When no context remains on
    the thread, AD recording stops and its tape is rewound.

Raises
------
RuntimeError
    If active pricing contexts are exited out of reverse entry order.
"""
        ...

    def evaluate_script(self, script: ScriptedProduct | Mapping[str, Any], n_paths: int = ..., seed: int = ...) -> dict[str, float]:
        """Evaluate dated payoff code using native LGM market models.

Parameters
----------
script : ScriptedProduct or mapping
    Dated payoff product or its native serialized mapping, including currency,
    events, and result variable. A bare source-code string is not accepted.
    An explicit discount_index is required outside the context base currency.
    Product model assignments override context simulation and pricer assignments.
n_paths : int, optional
    Positive Monte Carlo path count. Defaults to 1000.
seed : int, optional
    Unsigned 64-bit random seed. Defaults to 42.

Returns
-------
dict[str, float]
    Named numeric results returned by the native script engine.

Notes
-----
This bridge returns values, not cashflow or sensitivity tables. Native
cashflow diagnostics accumulate raw sampled payments, while pilot-fitted
martingale controls adjust only the selected result variable. At finite paths
their summed present values need not equal that reported variable, and an
arbitrary selected numeric variable need not represent payments at all.

Raises
------
ValueError
    If n_paths is zero.
QuantSupportError
    If the context is uninitialized, product decoding fails, discount/model
    dependencies are unavailable, mean reversion is invalid, or execution fails.
"""
        ...

    def evaluate(self, trade: Trade, requests: Sequence[Request | str] | None = ...) -> EvaluationResults:
        """Evaluate a position against the active base or scenario market.

Parameters
----------
trade : Trade
    Position wrapping a supported native instrument, including direction and
    position-size overrides. A bare instrument is not accepted by native
    dispatch. A registered Python pricer can instead handle its registered
    Python trade types and takes precedence over native pricing.
requests : sequence of Request or str, optional
    Requested outputs: Value, YieldToMaturity, ModifiedDuration, Sensitivities,
    ZSpread, OAS, Cashflows, or FairRate, as enums or case-insensitive names.
    None, the default, requests only Value. An explicit empty sequence is not
    replaced by Value. Supported requests depend on the instrument and pricer.

Returns
-------
EvaluationResults
    As-of-date results with optional scalar, sensitivity, and cashflow fields.
    Native results are copied out of AD state. Sensitivities are raw derivatives
    per unit pillar change, not pre-scaled basis-point changes. Python pricers
    receive ``evaluate(trade, requests, context)`` with typed Request values and
    must return EvaluationResults.

Raises
------
TypeError
    If native dispatch receives a bare instrument rather than a Trade.
ValueError
    If a native bond position requires a pricing anchor but none was supplied.
QuantSupportError
    If the context is uninitialized, a request is invalid or unsupported,
    market data or required model configuration are missing, no native pricer
    supports the instrument, valuation fails, or a Python pricer returns the
    wrong result type. Exceptions raised by a Python pricer propagate unchanged.
"""
        ...

    def run_xva(self, config: Any, netting_sets: Any) -> XvaResult:
        """Run native exposure simulation and XVA on the active market view.

Parameters
----------
config : XvaConfig
    Typed engine configuration specifying rate models, paths, random seed,
    and simulation-grid frequency. Credit, funding, and collateral
    assumptions belong to each netting set's CsaTerms, not this configuration.
netting_sets : sequence of NettingSet
    Typed client portfolios with distinct names and their own CSA terms.
    Only instruments supported by NettingSet's native claim conversion are
    accepted; Python pricer registrations are not used for this conversion.

Returns
-------
XvaResult
    Copied exposure profiles and pandas-backed XVA and sensitivity tables.
    Simulation uses the active scenario when one is entered on this thread.
    Profiles report pre-margin exposures; enabled variation margin affects the
    XVA aggregation, not those profiles. Adjustments are discounted to present
    value using the system/base curve.

Raises
------
QuantSupportError
    If the context is uninitialized, netting-set names are duplicated, claim
    conversion fails, model or market dependencies are missing, or engine
    construction or execution fails.
"""
        ...

    @property
    def curve_configurations(self) -> list[CurveConfiguration]:
        """Copied curve inputs, including bootstrap, explicit-rate, and composite definitions.

Assign a new sequence to replace the inputs and invalidate constructed market
data. Mutating the returned list does not update this context. Alias: curves.
"""
        ...

    @curve_configurations.setter
    def curve_configurations(self, value: Sequence[CurveConfiguration]) -> None:
        """Replace curve inputs and invalidate constructed market data.

        Parameters
        ----------
        value : Sequence[CurveConfiguration]
            New bootstrap, explicit-rate, or composite curve definitions.
        """
        ...

    @property
    def simulation_configurations(self) -> list[SimulationConfiguration]:
        """Copied simulation inputs. Assign a new sequence to replace them.

Reassignment invalidates constructed market data; editing the copy does not.
"""
        ...

    @simulation_configurations.setter
    def simulation_configurations(self, value: Sequence[SimulationConfiguration]) -> None:
        """Replace simulation inputs and invalidate constructed market data.

        Parameters
        ----------
        value : Sequence[SimulationConfiguration]
            New simulation definitions to construct on the next initialization.
        """
        ...

    @property
    def reference_date(self) -> Date:
        """Reference date of the underlying quote store."""
        ...

    @property
    def quotes(self) -> QuoteStore:
        """Copied quotes from the active base or scenario market view.

Before initialization, returns base inputs. Assign a QuoteStore to replace
the base quotes and evaluation date and invalidate constructed market data;
editing a returned snapshot does not update the context or its scenarios.
"""
        ...

    @quotes.setter
    def quotes(self, value: QuoteStore) -> None:
        """Replace base quotes and invalidate constructed market data.

        Parameters
        ----------
        value : QuoteStore
            New base quote store, including the evaluation reference date.
        """
        ...

    @property
    def csa_policy(self) -> SingleCurveCSADiscountPolicy:
        """Collateral policy for market construction and derivative discounting.

Assign a SingleCurveCSADiscountPolicy to replace the base currency/index and
invalidate constructed market data. The initial default is USD/SOFR.
"""
        ...

    @csa_policy.setter
    def csa_policy(self, value: SingleCurveCSADiscountPolicy) -> None:
        """Replace collateral policy and invalidate constructed market data.

        Parameters
        ----------
        value : SingleCurveCSADiscountPolicy
            New base collateral currency and remuneration index policy.
        """
        ...

    @property
    def volatility_surface_configurations(self) -> list[VolatilitySurfaceConfiguration]:
        """Copied volatility-surface inputs. Assign a new sequence to replace them.

Reassignment invalidates constructed market data; editing the copy does not.
"""
        ...

    @volatility_surface_configurations.setter
    def volatility_surface_configurations(self, value: Sequence[VolatilitySurfaceConfiguration]) -> None:
        """Replace surface inputs and invalidate constructed market data.

        Parameters
        ----------
        value : Sequence[VolatilitySurfaceConfiguration]
            New volatility surface definitions for subsequent initialization.
        """
        ...

    @property
    def volatility_cube_configurations(self) -> list[VolatilityCubeConfiguration]:
        """Copied volatility-cube inputs. Assign a new sequence to replace them.

Reassignment invalidates constructed market data; editing the copy does not.
"""
        ...

    @volatility_cube_configurations.setter
    def volatility_cube_configurations(self, value: Sequence[VolatilityCubeConfiguration]) -> None:
        """Replace cube inputs and invalidate constructed market data.

        Parameters
        ----------
        value : Sequence[VolatilityCubeConfiguration]
            New volatility cube definitions for subsequent initialization.
        """
        ...

    @property
    def credit_curve_configurations(self) -> list[CreditCurveConfiguration]:
        """Copied survival-curve bootstrap inputs. Assign a new sequence to replace them.

Reassignment invalidates constructed market data; editing the copy does not.
"""
        ...

    @credit_curve_configurations.setter
    def credit_curve_configurations(self, value: Sequence[CreditCurveConfiguration]) -> None:
        """Replace survival-curve inputs and invalidate constructed market data.

        Parameters
        ----------
        value : Sequence[CreditCurveConfiguration]
            New credit-curve bootstrap definitions for subsequent initialization.
        """
        ...

    @property
    def evaluation_date(self) -> Date:
        """Alias for `reference_date`, matching the `MarketDataProvider` contract."""
        ...

    @property
    def snapshot_timestamp(self) -> DateTime | None:
        """Selected observation instant, or None after market inputs are changed."""
        ...

    @property
    def base_currency(self) -> Currency:
        """Base collateral currency used for curve construction."""
        ...

    @property
    def quote_level(self) -> Level:
        """Construction/calibration quote level, initially Level.Mid.

Assign Level.Mid, Level.Bid, Level.Ask, or a case-insensitive variant name to
invalidate constructed market data and use that level on subsequent requests.
"""
        ...

    @quote_level.setter
    def quote_level(self, value: Level | str) -> None:
        """Replace construction quote level and invalidate constructed market data.

        Parameters
        ----------
        value : Level or str
            Mid, Bid, or Ask enum or case-insensitive variant name.
        """
        ...

    @property
    def pricer_configurations(self) -> list[PricerConfiguration]:
        """Copied instrument-level pricer selections and typed model assignments.

Assign a new sequence to replace selections and invalidate constructed market
data. Editing the returned list does not update the context.
"""
        ...

    @pricer_configurations.setter
    def pricer_configurations(self, value: Sequence[PricerConfiguration]) -> None:
        """Replace pricing selections and invalidate constructed market data.

        Parameters
        ----------
        value : Sequence[PricerConfiguration]
            New instrument-level pricer and typed model assignments.
        """
        ...

    @property
    def base_index(self) -> MarketIndex:
        """Base collateral/remuneration index used by the context."""
        ...

    @property
    def fixings(self) -> FixingStore:
        """Copied historical fixing store for native and Python pricers.

Assign a FixingStore to replace observations and invalidate constructed market
data. Editing the returned store does not update this context.
"""
        ...

    @fixings.setter
    def fixings(self, value: FixingStore) -> None:
        """Replace historical fixings and invalidate constructed market data.

        Parameters
        ----------
        value : FixingStore
            New historical observations supplied to native and Python pricers.
        """
        ...

    @property
    def fx(self) -> FxStore:
        """Copied FX spot store for native and Python pricers.

Assign an FxStore to replace spot rates and invalidate constructed market data.
Editing the returned store does not update this context.
"""
        ...

    @fx.setter
    def fx(self, value: FxStore) -> None:
        """Replace FX spots and invalidate constructed market data.

        Parameters
        ----------
        value : FxStore
            New currency-pair spot rates supplied to native and Python pricers.
        """
        ...

    @property
    def fixed_income_policy(self) -> FixedIncomeDiscountPolicy | None:
        """Fixed-income discount override, or None for the context base-curve policy.

Assign a FixedIncomeDiscountPolicy or None to change subsequent fixed-income
valuations. Unlike market-input reassignment, this does not rebuild market data.
"""
        ...

    @fixed_income_policy.setter
    def fixed_income_policy(self, value: FixedIncomeDiscountPolicy | None) -> None:
        """Replace the discount override without rebuilding constructed market data.

        Parameters
        ----------
        value : FixedIncomeDiscountPolicy or None
            New fixed-income discount override; None restores the base-curve policy.
        """
        ...


@final
class LoanPricer:
    """Native loan valuation with refinancing and discount-policy overrides.

Parameters
----------
refinancing_benchmark_index : MarketIndex or str, optional
    Benchmark index for refinancing-incentive prepayment. Its metadata supplies
    observation tenor and quote convention; it excludes replacement-loan lending
    spreads and fees. None, the default, supplies no override, so refinancing-
    incentive prepayment requires an explicitly configured benchmark.
discount_policy : FixedIncomeDiscountPolicy, optional
    Loan discount-curve override. None, the default, uses the context's fixed-
    income policy, which by default maps its base currency to its base curve.
rebuild_amortization_for_fair_rate : bool, optional
    Defaults to False, preserving contractual principal in FairRate trials.
    True rebuilds amortization for each candidate from its repayment structure
    and initial all-in rate. Neither setting changes the original loan or the
    principal used for Value and Sensitivities.
"""

    def __new__(cls, *, refinancing_benchmark_index: _IndexLike | None = ..., discount_policy: FixedIncomeDiscountPolicy | None = ..., rebuild_amortization_for_fair_rate: bool = ...) -> Self:
        """Configure refinancing/discount overrides and whether FairRate trials rebuild amortization."""
        ...

    def evaluate(self, trade: Trade, context: PricingContext, requests: Sequence[Request | str] | None = ...) -> EvaluationResults:
        """Value a loan's signed expected receipts, excluding its initial advance.

Parameters
----------
trade : Trade
    Position wrapping a Loan. A notional override rebuilds the contractual
    principal schedule at that amount; side controls receipt direction.
context : PricingContext
    Initialized native context, entered on its owning thread or initialized
    explicitly. Market inputs are resolved from its active scenario view.
requests : sequence of Request or str, optional
    Value, Cashflows, Sensitivities, or FairRate, as typed requests or names.
    None, the default, selects Value. Sensitivities are derivatives per unit
    market-pillar change. FairRate solves an annual fixed coupon or floating
    spread at or before origination over [-0.05, 0.50]; mixed, stepped, and
    bounded coupon schedules are unsupported.

Returns
-------
EvaluationResults
    Signed receipt PV, raw pillar derivatives, annual decimal fair rate, and/or
    expected future receipts. Cashflow leg indices 0, 1, and 2 identify scheduled
    payments, prepayment, and recovery respectively.

Raises
------
TypeError
    If trade does not wrap a Loan.
QuantSupportError
    If the context is uninitialized, observations or projection terms are
    unavailable/invalid, a request is unsupported, or the fair-rate solve fails.
"""
        ...


@disjoint_base
class Trade:
    """Generic position in an instrument.

This mirrors the Rust `Trade` contract while remaining subclassable so a
Python trade can carry any product-specific state it needs.

Parameters
----------
instrument : Instrument
    Contract held by the position. Python objects may also be carried for
    registered Python pricers; native evaluation requires a supported product.
trade_date : Date, datetime.date, or str
    Execution date; strings use ISO ``YYYY-MM-DD`` format.
side : Side or str, optional
    Direction, defaulting to Side.LongReceive when None. Strings accept
    LongReceive, Long, Receive, PayShort, Pay, or Short, case-insensitively.
    Use side, not a negative amount, to reverse a position.
notional : float, optional
    Finite non-negative position amount in the instrument's notional currency.
    None, the default, leaves the contractual amount unchanged.
domestic_notional : float, optional
    Finite non-negative domestic-leg amount in domestic currency units. None,
    the default, leaves the instrument's domestic amount unchanged.
foreign_notional : float, optional
    Finite non-negative foreign-leg amount in foreign currency units. None,
    the default, leaves the instrument's foreign amount unchanged.
num_contracts : float, optional
    Finite non-negative futures position size in contracts. Defaults to 1.0.
pricing_anchor : BondPricingAnchor or mapping, optional
    Bond price/spread anchor, as a typed anchor or its native externally tagged
    mapping. None, the default, supplies no anchor; native bond pricing requires
    one. Prices use the bond's units and spreads use decimal annual rates.
settlement_date : Date, datetime.date, or str, optional
    Bond settlement-date override; strings use ISO ``YYYY-MM-DD``. None, the
    default, leaves settlement selection to the instrument's native pricer.
spread_day_counter : DayCounter or str, optional
    Bond spread day-count override, as an enum or case-insensitive variant
    name. None, the default, leaves the native spread convention unchanged.

Raises
------
ValueError
    If a position amount or num_contracts is negative or non-finite.
QuantSupportError
    If a date, direction, day counter, or pricing anchor cannot be converted
    or the supplied anchor fails native validation.

Notes
-----
Positions retain a reference to the instrument and are thread-confined.
"""

    def __new__(cls, instrument: Instrument, trade_date: _DateLike, side: Side | str | None = ..., *, notional: float | None = ..., domestic_notional: float | None = ..., foreign_notional: float | None = ..., num_contracts: float = ..., pricing_anchor: Any | None = ..., settlement_date: _DateLike | None = ..., spread_day_counter: Any | None = ...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    def with_pricing_anchor(self, anchor: Any) -> Any:
        """Copy the native position fields with a replacement bond anchor.

Parameters
----------
anchor : BondPricingAnchor or mapping
    New typed bond anchor or native tagged mapping, for example
    ``{"ZSpread": 0.01}``. None is not a request to remove the anchor.

Returns
-------
Trade
    New base Trade sharing the original instrument and preserving dates,
    direction, and size. Python-subclass attributes are not copied.

Raises
------
QuantSupportError
    If anchor decoding or native validation fails.
"""
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
by built-in pricers.

Notes
-----
The native base object is thread-confined and initially has no discount policy.
"""

    def __new__(cls) -> Self:
        ...

    def evaluate(self, _trade: Any, _requests: Any, _context: Any) -> EvaluationResults:
        """Evaluate a Python trade; subclasses must implement this operation.

Parameters
----------
_trade : object
    Registered Python trade instance supplied unchanged by the dispatcher.
_requests : sequence of Request
    Typed output requests supplied by the dispatcher, already converted from
    names and with its default Value request applied when requests was None.
_context : object
    Market-data provider supplied by the caller, normally an initialized
    PricingContext. Query only the provider operations the implementation needs.

Returns
-------
EvaluationResults
    Required result type for an override; the dispatcher validates this type.

Raises
------
NotImplementedError
    Always raised by the base implementation.
"""
        ...

    def market_data_request(self, _trade: Any) -> Any:
        """Describes required market data, if the implementation uses an explicit
request object. Python pricers may also query the context directly.

Parameters
----------
_trade : object
    Position whose market dependencies a subclass may describe.

Returns
-------
object or None
    Base implementation returns None. The Python dispatcher calls evaluate()
    directly and does not consume this optional dependency description.
"""
        ...

    def set_discount_policy(self, policy: Any) -> Any:
        """Store a discount-policy object for a Python pricer's own use.

Parameters
----------
policy : object
    Arbitrary Python policy retained by reference and exposed by discount_policy.
    No policy interface is validated and native context policies are unaffected.

Returns
-------
None
    Replaces the stored policy; the base pricer initially has no policy.
"""
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
ordinary Python objects can be used with the standalone `Evaluator`.
The native base object is thread-confined; it supplies no market data itself.
"""

    def __new__(cls) -> Self:
        ...

    @property
    def evaluation_date(self) -> Date:
        """Reference date supplied by a concrete provider.

Raises
------
NotImplementedError
    Always raised by the base property; a concrete provider must implement it.
"""
        ...


@final
class Evaluator:
    """Runtime dispatcher for heterogeneous Python trade collections.

Registrations are keyed by Python trade type, matching the role of the
Rust `Evaluator`'s `TypeId -> ErasedPricer` map. Each evaluator starts with an
empty registry and is thread-confined. Registrations retain their classes and
pricer objects; len(evaluator) counts exact registered classes, not trades.
"""

    def __repr__(self) -> str:
        ...

    def __len__(self) -> int:
        ...

    def register(self, trade_type: Any, pricer: Any) -> Any:
        """Associate a Python trade class with a pricer object.

Parameters
----------
trade_type : type
    Exact Python position class to register. Matching follows the trade's MRO,
    then virtual-subclass matches, with the newest virtual match preferred.
pricer : object
    Object with callable ``evaluate(trade, requests, context)`` returning
    EvaluationResults. Replaces the registration for the exact same class;
    inheritance from Pricer is not required.

Returns
-------
None
    Registration is stored on this evaluator.

Raises
------
QuantSupportError
    If pricer has no callable evaluate attribute.
"""
        ...

    def unregister(self, trade_type: Any) -> Any:
        """Remove a registration without altering related classes.

Parameters
----------
trade_type : type
    Exact Python position class whose registration is to be removed.

Returns
-------
bool
    True if a registration was removed, otherwise False.
"""
        ...

    def pricer_for(self, trade: Trade) -> Any:
        """Look up the closest registered pricer for a position.

Parameters
----------
trade : Trade
    Position instance to match against registered classes, its MRO first and
    then virtual-subclass registrations.

Returns
-------
object or None
    Selected pricer, or None when no class matches. This evaluator has no
    native-pricer fallback.
"""
        ...

    def evaluate(self, trade: Trade, context: Any, requests: Sequence[Request | str] | None = ...) -> Any:
        """Evaluate a registered Python trade against a market-data provider.

Parameters
----------
trade : Trade
    Position instance handled by a registered Python pricer. Any registered
    Python trade type can be dispatched; native instrument dispatch is not used.
context : object
    Provider passed unchanged to the pricer. A PricingContext must already be
    initialized and used on its owning thread. Other Python providers need only
    expose the operations consumed by the selected pricer.
requests : sequence of Request or str, optional
    Output requests as enums or names accepted by Request.parse(). None, the
    default, selects Value. An empty sequence remains empty. The pricer receives
    a list of typed Request values as its second argument, before context.

Returns
-------
EvaluationResults
    Result returned by the selected Python pricer, checked by the dispatcher.

Raises
------
QuantSupportError
    If no pricer matches, requests cannot be parsed, a supplied PricingContext
    is uninitialized, or the pricer returns something other than EvaluationResults.
    Exceptions raised by the Python pricer itself propagate unchanged.
"""
        ...


@disjoint_base
class EvaluationResults:
    """Detached valuation outputs, also constructible by Python pricers.

Parameters
----------
evaluation_date : Date, datetime.date, or str
    As-of date of the evaluation; strings use ISO ``YYYY-MM-DD`` format.
identifier : str
    Instrument identifier used to label this result.
price : float, optional
    Signed present value in the pricer's valuation currency. Defaults to None;
    the constructor stores the supplied scalar without computing a value.
yield_to_maturity : float, optional
    Yield in decimal annual rate units under the pricer's convention. Defaults
    to None. The ytm property aliases this field.
modified_duration : float, optional
    Modified duration under the pricer's yield/time convention. Defaults to None.
z_spread : float, optional
    Continuously compounded Z-spread in decimal annual rate units; 0.01 is
    100 basis points. Defaults to None.
oas : float, optional
    Option-adjusted spread in decimal annual rate units. Defaults to None.
fair_rate : float, optional
    Par coupon or spread in decimal annual rate units, as defined by the pricer.
    Defaults to None.
sensitivities : mapping or sequence of (str, float), optional
    Raw pillar labels and exposures, normally derivatives per unit input change.
    Pair sequences preserve order and duplicates; mappings are sorted by label.
    None, the default, means no sensitivity output. The DataFrame property sums
    duplicate labels and sorts them; raw_sensitivities retains the stored pairs.
cashflows : sequence of mappings, optional
    Rows requiring string ``payment_date``, ``type`` (or ``cashflow_type``),
    numeric ``amount``, and string ``currency``. Optional ``fixing``,
    ``caplet_strike``, and ``floorlet_strike`` default to None; ``accrual_period``
    and ``leg_index`` default to 0.0 and 0. Amounts are in the row's currency,
    rates/strikes are decimal rates, and accrual periods are year fractions.
    None, the default, means no cashflow output; an empty list is distinct.

Raises
------
QuantSupportError
    If the date, sensitivities, or cashflow rows cannot be converted.

Notes
-----
Scalar fields are stored as supplied; no consistency checks or valuation are
performed. Table properties construct new pandas DataFrames on access.
"""

    def __new__(cls, evaluation_date: _DateLike, identifier: str, *, price: Any | None = ..., yield_to_maturity: Any | None = ..., modified_duration: Any | None = ..., z_spread: Any | None = ..., oas: Any | None = ..., fair_rate: Any | None = ..., sensitivities: Any | None = ..., cashflows: Any | None = ...) -> Self:
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
        """Return aggregated pillar derivatives as a fresh pandas table.

Returns
-------
pandas.DataFrame or None
    Columns ``pillar`` (string label) and ``value`` (raw exposure per unit input
    change), with duplicates summed and rows sorted by pillar. None means no
    sensitivity output; supplied empty sensitivities produce an empty frame.

Raises
------
QuantSupportError
    If pandas is unavailable. Pandas construction errors propagate unchanged.
"""
        ...

    @property
    def cashflows(self) -> DataFrame | None:
        """Return contractual or expected cashflow rows as a fresh pandas table.

Returns
-------
pandas.DataFrame or None
    Columns ``payment_date``, ``type``, ``amount``, ``fixing``, ``accrual_period``,
    ``currency``, ``caplet_strike``, ``floorlet_strike``, and ``leg_index``.
    Dates and currency codes are strings; amounts use the row currency, rates
    use decimal units, and accrual_period is a year fraction. Optional observations
    may be missing. Row order is preserved. None means no cashflow output;
    supplied empty rows produce an empty frame with the same columns.

Raises
------
QuantSupportError
    If pandas is unavailable. Pandas construction errors propagate unchanged.
"""
        ...

    @property
    def price(self) -> float | None:
        """Net present value, if `"Value"` was requested."""
        ...

    @property
    def raw_sensitivities(self) -> Any:
        """Return copied raw (pillar_label, exposure) pairs, or None if absent.

Stored ordering and duplicate labels are preserved. Exposures are not scaled
to basis points; the aggregated sensitivities property sums duplicate labels.
"""
        ...

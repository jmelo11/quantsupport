"""Market scenarios, collateral agreements, exposures, and XVA."""
from datetime import date as PythonDate
from typing import Any, ClassVar, Mapping, Sequence, final, overload
from typing_extensions import Self, disjoint_base
from pandas import DataFrame
from ..market import _CurrencyLike, _IndexLike
__all__ = ['ScenarioType', 'Shock', 'Scenario', 'XvaConfig', 'CsaTerms', 'NettingSet', 'XvaResult', 'ExposureProfile']

@final
class ScenarioType:
    """How a scenario shock is applied to a quote value."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> ScenarioType:
        """Parse the interpretation of a market shock.

Parameters
----------
s : str
    ``Absolute`` or ``Relative``, matched case-insensitively.

Returns
-------
ScenarioType
    Absolute for an additive shock or Relative for a fractional shock.

Raises
------
QuantSupportError
    If s is not a recognized scenario-type name.
"""
        ...
    Absolute: ClassVar[ScenarioType]
    Relative: ClassVar[ScenarioType]

@final
class Shock:
    """A market shock selecting quote identifiers or explicit zero-rate pillars.

Parameters
----------
target : str
    Nonempty exact pillar identifier or underscore-separated selector. For example,
    EURIBOR12m selects its deposit and swap quotes; FUNDINGSPREAD selects
    every explicit zero-rate pillar of that funding-spread curve.
value : float
    Finite additive change in the selected input's units for Absolute shocks
    (for decimal interest rates, 0.01 is 100 basis points), or fractional change
    for Relative shocks (0.10 increases inputs by 10%). No default is supplied.
shock_type : ScenarioType
    Required typed ScenarioType.Absolute or ScenarioType.Relative; strings are
    not accepted by this constructor. Absolute adds value to the input; Relative
    multiplies it by 1 + value.

Raises
------
ValueError
    If target is empty or value is not finite.

Notes
-----
Immutable. Combine shocks in Scenario and enter it after a PricingContext."""

    def __new__(cls, *, target: str, value: float, shock_type: ScenarioType) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @property
    def target(self) -> str:
        """Exact pillar identifier or underscore-separated selector."""
        ...

    @property
    def value(self) -> float:
        """Additive decimal change or fractional relative change."""
        ...

    @property
    def shock_type(self) -> ScenarioType:
        """Interpretation of the shock value."""
        ...

@final
class Scenario:
    """A combination of temporary market shocks used in ``with context, scenario:``.

Parameters
----------
shocks : sequence of Shock
    Typed market shocks, applied in their supplied order. Each Shock supplies
    a target selector, value and ScenarioType. Required keyword argument;
    an empty sequence is allowed and adds no shocks of its own.

Notes
-----
Enter after a PricingContext. Nested scenarios compose shocks in entry order.
The base context retains its market inputs and constructed objects. Requested
curves and volatility markets have independent AD state. Leaving the scope,
including through an exception, discards the temporary view. Reusable.
The Scenario object and active-scope stacks are thread-confined; enter and
exit on the owning thread, exiting before the enclosing PricingContext.
Do not retain AD-backed shocked market objects for use after scenario exit.
"""

    def __new__(cls, *, shocks: Sequence[Shock]) -> Self:
        ...

    @property
    def shocks(self) -> list[Shock]:
        """Read-only ordered shocks, returned as a fresh list of shock copies."""
        ...

    def __enter__(self) -> Self:
        """Activate shocks on the innermost context entered on this thread.

Returns
-------
Scenario
    This scenario. It retains the selected context until exit, validates the
    combined shocks, and constructs an independent market cache on demand.
    Distinct nested scenarios compose in entry order without mutating base inputs.

Raises
------
RuntimeError
    If no PricingContext is active on this thread, or this Scenario object is
    already active. Calling context.initialize() alone does not enter a scope.
QuantSupportError
    If the selected context is uninitialized or the shocks fail native validation.
"""
        ...

    def __exit__(self, _exc_type: Any=..., _exc_value: Any=..., _traceback: Any=...) -> bool:
        """Discard the temporary market view and release its context reference.

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
    Always False, so exceptions propagate. The prior scenario or base view is
    restored and this scenario may be entered again after exit.

Raises
------
RuntimeError
    If this scenario is inactive or scenarios exit out of reverse entry order.
"""
        ...

@final
class XvaConfig:
    """XVA engine configuration (models, paths, seed, frequency).

Credit, funding, and collateral parameters belong to each client's CsaTerms,
not this configuration. Load this object through from_dict() or from_json().
Model calibration and engine-level checks occur when PricingContext.run_xva()
constructs and runs the engine, not in these deserializing factories.
"""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> XvaConfig:
        """Deserialize the native exposure-engine configuration.

Parameters
----------
obj : mapping
    Required fields are ``model_configs`` (index-keyed native LGM rate-model
    configurations), ``n_paths`` (Monte Carlo count), ``seed`` (unsigned 64-bit
    Owen-scrambling seed), and ``frequency`` (native Frequency variant, such as
    ``"Monthly"``). These have no serde defaults. ``fx_configs`` defaults to an
    empty list; each supplied FX entry requires ``foreign_currency`` and
    annual decimal ``fx_vol``, with correlation ``rho`` defaulting to 0.0.
    Use an even n_paths for antithetic partners. Model/index variants use native
    serde representations; unknown top-level fields are rejected.
    Each rate-model entry requires market_index; unless a driver index is used,
    supply lambda (mean reversion in inverse years) and parameter_source.
    Fixed Gaussian sigma is absolute annual rate volatility: 0.01 means
    100 basis points per square-root year. A driver reuses another curve's
    stochastic factor instead of providing independent model parameters.

Returns
-------
XvaConfig
    Deserialized simulation/model setup, without constructing market data.

Raises
------
QuantSupportError
    If required fields are absent or the configuration cannot be deserialized.
"""
        ...

    @staticmethod
    def from_json(path: str) -> XvaConfig:
        """Load one exposure-engine configuration from a JSON file.

Parameters
----------
path : str
    Absolute filesystem path or path relative to the current working directory.
    The UTF-8 file must contain one object in the from_dict() schema, not a list
    or a wrapped configuration. The argument is a path, not inline JSON text.

Returns
-------
XvaConfig
    Deserialized engine configuration; model calibration is deferred to run_xva().

Raises
------
QuantSupportError
    If the file cannot be read, contains invalid JSON, or fails schema decoding.
"""
        ...

@final
class CsaTerms:
    """Per-client collateral, credit, and funding assumptions for XVA.

Parameters
----------
collateral_index : MarketIndex or str
    Required collateral remuneration index, as a typed index or native serialized
    name. Other-currency trades use collateral-adjusted discount curves.
collateral_currency : Currency or str
    Required currency of collateral balances, as an enum or ISO alphabetic code.
credit_spread : float
    Required flat counterparty credit spread in decimal annual rate units;
    0.01 is 100 basis points. Used for CVA only when credit_index is absent.
recovery : float
    Required counterparty recovery fraction; 0.4 represents 40 percent.
funding_spread : float, optional
    Flat FVA funding spread in decimal annual rate units. Defaults to 0.0.
    Ignored if funding_spread_curve or funding_index is supplied.
credit_index : MarketIndex or str, optional
    Counterparty credit-curve index, default None. When present, CVA uses its
    survival probabilities and CDS-quote sensitivities instead of credit_spread.
    A data-carrying index may be supplied as a typed MarketIndex or native tagged
    mapping, for example ``{"Credit": "CLIENT-A"}``, not its display string.
funding_spread_curve : ZeroRateTermStructure or mapping, optional
    Reusable decimal funding-spread term structure, default None. The binding
    also accepts a TenorBasedZeroRateTermStructure. A native mapping carries
    its reference date, dates, rates, and rate/interpolation conventions.
    Replaces the flat spread; with funding_index it is an additive spread
    overlay on the curve-implied basis rather than a replacement discount curve.
funding_index : MarketIndex or str, optional
    Bootstrapped funding discount-curve index, default None. FVA uses forward
    spreads of this curve over the engine system/base curve at simulation
    dates, plus any funding_spread_curve overlay. Replaces funding_spread.
variation_margin : bool, optional
    Defaults to False for uncollateralized exposure. True enables margin calls
    at each simulation date using the thresholds and minimum transfer amount.
counterparty_threshold : float, optional
    Finite non-negative unsecured positive exposure allowed before the
    counterparty posts collateral, in collateral currency units. Defaults to 0.0.
own_threshold : float, optional
    Finite non-negative unsecured negative-exposure allowance before this party
    posts collateral, in collateral currency units. Defaults to 0.0.
minimum_transfer_amount : float, optional
    Finite non-negative minimum absolute change in collateral balance triggering
    a transfer, in collateral currency units. Equality triggers a transfer.
    Defaults to 0.0. Thresholds and this amount are validated even with margin off.

Raises
------
QuantSupportError
    If an index, currency, or curve cannot be converted, or a margin threshold
    or transfer minimum is negative or non-finite.
"""

    def __new__(cls, collateral_index: _IndexLike, collateral_currency: _CurrencyLike, credit_spread: Any, recovery: Any, funding_spread: float=..., credit_index: _IndexLike | None=..., funding_spread_curve: Any | None=..., funding_index: Any | None=..., *, variation_margin: bool=..., counterparty_threshold: float=..., own_threshold: float=..., minimum_transfer_amount: float=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> CsaTerms:
        """Deserialize and validate one client's CSA terms.

Parameters
----------
obj : mapping
    Native CSA fields. ``collateral_index``, ``collateral_currency``,
    ``credit_spread``, and ``recovery`` are required. Funding spread, thresholds,
    and minimum transfer default to 0.0; variation_margin defaults to False;
    credit_index, funding_index, and funding_spread_curve default to None.
    Financial units and funding precedence match the class constructor.
    Native tagged index representations are required for data-carrying variants,
    for example ``{"Credit": "CLIENT-A"}``; typed enums and curves are normalized.

Returns
-------
CsaTerms
    CSA terms with finite non-negative margin thresholds and transfer minimum.

Raises
------
QuantSupportError
    If decoding fails or margin parameter validation fails.
"""
        ...

    @staticmethod
    def from_json(path: str) -> CsaTerms:
        """Load and validate one client's CSA terms from a JSON file.

Parameters
----------
path : str
    Absolute filesystem path or path relative to the current working directory.
    The UTF-8 file must contain a single object in the from_dict() schema, using
    native serde values, not a list, display-only index strings, or inline JSON
    passed as the path argument.

Returns
-------
CsaTerms
    Decoded terms after margin parameter validation.

Raises
------
QuantSupportError
    If file reading, JSON decoding, CSA deserialization, or margin validation fails.
"""
        ...

    @property
    def own_threshold(self) -> Any:
        """Read-only unsecured exposure allowed to this party, in collateral currency units."""
        ...

    @property
    def collateral_currency(self) -> Any:
        """Read-only currency of collateral balances and margin thresholds."""
        ...

    @property
    def minimum_transfer_amount(self) -> Any:
        """Read-only minimum margin transfer size, in collateral currency units."""
        ...

    @property
    def recovery(self) -> Any:
        """Read-only counterparty recovery fraction; 0.4 represents 40 percent."""
        ...

    @property
    def collateral_index(self) -> Any:
        """Read-only market index of the collateral remuneration curve."""
        ...

    @property
    def counterparty_threshold(self) -> Any:
        """Read-only unsecured exposure allowed to the counterparty, in collateral currency units."""
        ...

    @property
    def funding_spread(self) -> Any:
        """Read-only flat funding spread in decimal rate units; 0.01 is 100 basis points."""
        ...

    @property
    def funding_spread_curve(self) -> Any:
        """The reusable zero-rate spread curve, or `None` when absent."""
        ...

    @property
    def credit_index(self) -> Any:
        """Read-only counterparty credit-curve index, or None when not configured."""
        ...

    @property
    def funding_index(self) -> Any:
        """Read-only funding-curve market index, or None when not configured."""
        ...

    @property
    def variation_margin(self) -> Any:
        """Read-only flag enabling variation margin with the configured thresholds and transfer minimum."""
        ...

    @property
    def credit_spread(self) -> Any:
        """Read-only flat counterparty credit spread in decimal rate units."""
        ...

@final
class NettingSet:
    """A group of trades under a single netting agreement (one client),
carrying the client's CsaTerms.

Parameters
----------
name : str
    Agreement identifier used to label exposure profiles and XVA results.
    Must be unique among netting sets in one PricingContext.run_xva() call;
    duplicate names are rejected there, not by this constructor.
trades : iterable of Trade
    Positions, not bare instruments, converted to native contingent claims.
    Supported instruments are Swap, CrossCurrencySwap, BasisSwap,
    FixFloatCrossCurrencySwap, FixedRateBond, FloatingRateNote, and
    FixedRateDeposit. Trade references are retained; generators are consumed.
csa : CsaTerms
    Typed collateral, credit, and funding terms, copied into the netting set.

Raises
------
TypeError
    If an element is not a Trade, or trades is not iterable.
QuantSupportError
    If a position's instrument is unsupported or native claim conversion fails.
"""

    def __new__(cls, name: str, trades: Any, csa: Any) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    def __len__(self) -> int:
        ...

    @property
    def name(self) -> str:
        """Read-only netting-agreement identifier used to label XVA results."""
        ...

@final
class XvaResult:
    """Result of an XVA engine run."""

    def __repr__(self) -> str:
        ...

    @property
    def exposures(self) -> Any:
        """Return copied per-netting-set exposure profiles.

Returns
-------
list of ExposureProfile
    One profile per returned exposure cube, labeled by its netting_set name.
    Each profile can be converted to a fresh table with to_dataframe().
    Profiles use pre-margin NPVs even when the XVA calculation enables margin.
"""
        ...

    @property
    def xva_values(self) -> Any:
        """Return XVA measures as a fresh pandas table.

Returns
-------
pandas.DataFrame
    Columns ``netting_set`` (agreement name), ``measure`` (native adjustment
    label, such as CVA or FVA), and ``value`` (monetary present value in the
    engine's base currency, discounted using the system/base curve).
    Missing native XVA values produce
    an empty frame with these columns, not None. Engine row order is preserved.

Raises
------
QuantSupportError
    If pandas is unavailable. Pandas construction errors propagate unchanged.
"""
        ...

    @property
    def sensitivities(self) -> DataFrame | None:
        """Return raw XVA derivatives as a fresh pandas table.

Returns
-------
pandas.DataFrame
    Columns ``parameter`` and ``value``, sorted lexicographically by parameter.
    Aggregator inputs are prefixed with the netting-set name, for example
    ``"clientA.CVA.credit_spread"``. Values are derivatives per unit parameter
    change, not basis-point-scaled. The runtime returns an empty frame when no
    sensitivities are present, not None, despite the retained optional annotation.

Raises
------
QuantSupportError
    If pandas is unavailable. Pandas construction errors propagate unchanged.
"""
        ...

@final
class ExposureProfile:
    """Per-netting-set, pre-margin exposure means by simulation date.

EPE averages positive NPVs, ENE averages negative NPVs with their negative
sign, and EE averages all NPVs. These are values as of each future simulation
date, before any variation-margin adjustment, not discounted XVA measures.
"""

    def __repr__(self) -> str:
        ...

    def to_dataframe(self) -> DataFrame:
        """Copy the exposure time series into a pandas table.

Returns
-------
pandas.DataFrame
    Columns ``date`` (native date strings), ``epe`` (expected positive
    exposure), ``ene`` (expected negative exposure), and ``ee`` (expected
    exposure). Values are in the engine's base currency, as of each simulation
    date and before variation margin. ENE is non-positive; EE is the unconditional
    mean and equals EPE + ENE. Rows preserve simulation date order. Every call constructs
    a new frame; an empty profile yields the same columns with no rows.

Raises
------
QuantSupportError
    If pandas is unavailable. Pandas construction errors propagate unchanged.
"""
        ...

    @property
    def netting_set(self) -> Any:
        """Read-only identifier of the netting set represented by this profile."""
        ...

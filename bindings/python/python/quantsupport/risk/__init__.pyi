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
        """Parses a scenario type from its name."""
        ...
    Absolute: ClassVar[ScenarioType]
    Relative: ClassVar[ScenarioType]

@final
class Shock:
    """A market shock selecting quote identifiers or explicit zero-rate pillars.

Parameters
----------
target : str
    Exact pillar identifier or underscore-separated selector. For example,
    EURIBOR12m selects its deposit and swap quotes; FUNDINGSPREAD selects
    every explicit zero-rate pillar of that funding-spread curve.
value : float
    Additive decimal change for Absolute shocks (0.01 is 100 basis points),
    or fractional change for Relative shocks (0.10 increases inputs by 10%).
shock_type : ScenarioType
    Absolute or Relative interpretation of the value.

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
    a target selector, value and ScenarioType.

Notes
-----
Enter after a PricingContext. Nested scenarios compose shocks in entry order.
The base context retains its market inputs and constructed objects. Requested
curves and volatility markets have independent AD state. Leaving the scope,
including through an exception, discards the temporary view. Reusable."""

    def __new__(cls, *, shocks: Sequence[Shock]) -> Self:
        ...

    @property
    def shocks(self) -> list[Shock]:
        """Read-only ordered shocks, returned as a fresh list of shock copies."""
        ...

    def __enter__(self) -> Self:
        ...

    def __exit__(self, _exc_type: Any=..., _exc_value: Any=..., _traceback: Any=...) -> bool:
        ...

@final
class XvaConfig:
    """XVA engine configuration (models, paths, seed, frequency).

Credit, funding and collateral parameters are **not** part of this
config — they are per client, see [`CsaTerms`]."""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> XvaConfig:
        """Builds the configuration from a dict."""
        ...

    @staticmethod
    def from_json(path: str) -> XvaConfig:
        """Loads the configuration from a JSON file."""
        ...

@final
class CsaTerms:
    """Per-client CSA terms: collateral treatment and credit/funding parameters."""

    def __new__(cls, collateral_index: _IndexLike, collateral_currency: _CurrencyLike, credit_spread: Any, recovery: Any, funding_spread: float=..., credit_index: _IndexLike | None=..., funding_spread_curve: Any | None=..., funding_index: Any | None=..., *, variation_margin: bool=..., counterparty_threshold: float=..., own_threshold: float=..., minimum_transfer_amount: float=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> CsaTerms:
        """Builds CSA terms from a dict."""
        ...

    @staticmethod
    def from_json(path: str) -> CsaTerms:
        """Loads CSA terms from a JSON file."""
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
carrying the client's [`CsaTerms`]."""

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
        """Per-netting-set exposure profiles."""
        ...

    @property
    def xva_values(self) -> Any:
        """XVA values as a DataFrame with columns netting_set/measure/value."""
        ...

    @property
    def sensitivities(self) -> DataFrame | None:
        """Sensitivities as a DataFrame with columns parameter/value.
Aggregator parameters are prefixed with the netting-set name
(e.g. `"clientA.CVA.credit_spread"`)."""
        ...

@final
class ExposureProfile:
    """Per-netting-set exposure profile (EPE / ENE / EE by date)."""

    def __repr__(self) -> str:
        ...

    def to_dataframe(self) -> DataFrame:
        """Exposure profile as a DataFrame with columns date/epe/ene/ee."""
        ...

    @property
    def netting_set(self) -> Any:
        """Read-only identifier of the netting set represented by this profile."""
        ...

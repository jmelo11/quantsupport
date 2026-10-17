"""Observable market data, construction inputs, curves, volatility, and simulations."""
from datetime import date as PythonDate
from typing import Any, ClassVar, Mapping, Sequence, final, overload
from typing_extensions import Self, disjoint_base
from pandas import DataFrame
from ..time import Compounding, Date, DayCounter, Frequency, Period, _DateLike, _PeriodLike

_CurrencyLike = Currency | str
_IndexLike = MarketIndex | str | Mapping[str, Any]

__all__ = ['Currency', 'MarketIndex', 'Level', 'Interpolator', 'VolatilityType', 'SmileType', 'QuoteStore', 'FixingStore', 'FxStore', 'CurveConstruction', 'CurveConfiguration', 'CreditCurveConfiguration', 'VolatilitySurfaceConfiguration', 'VolatilityCubeConfiguration', 'SimulationConfiguration', 'ZeroRateTermStructure', 'TenorBasedZeroRateTermStructure', 'DiscountCurve', 'VolatilitySurface', 'VolatilityCube', 'Simulation']

@final
class Level:
    """Market quote level used for construction and calibration."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Level:
        """Parses a quote level from its name (case-insensitive)."""
        ...
    Mid: ClassVar[Level]
    Bid: ClassVar[Level]
    Ask: ClassVar[Level]

@final
class Currency:
    """ISO currency."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Currency:
        """Parses a currency from its alphabetic code (e.g. `"USD"`)."""
        ...

    @property
    def code(self) -> str:
        """Alphabetic ISO 4217 code."""
        ...

    @property
    def name(self) -> str:
        """Display name (e.g. `"US Dollar"`)."""
        ...

    @property
    def numeric_code(self) -> int:
        """Numeric ISO 4217 code."""
        ...

    @property
    def symbol(self) -> str:
        """Currency symbol (e.g. `"$"`)."""
        ...

    @property
    def precision(self) -> int:
        """Decimal precision."""
        ...
    USD: ClassVar[Currency]
    EUR: ClassVar[Currency]
    JPY: ClassVar[Currency]
    ZAR: ClassVar[Currency]
    CLP: ClassVar[Currency]
    CLF: ClassVar[Currency]
    CHF: ClassVar[Currency]
    BRL: ClassVar[Currency]
    COP: ClassVar[Currency]
    MXN: ClassVar[Currency]
    AUD: ClassVar[Currency]
    CAD: ClassVar[Currency]
    CNY: ClassVar[Currency]
    GBP: ClassVar[Currency]
    NZD: ClassVar[Currency]
    NOK: ClassVar[Currency]
    SEK: ClassVar[Currency]
    PEN: ClassVar[Currency]
    CNH: ClassVar[Currency]
    INR: ClassVar[Currency]
    TWD: ClassVar[Currency]
    HKD: ClassVar[Currency]
    KRW: ClassVar[Currency]
    DKK: ClassVar[Currency]
    IDR: ClassVar[Currency]

@final
class MarketIndex:
    """A market index: rate indices (SOFR, ESTR, ...), equity underlyings, FX
pairs, collateral curves and custom indices.

Plain rate indices are available as class constants
(`MarketIndex.SOFR`); parameterised variants are built with
`MarketIndex.equity`, `MarketIndex.fx_pair`,
`MarketIndex.collateral` and `MarketIndex.other`."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def equity(name: str) -> MarketIndex:
        """An equity underlying identified by name."""
        ...

    @staticmethod
    def fx_pair(base: _CurrencyLike, quote: _CurrencyLike) -> MarketIndex:
        """A directed FX spot pair (1 base = x quote)."""
        ...

    @staticmethod
    def collateral(currency: _CurrencyLike, collateral_currency: _CurrencyLike) -> MarketIndex:
        """Collateral discount curve for cashflows in `currency` posted under a
CSA denominated in `collateral_currency`."""
        ...

    @staticmethod
    def credit(name: str) -> MarketIndex:
        """Credit curve of a reference entity, identified by name."""
        ...

    @staticmethod
    def other(name: str) -> MarketIndex:
        """A custom index identified by name."""
        ...

    @staticmethod
    def parse(s: str) -> MarketIndex:
        """Parses a plain rate index from its name (e.g. `"SOFR"`),
case-insensitively."""
        ...
    SOFR: ClassVar[MarketIndex]
    SOFRCompounded: ClassVar[MarketIndex]
    TermSOFR1m: ClassVar[MarketIndex]
    TermSOFR3m: ClassVar[MarketIndex]
    TermSOFR6m: ClassVar[MarketIndex]
    TermSOFR12m: ClassVar[MarketIndex]
    ESTR: ClassVar[MarketIndex]
    EURIBOR1m: ClassVar[MarketIndex]
    EURIBOR3m: ClassVar[MarketIndex]
    EURIBOR6m: ClassVar[MarketIndex]
    EURIBOR12m: ClassVar[MarketIndex]
    SONIA: ClassVar[MarketIndex]
    TONAR: ClassVar[MarketIndex]
    TIBOR3m: ClassVar[MarketIndex]
    TIBOR6m: ClassVar[MarketIndex]
    SARON: ClassVar[MarketIndex]
    CORRA: ClassVar[MarketIndex]
    AONIA: ClassVar[MarketIndex]
    NZONIA: ClassVar[MarketIndex]
    NOWA: ClassVar[MarketIndex]
    SWESTR: ClassVar[MarketIndex]
    ICP: ClassVar[MarketIndex]
    VIX: ClassVar[MarketIndex]

@final
class Interpolator:
    """Interpolation rule for curve pillars."""
    Linear: ClassVar[Interpolator]
    LogLinear: ClassVar[Interpolator]
    CubicSpline: ClassVar[Interpolator]

    @staticmethod
    def parse(s: str) -> Interpolator:
        """Parses an interpolation rule by its name."""
        ...

    def __str__(self) -> str:
        ...

    def __repr__(self) -> str:
        ...

@final
class VolatilityType:
    """Quotation convention of a volatility."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> VolatilityType:
        """Parses a volatility type from its name."""
        ...
    Black: ClassVar[VolatilityType]
    Normal: ClassVar[VolatilityType]

@final
class SmileType:
    """Smile axis of a volatility surface or cube."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> SmileType:
        """Parses a smile type from its name."""
        ...
    Strike: ClassVar[SmileType]
    Delta: ClassVar[SmileType]
    LogMoneyness: ClassVar[SmileType]

@final
class QuoteStore:
    """Market quotes with a reference date.

Created from `{"reference_date": "YYYY-MM-DD", "quotes": [{"identifier": ..., "mid": ...}]}`."""

    def __repr__(self) -> str:
        ...

    def __len__(self) -> int:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> QuoteStore:
        """Builds a quote store from a dict."""
        ...

    @staticmethod
    def from_json(path: str) -> QuoteStore:
        """Loads a quote store from a JSON file."""
        ...

    def identifiers(self) -> list[str]:
        """Sorted quote identifiers."""
        ...

    def value(self, identifier: str, level: str=...) -> float:
        """Returns one quote level by identifier (`"mid"`, `"bid"`, or `"ask"`)."""
        ...

    def levels(self, identifier: str) -> Any:
        """Returns `(mid, bid, ask)` for one quote identifier."""
        ...

    def to_dataframe(self) -> DataFrame:
        """All quotes as a DataFrame with columns `identifier`, `mid`, `bid`,
`ask`, sorted by identifier."""
        ...

    @property
    def reference_date(self) -> Date:
        """Reference date."""
        ...

@final
class FixingStore:
    """Historical index fixings."""

    def __new__(cls) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> FixingStore:
        """Builds a fixing store from a dict."""
        ...

    @staticmethod
    def from_json(path: str) -> FixingStore:
        """Loads a fixing store from a JSON file."""
        ...

    def add(self, index: _IndexLike, date: _DateLike, value: float) -> None:
        """Adds or replaces a fixing."""
        ...

    def fixing(self, index: _IndexLike, date: _DateLike) -> float:
        """Returns one historical fixing."""
        ...

    def to_dataframe(self, index: _IndexLike) -> DataFrame:
        """All fixings for an index as a DataFrame with `date` and `value` columns."""
        ...

@final
class FxStore:
    """FX spot rates. Records are `{"base": "CLP", "quote": "USD", "rate": 0.00111}`,
meaning 1 base = rate quote."""

    def __new__(cls) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> FxStore:
        """Builds an FX store from a list of rate records."""
        ...

    @staticmethod
    def from_json(path: str) -> FxStore:
        """Loads FX rate records from a JSON file (a list of records)."""
        ...

    def add(self, base: Any, quote: Any, rate: float) -> None:
        """Adds a spot rate: 1 `base` = `rate` `quote`."""
        ...

    def rate(self, base: Any, quote: Any) -> float:
        """Returns a direct, inverse, or triangulated FX rate."""
        ...

@final
class CurveConstruction:
    """Native curve construction rule: calibration, zero rates, or composition."""

    @staticmethod
    def bootstrapped() -> CurveConstruction:
        """Calibrate the curve to the quote identifiers in CurveConfiguration.quotes."""
        ...

    @staticmethod
    def zero_rate(*, pillars: Sequence[_DateLike | _PeriodLike], rates: Sequence[float], compounding: Compounding | str | None=..., frequency: Frequency | str | None=...) -> CurveConstruction:
        """Interpolate annual decimal rates at dates or reference-date tenors."""
        ...

    @staticmethod
    def composite(*, base_index: _IndexLike, spread_index: _IndexLike) -> CurveConstruction:
        """Multiply benchmark and spread discount factors."""
        ...

@final
class CurveConfiguration:
    """Wrapper around the library's `QsCurveConfiguration`.
Define a curve for construction by PricingContext.

Parameters
----------
market_index : MarketIndex or str
    Identity used to request the curve for projection or discounting.
quotes : sequence of str, optional
    Calibration quote identifiers. Used with bootstrapped construction.
construction : CurveConstruction, optional
    Bootstrapped, explicit zero-rate, or composite construction rule.
    Defaults to bootstrapped construction.
day_counter : DayCounter or str, optional
    Curve time basis. Defaults to Actual360.
interpolator : Interpolator or str, optional
    Linear, LogLinear, or CubicSpline. Defaults to LogLinear.
enable_extrapolation : bool, optional
    Allow evaluation beyond the supplied pillars. Defaults to True."""

    def __new__(cls, *, market_index: _IndexLike, quotes: Sequence[str] | None=..., construction: CurveConstruction | None=..., day_counter: DayCounter | str | None=..., interpolator: Interpolator | str | None=..., enable_extrapolation: bool=...) -> Self:
        """Configure a curve; construction defaults to calibration from quotes."""
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> CurveConfiguration:
        """Builds a single configuration from a dict."""
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[CurveConfiguration]:
        """Builds configurations from a list, single dict, or wrapped list."""
        ...

    @staticmethod
    def from_json(path: str) -> list[CurveConfiguration]:
        """Loads configurations from a JSON file. Accepts a list, a
single object, or an object wrapping a single list (e.g.
`{"curve_specs": [...]}`). Always returns a list."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns the configuration as a dict."""
        ...

@final
class CreditCurveConfiguration:
    """Wrapper around the library's `QsCreditCurveConfiguration`."""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> CreditCurveConfiguration:
        """Builds a single configuration from a dict."""
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[CreditCurveConfiguration]:
        """Builds configurations from a list, single dict, or wrapped list."""
        ...

    @staticmethod
    def from_json(path: str) -> list[CreditCurveConfiguration]:
        """Loads configurations from a JSON file. Accepts a list, a
single object, or an object wrapping a single list (e.g.
`{"curve_specs": [...]}`). Always returns a list."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns the configuration as a dict."""
        ...

@final
class VolatilitySurfaceConfiguration:
    """Wrapper around the library's `QsVolatilitySurfaceConfiguration`."""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> VolatilitySurfaceConfiguration:
        """Builds a single configuration from a dict."""
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[VolatilitySurfaceConfiguration]:
        """Builds configurations from a list, single dict, or wrapped list."""
        ...

    @staticmethod
    def from_json(path: str) -> list[VolatilitySurfaceConfiguration]:
        """Loads configurations from a JSON file. Accepts a list, a
single object, or an object wrapping a single list (e.g.
`{"curve_specs": [...]}`). Always returns a list."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns the configuration as a dict."""
        ...

@final
class VolatilityCubeConfiguration:
    """Wrapper around the library's `QsVolatilityCubeConfiguration`."""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> VolatilityCubeConfiguration:
        """Builds a single configuration from a dict."""
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[VolatilityCubeConfiguration]:
        """Builds configurations from a list, single dict, or wrapped list."""
        ...

    @staticmethod
    def from_json(path: str) -> list[VolatilityCubeConfiguration]:
        """Loads configurations from a JSON file. Accepts a list, a
single object, or an object wrapping a single list (e.g.
`{"curve_specs": [...]}`). Always returns a list."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns the configuration as a dict."""
        ...

@final
class SimulationConfiguration:
    """Wrapper around the library's `QsSimulationConfiguration`."""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> SimulationConfiguration:
        """Builds a single configuration from a dict."""
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[SimulationConfiguration]:
        """Builds configurations from a list, single dict, or wrapped list."""
        ...

    @staticmethod
    def from_json(path: str) -> list[SimulationConfiguration]:
        """Loads configurations from a JSON file. Accepts a list, a
single object, or an object wrapping a single list (e.g.
`{"curve_specs": [...]}`). Always returns a list."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns the configuration as a dict."""
        ...

@final
class ZeroRateTermStructure:
    """A zero-rate curve with dated pillars."""

    def __new__(cls, reference_date: _DateLike, dates: Sequence[_DateLike], rates: Sequence[float], *, day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., frequency: Frequency | str | None=..., interpolator: Interpolator | str | None=..., enable_extrapolation: bool=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> ZeroRateTermStructure:
        """Construct from a mapping in the native QuantSupport serialization format."""
        ...

    @staticmethod
    def from_json(path: str) -> ZeroRateTermStructure:
        """Load from a JSON file path using the native QuantSupport schema."""
        ...

    def zero_rate(self, date: _DateLike) -> float:
        """Interpolated decimal zero rate at a date, under the curve's rate conventions."""
        ...

    def zero_rate_from_time(self, time: float) -> float:
        """Interpolated decimal zero rate at a year fraction from the reference date."""
        ...

    def discount_factor(self, date: _DateLike) -> float:
        """Discount factor from the reference date to the supplied date."""
        ...

    def discount_factor_from_time(self, time: float) -> float:
        """Discount factor at a year fraction from the reference date."""
        ...

    def forward_rate(self, start_date: _DateLike, end_date: _DateLike, compounding: Compounding | str | None=..., frequency: Frequency | str | None=...) -> float:
        """Decimal forward rate between dates; omitted conventions use the curve's compounding and frequency."""
        ...

    def forward_rate_from_time(self, start: float, end: float) -> float:
        """Forward rate between two year fractions measured from the reference date."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Exports the equivalent dated curve."""
        ...

    @property
    def reference_date(self) -> Date:
        """Read-only origin for curve times and discount factors."""
        ...

    @property
    def enable_extrapolation(self) -> bool:
        """Read-only flag permitting queries outside the pillar range."""
        ...

    @property
    def compounding(self) -> Compounding:
        """Read-only compounding convention for the curve's zero rates."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only convention converting dates to year fractions from the reference date."""
        ...

    @property
    def dates(self) -> list[Date]:
        """Read-only pillar dates, returned as a fresh list."""
        ...

    @property
    def interpolator(self) -> Interpolator:
        """Read-only interpolation method applied to zero-rate pillars."""
        ...

    @property
    def frequency(self) -> Frequency:
        """Read-only compounding frequency for the curve's zero rates."""
        ...

    @property
    def rates(self) -> list[float]:
        """Read-only decimal zero rates in pillar-date order, returned as a fresh list."""
        ...

@final
class TenorBasedZeroRateTermStructure:
    """A zero-rate curve with tenor pillars."""

    def __new__(cls, reference_date: _DateLike, tenors: Sequence[_PeriodLike], rates: Sequence[float], *, day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., frequency: Frequency | str | None=..., interpolator: Interpolator | str | None=..., enable_extrapolation: bool=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    def zero_rate(self, date: _DateLike) -> float:
        """Interpolated decimal zero rate at a date, under the curve's rate conventions."""
        ...

    def zero_rate_from_time(self, time: float) -> float:
        """Interpolated decimal zero rate at a year fraction from the reference date."""
        ...

    def discount_factor(self, date: _DateLike) -> float:
        """Discount factor from the reference date to the supplied date."""
        ...

    def discount_factor_from_time(self, time: float) -> float:
        """Discount factor at a year fraction from the reference date."""
        ...

    def forward_rate(self, start_date: _DateLike, end_date: _DateLike, compounding: Compounding | str | None=..., frequency: Frequency | str | None=...) -> float:
        """Decimal forward rate between dates; omitted conventions use the curve's compounding and frequency."""
        ...

    def forward_rate_from_time(self, start: float, end: float) -> float:
        """Forward rate between two year fractions measured from the reference date."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Exports the equivalent dated curve."""
        ...

    @property
    def dates(self) -> list[Date]:
        """Read-only pillar dates resolved from the tenors, returned as a fresh list."""
        ...

    @property
    def enable_extrapolation(self) -> bool:
        """Read-only flag permitting queries outside the pillar range."""
        ...

    @property
    def reference_date(self) -> Date:
        """Read-only origin from which pillar tenors and curve times are measured."""
        ...

    @property
    def frequency(self) -> Frequency:
        """Read-only compounding frequency for the curve's zero rates."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only convention converting dates to year fractions from the reference date."""
        ...

    @property
    def interpolator(self) -> Interpolator:
        """Read-only interpolation method applied to zero-rate pillars."""
        ...

    @property
    def rates(self) -> list[float]:
        """Read-only decimal zero rates in pillar order, returned as a fresh list."""
        ...

    @property
    def tenors(self) -> list[Period]:
        """Read-only pillar offsets from the reference date, returned as a fresh list."""
        ...

    @property
    def compounding(self) -> Compounding:
        """Read-only compounding convention for the curve's zero rates."""
        ...

@final
class DiscountCurve:
    """A discount curve constructed by the pricing context."""

    def __repr__(self) -> str:
        ...

    def discount_factor(self, date: _DateLike) -> float:
        """Discount factor at a date."""
        ...

    def forward_rate(self, start: _DateLike, end: _DateLike, compounding: Compounding | str | None=..., frequency: Frequency | str | None=...) -> float:
        """Forward rate between two dates (default Simple / Annual)."""
        ...

    def nodes(self) -> DataFrame:
        """Curve nodes as a DataFrame with columns `date` and `discount_factor`."""
        ...

    def pillars(self) -> DataFrame:
        """Curve pillars (bootstrap instruments) as a DataFrame with columns
`label` and `value`."""
        ...

    @property
    def reference_date(self) -> Date:
        """Curve reference date."""
        ...

    @property
    def day_counter(self) -> DayCounter | None:
        """Day count convention of the curve, if available."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Market index the curve discounts."""
        ...

@final
class VolatilitySurface:
    """A constructed volatility surface."""

    def __repr__(self) -> str:
        ...

    def volatility(self, expiry: Any, key: float) -> float:
        """Volatility at an expiry (`Date`, ISO string, `Period` or period
string) and smile coordinate `key`."""
        ...

    def pillars(self) -> DataFrame:
        """Surface pillars as a DataFrame with columns `label` and `value`."""
        ...

    @property
    def smile_type(self) -> Any:
        """Smile axis (Strike, Delta or LogMoneyness)."""
        ...

    @property
    def volatility_type(self) -> Any:
        """Volatility quotation convention (Black or Normal)."""
        ...

    @property
    def reference_date(self) -> Date:
        """Surface reference date."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Underlying market index."""
        ...

@final
class VolatilityCube:
    """A constructed volatility cube (expiry x underlying maturity x smile)."""

    def __repr__(self) -> str:
        ...

    def volatility(self, expiry: Any, maturity: _PeriodLike, key: float) -> float:
        """Volatility at an expiry (`Date` or `Period`), underlying maturity
(`Period`) and smile coordinate `key`."""
        ...

    def pillars(self) -> DataFrame:
        """Cube pillars as a DataFrame with columns `label` and `value`."""
        ...

    @property
    def smile_type(self) -> Any:
        """Smile axis (Strike, Delta or LogMoneyness)."""
        ...

    @property
    def volatility_type(self) -> Any:
        """Volatility quotation convention (Black or Normal)."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Underlying market index."""
        ...

    @property
    def reference_date(self) -> Date:
        """Cube reference date."""
        ...

@final
class Simulation:
    """A constructed Monte Carlo simulation."""

    def __repr__(self) -> str:
        ...

    def calibration_diagnostics(self) -> DataFrame | None:
        """Hull-White calibration records as a DataFrame, or `None` for an uncalibrated model.
Columns include market/model prices, sigma and the signed price error."""
        ...

    def dates(self) -> list[Date]:
        """Simulation dates."""
        ...

    def paths(self) -> DataFrame:
        """Simulated paths as a DataFrame: one row per path, one column per
simulation date (ISO strings)."""
        ...

    @property
    def dt(self) -> float:
        """Time step between simulation dates, in years."""
        ...

    @property
    def n_paths(self) -> int:
        """Number of simulated paths."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Simulated market index."""
        ...

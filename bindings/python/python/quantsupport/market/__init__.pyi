"""Observable market data, construction inputs, curves, volatility, and simulations."""
from datetime import date as PythonDate
from typing import Any, ClassVar, Mapping, Sequence, final, overload
from typing_extensions import Self, disjoint_base
from pandas import DataFrame
from ..time import Compounding, Date, DateTime, DayCounter, Frequency, Period, _DateLike, _DateTimeLike, _PeriodLike
import quantsupport.pricing as _pricing

_CurrencyLike = Currency | str
_IndexLike = MarketIndex | str | Mapping[str, Any]

__all__ = ['SnapshotSelectionRule', 'SnapshotSelector', 'MarketSnapshot', 'MarketSnapshotStore', 'Currency', 'MarketIndex', 'Level', 'Interpolator', 'VolatilityType', 'SmileType', 'QuoteStore', 'FixingStore', 'FxStore', 'CurveConfiguration', 'CreditCurveConfiguration', 'VolatilitySurfaceConfiguration', 'VolatilityCubeConfiguration', 'SimulationConfiguration', 'ZeroRateTermStructure', 'TenorBasedZeroRateTermStructure', 'DiscountCurve', 'VolatilitySurface', 'VolatilityCube', 'Simulation']

@final
class SnapshotSelectionRule:
    """Timestamp matching: Exact, Closest, or Previous (never selects future data).

    Closest can select later data. Equidistant timestamps prefer the earlier one.
    """
    Exact: ClassVar[SnapshotSelectionRule]
    Closest: ClassVar[SnapshotSelectionRule]
    Previous: ClassVar[SnapshotSelectionRule]

@final
class SnapshotSelector:
    """Select one coherent historical market and calibration quote side.

    Parameters
    ----------
    timestamp : DateTime, datetime.datetime, or str
        Requested instant. External datetimes and RFC 3339 strings need offsets.
    rule : SnapshotSelectionRule or str, optional
        Exact, Closest, or Previous. Defaults to Previous.
    level : Level or str, optional
        Mid, Bid, or Ask. Defaults to Mid; missing consumed sides are errors.
    """
    def __new__(cls, timestamp: _DateTimeLike, *, rule: SnapshotSelectionRule | str | None = ..., level: Level | str | None = ...) -> Self: ...
    @property
    def timestamp(self) -> DateTime:
        """Requested UTC instant as a library DateTime."""
        ...
    @property
    def rule(self) -> SnapshotSelectionRule:
        """Timestamp matching policy."""
        ...
    @property
    def level(self) -> Level:
        """Quote side used for construction and model calibration."""
        ...

@final
class MarketSnapshot:
    """Timestamped market inputs, including curve construction configurations.

    Curves are rebuilt from quote identifiers or composite component indices.
    Runtime AD objects, fixings, temporary scenarios, Python pricer registrations,
    and Python fixed-income policies are not archived. FX spots are side-independent;
    zero-rate and discount-factor quotes use the selected side. Use from_context or from_dict to construct.
    """
    @staticmethod
    def from_context(timestamp: _DateTimeLike, context: _pricing.PricingContext) -> MarketSnapshot:
        """Capture the context's base inputs, excluding temporary scenarios.

        Parameters
        ----------
        timestamp : DateTime, datetime.datetime, or str
            Offset-aware observation instant, distinct from the business date.
        context : PricingContext
            Market whose serializable inputs should be captured. Externally
            supplied runtime market objects raise QuantSupportError.
        """
        ...
    @staticmethod
    def from_dict(value: Mapping[str, Any]) -> MarketSnapshot:
        """Restore one native serialized market snapshot.

        Parameters
        ----------
        value : mapping
            Native schema returned by to_dict; timestamp also accepts DateTime.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """Native serialized inputs; timestamp is an RFC 3339 string."""
        ...
    @property
    def timestamp(self) -> DateTime:
        """Actual observation instant as a library DateTime."""
        ...
    @property
    def reference_date(self) -> Date:
        """Market business date used as the pricing evaluation date."""
        ...
    @property
    def quotes(self) -> QuoteStore:
        """Copy of the market's quote observations."""
        ...

@final
class MarketSnapshotStore:
    """Archive coherent historical markets by DateTime with JSON persistence."""
    def __new__(cls) -> Self: ...
    def insert(self, snapshot: MarketSnapshot) -> None:
        """Insert without overwriting; duplicate instants are errors.

        Parameters
        ----------
        snapshot : MarketSnapshot
            Timestamped set of market inputs.
        """
        ...
    @property
    def timestamps(self) -> list[DateTime]:
        """Available instants in ascending order, as library DateTime values."""
        ...
    def select(self, selector: SnapshotSelector) -> MarketSnapshot:
        """Resolve one complete market or raise QuantSupportError.

        Parameters
        ----------
        selector : SnapshotSelector
            Requested observation instant and matching rule. Inspect the returned
            snapshot's timestamp to see the actual selected instant.
        """
        ...
    def pricing_context(self, selector: SnapshotSelector, *, fixings: FixingStore | None = ..., fixed_income_policy: _pricing.FixedIncomeDiscountPolicy | None = ...) -> _pricing.PricingContext:
        """Create a fresh context using the matched market's business date.

        Parameters
        ----------
        selector : SnapshotSelector
            Requested timestamp, matching rule, and quote side.
        fixings : FixingStore or None, optional
            Historical fixing dataset. Publication vintages are not filtered.
        fixed_income_policy : FixedIncomeDiscountPolicy or None, optional
            Explicit policy for the new context; otherwise uses its base curve.

        Notes
        -----
        Context construction is lazy. Dates and sides never share built curves
        or AD state. The requested instant does not roll the matched curves to
        a different evaluation date. Runtime provenance is snapshot_timestamp.
        """
        ...
    @staticmethod
    def from_json(path: str) -> MarketSnapshotStore:
        """Load a timestamp-ordered JSON array, rejecting duplicate instants.

        Parameters
        ----------
        path : str
            Path to an archive written by to_json.
        """
        ...
    def to_json(self, path: str) -> None:
        """Save observations and configurations, excluding runtime AD objects.

        Parameters
        ----------
        path : str
            Destination file path; an existing file is overwritten.
        """
        ...
    @staticmethod
    def from_dict(value: Sequence[Mapping[str, Any]]) -> MarketSnapshotStore:
        """Load a native snapshot array, rejecting duplicate instants.

        Parameters
        ----------
        value : sequence of mappings
            Native snapshot dictionaries, as returned by to_dict.
        """
        ...
    def to_dict(self) -> list[dict[str, Any]]:
        """Native snapshot array in timestamp order, with RFC 3339 strings."""
        ...

@final
class Level:
    """Market quote level used for construction and calibration."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Level:
        """Parse the quote side used for construction or calibration.

        Parameters
        ----------
        s : str
            Case-insensitive name: ``Mid``, ``Bid``, or ``Ask``.

        Returns
        -------
        Level
            Matching quote-level selector.
        """
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
        """Parse a supported ISO currency code.

        Parameters
        ----------
        s : str
            Case-insensitive alphabetic code of a supported ``Currency``
            member, such as ``"USD"`` or ``"EUR"``; not a name or symbol.

        Returns
        -------
        Currency
            Matching currency.
        """
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
        """Identify an equity underlying.

        Parameters
        ----------
        name : str
            Equity identifier retained verbatim for market-data lookup.

        Returns
        -------
        MarketIndex
            Equity index carrying the supplied identifier.
        """
        ...

    @staticmethod
    def fx_pair(base: _CurrencyLike, quote: _CurrencyLike) -> MarketIndex:
        """Identify a directed FX pair.

        Parameters
        ----------
        base : Currency or str
            Base currency as an enum or case-insensitive supported ISO code.
        quote : Currency or str
            Quote currency as an enum or case-insensitive supported ISO code;
            must differ from ``base``.

        Returns
        -------
        MarketIndex
            Pair whose spot is quote-currency units per one base-currency unit.
        """
        ...

    @staticmethod
    def collateral(currency: _CurrencyLike, collateral_currency: _CurrencyLike) -> MarketIndex:
        """Identify a collateral-dependent discount curve.

        Parameters
        ----------
        currency : Currency or str
            Cashflow currency, as an enum or case-insensitive supported ISO code.
        collateral_currency : Currency or str
            CSA collateral currency, as an enum or case-insensitive supported
            ISO code.

        Returns
        -------
        MarketIndex
            Curve identity for cashflows in ``currency`` collateralized in
            ``collateral_currency``.
        """
        ...

    @staticmethod
    def credit(name: str) -> MarketIndex:
        """Identify a reference entity's credit curve.

        Parameters
        ----------
        name : str
            Reference-entity identifier, retained verbatim.

        Returns
        -------
        MarketIndex
            Credit index for the named entity.
        """
        ...

    @staticmethod
    def other(name: str) -> MarketIndex:
        """Identify a custom market-data index.

        Parameters
        ----------
        name : str
            Custom lookup identifier, retained verbatim.

        Returns
        -------
        MarketIndex
            Custom index carrying the supplied name.
        """
        ...

    @staticmethod
    def parse(s: str) -> MarketIndex:
        """Parse a fieldless market-index member.

        Parameters
        ----------
        s : str
            Case-insensitive class-constant name, such as ``"SOFR"`` or
            ``"EURIBOR3m"``. Parameterized equity, credit, FX, collateral,
            and custom variants must be built with their dedicated factories.

        Returns
        -------
        MarketIndex
            Matching fieldless index.
        """
        ...
    FedFunds: ClassVar[MarketIndex]
    CDI: ClassVar[MarketIndex]
    DESTR: ClassVar[MarketIndex]
    KOFR: ClassVar[MarketIndex]
    ZARONIA: ClassVar[MarketIndex]
    BBSW1m: ClassVar[MarketIndex]
    BBSW2m: ClassVar[MarketIndex]
    BBSW3m: ClassVar[MarketIndex]
    BBSW4m: ClassVar[MarketIndex]
    BBSW6m: ClassVar[MarketIndex]
    BKBM1m: ClassVar[MarketIndex]
    BKBM2m: ClassVar[MarketIndex]
    BKBM3m: ClassVar[MarketIndex]
    BKBM4m: ClassVar[MarketIndex]
    BKBM6m: ClassVar[MarketIndex]
    NIBOR1m: ClassVar[MarketIndex]
    NIBOR3m: ClassVar[MarketIndex]
    NIBOR6m: ClassVar[MarketIndex]
    NIBOR12m: ClassVar[MarketIndex]
    SHIBOR1d: ClassVar[MarketIndex]
    SHIBOR1w: ClassVar[MarketIndex]
    SHIBOR2w: ClassVar[MarketIndex]
    SHIBOR1m: ClassVar[MarketIndex]
    SHIBOR3m: ClassVar[MarketIndex]
    SHIBOR6m: ClassVar[MarketIndex]
    SHIBOR12m: ClassVar[MarketIndex]
    EURIBOR1w: ClassVar[MarketIndex]
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
        """Parse an interpolation rule.

        Parameters
        ----------
        s : str
            Case-insensitive name: ``Linear``, ``LogLinear``, or ``CubicSpline``.

        Returns
        -------
        Interpolator
            Matching interpolation method.
        """
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
        """Parse a volatility quotation convention.

        Parameters
        ----------
        s : str
            Case-insensitive ``Black`` (lognormal) or ``Normal`` (absolute).

        Returns
        -------
        VolatilityType
            Convention determining how quoted volatilities are interpreted.
        """
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
        """Parse the coordinate used along a volatility smile.

        Parameters
        ----------
        s : str
            Case-insensitive name: ``Strike``, ``Delta``, or ``LogMoneyness``.

        Returns
        -------
        SmileType
            Matching smile-axis convention.
        """
        ...
    Strike: ClassVar[SmileType]
    Delta: ClassVar[SmileType]
    LogMoneyness: ClassVar[SmileType]

@final
class QuoteStore:
    """Observable market quotes indexed by identifier and tied to a reference date.

    Quote values retain their instrument-specific units; the store does not
    convert percentages, basis points, prices, or volatilities.
    """

    def __repr__(self) -> str:
        ...

    def __len__(self) -> int:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> QuoteStore:
        """Construct a quote store from native quote records.

        Parameters
        ----------
        obj : mapping
            Native quote-store records with ``reference_date`` and ``quotes``.
            Each quote record identifies the quote with a native parseable
            ``identifier`` and supplies its available ``mid``, ``bid``, and
            ``ask`` levels; omitted levels are absent, not inferred. Identifiers
            must follow the library's instrument-quote syntax, not arbitrary labels.
            ISO date strings use ``YYYY-MM-DD``; ``Date`` values are normalized
            by the Python input converter.

        Returns
        -------
        QuoteStore
            Store containing the supplied quote records.
        """
        ...

    @staticmethod
    def from_json(path: str) -> QuoteStore:
        """Load native quote-store records from disk.

        Parameters
        ----------
        path : str
            JSON file path, not JSON text. The file contains ``reference_date``
            and a ``quotes`` array in the native quote-store schema.

        Returns
        -------
        QuoteStore
            Loaded market quotes and their reference date.
        """
        ...

    def identifiers(self) -> list[str]:
        """List the available quote identifiers.

        Returns
        -------
        list of str
            Identifiers sorted lexicographically.
        """
        ...

    def value(self, identifier: str, level: str=...) -> float:
        """Retrieve one observed quote level.

        Parameters
        ----------
        identifier : str
            Exact identifier of the stored market quote.
        level : str, optional
            Case-insensitive ``"mid"``, ``"bid"``, or ``"ask"``; defaults to
            ``"mid"``. This method accepts a string, not a ``Level`` object.

        Returns
        -------
        float
            Stored value in the quote's native units. Missing identifiers or
            unavailable levels raise an error rather than returning a fallback.
        """
        ...

    def levels(self, identifier: str) -> Any:
        """Retrieve all three levels for one quote.

        Parameters
        ----------
        identifier : str
            Exact identifier of the stored market quote.

        Returns
        -------
        tuple of float or None
            ``(mid, bid, ask)`` in native quote units, with ``None`` for an
            unavailable level. A missing identifier raises an error.
        """
        ...

    def to_dataframe(self) -> DataFrame:
        """Tabulate all observed quotes.

        Returns
        -------
        pandas.DataFrame
            One row per quote, sorted by ``identifier``. ``identifier`` is the
            lookup label; ``mid``, ``bid``, and ``ask`` contain the available
            levels in each quote's native units. Unavailable levels become
            missing pandas values.
        """
        ...

    @property
    def reference_date(self) -> Date:
        """Reference date."""
        ...

@final
class FixingStore:
    """Stored historical index observations.

    Missing observations raise an error on lookup. Neither automatic
    forecasting nor validated missing-fixing interpolation is part of this
    interface. The native fill_missing_fixings routine is not exposed here
    and currently uses reversed interpolation coordinates; it must not be
    treated as reliable historical-data imputation.
    """

    def __new__(cls) -> Self:
        """Create an empty historical-fixing store."""
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> FixingStore:
        """Deserialize historical fixings.

        Parameters
        ----------
        obj : mapping
            Native ``FixingStore`` object with a ``values`` mapping from native
            fieldless index names to date/value mappings, such as
            ``{"values": {"SOFR": {"2025-01-02": 0.05}}}``. Index names are
            case-sensitive, dates use ``YYYY-MM-DD``, and observations retain
            their native units. This is not a single fixing record or DataFrame.

        Returns
        -------
        FixingStore
            Historical observations grouped by market index.
        """
        ...

    @staticmethod
    def from_json(path: str) -> FixingStore:
        """Load historical fixings from disk.

        Parameters
        ----------
        path : str
            Path to a JSON file in the native ``FixingStore`` schema,
            not an inline JSON document.

        Returns
        -------
        FixingStore
            Loaded historical observations.
        """
        ...

    def add(self, index: _IndexLike, date: _DateLike, value: float) -> None:
        """Insert or replace an index observation on a date.

        Parameters
        ----------
        index : MarketIndex, str, or mapping
            Typed index, native fieldless index name such as ``"SOFR"``,
            or native tagged index mapping. String names are case-sensitive
            on this conversion path; use typed factories for parameterized indices.
        date : Date, datetime.date, or str
            Observation date; strings use ``YYYY-MM-DD``.
        value : float
            Observed value in the index's native units, such as an annual
            decimal rate (``0.05`` for 5%). No unit conversion is applied.
        """
        ...

    def fixing(self, index: _IndexLike, date: _DateLike) -> float:
        """Look up an observed historical fixing.

        Parameters
        ----------
        index : MarketIndex, str, or mapping
            Typed index, case-sensitive native fieldless name, or native
            tagged index mapping, as accepted by ``add``.
        date : Date, datetime.date, or str
            Observation date; strings use ``YYYY-MM-DD``.

        Returns
        -------
        float
            Stored observation in the index's native units. Missing index/date
            observations raise an error; this query does not forecast a fixing.
        """
        ...

    def to_dataframe(self, index: _IndexLike) -> DataFrame:
        """Tabulate the historical observations for one index.

        Parameters
        ----------
        index : MarketIndex, str, or mapping
            Typed index, case-sensitive native fieldless name, or native
            tagged index mapping.

        Returns
        -------
        pandas.DataFrame
            ``date`` contains ISO observation-date strings; ``value`` contains
            fixings in the index's native units, sorted chronologically.
            No interpolation is performed; a missing index raises an error.
        """
        ...

@final
class FxStore:
    """FX spot rates. Records are `{"base": "CLP", "quote": "USD", "rate": 0.00111}`,
meaning 1 base = rate quote."""

    def __new__(cls) -> Self:
        """Create an empty FX spot-rate store."""
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> FxStore:
        """Deserialize a sequence of directed FX spot records.

        Parameters
        ----------
        obj : sequence of mappings
            Native record list, despite the method's name. Each record has
            ``base``, ``quote``, and ``rate``: one base-currency unit equals
            ``rate`` quote-currency units. Currency codes in serialized records
            use native enum names such as ``"USD"`` and ``"EUR"``.

        Returns
        -------
        FxStore
            Store built from the supplied directed rates.
        """
        ...

    @staticmethod
    def from_json(path: str) -> FxStore:
        """Load directed FX spot records from disk.

        Parameters
        ----------
        path : str
            JSON file path, not JSON text. The root is a list of records with
            ``base``, ``quote``, and ``rate`` fields, not a wrapping object.

        Returns
        -------
        FxStore
            Loaded FX spots, quoted as quote units per base unit.
        """
        ...

    def add(self, base: Any, quote: Any, rate: float) -> None:
        """Insert a directed FX spot rate.

        Parameters
        ----------
        base : Currency or str
            Base currency enum or case-insensitive supported ISO code.
        quote : Currency or str
            Quote currency enum or case-insensitive supported ISO code.
        rate : float
            Quote-currency units exchanged for one base-currency unit.
        """
        ...

    def rate(self, base: Any, quote: Any) -> float:
        """Resolve an FX conversion rate.

        Parameters
        ----------
        base : Currency or str
            Source currency enum or case-insensitive supported ISO code.
        quote : Currency or str
            Destination currency enum or case-insensitive supported ISO code.

        Returns
        -------
        float
            Quote-currency units per base-currency unit, obtained from a direct,
            inverse, or triangulated rate. A missing conversion raises an error.
        """
        ...

@final
class CurveConfiguration:
    """Inputs for a projection or discount curve built by ``PricingContext``.

    Quote identifiers select instrument calibration or direct zero-rate/discount
    interpolation. Use ``composite`` to combine two other configured curves.
    """

    def __new__(cls, *, market_index: _IndexLike, quotes: Sequence[str] | None=..., day_counter: DayCounter | str | None=..., interpolator: Interpolator | str | None=..., enable_extrapolation: bool=...) -> Self:
        """Configure a curve for later construction by a pricing context.

        Parameters
        ----------
        market_index : MarketIndex, str, or mapping
            Curve lookup identity: typed index, case-sensitive native fieldless
            name such as ``"SOFR"``, or native tagged index mapping.
        quotes : sequence of str or None, optional
            Calibration instruments, ``DiscountFactor_CCY_Index_Pillar``, or
            ``ZeroRate_CCY_Index_Pillar[_Compounding[_Frequency]]`` identifiers.
            Direct curves use one quote family with matching conventions;
            pillars are tenors or ISO dates. Zero rates default to Continuous
            and Annual. The curve supplies day count and interpolation.
            Omitted or ``None`` supplies an empty list, not inferred quotes.
        day_counter : DayCounter, str, or None, optional
            Curve time basis as an enum or case-insensitive name. Omitted or
            ``None`` uses ``Actual360``.
        interpolator : Interpolator, str, or None, optional
            Enum or case-insensitive ``Linear``, ``LogLinear``, or
            ``CubicSpline``. Omitted or ``None`` uses ``LogLinear``.
        enable_extrapolation : bool, optional
            Allow evaluation outside the pillar range. Defaults to ``True``.
        """
        ...

    @staticmethod
    def composite(*, market_index: _IndexLike, base_index: _IndexLike, spread_index: _IndexLike) -> CurveConfiguration:
        """Multiply discount factors from two configured component curves.

        Parameters
        ----------
        market_index : MarketIndex, str, or mapping
            Identity of the resulting composite curve.
        base_index : MarketIndex, str, or mapping
            Benchmark curve configured in the same context.
        spread_index : MarketIndex, str, or mapping
            Spread curve configured in the same context. Components must share
            a reference date and day-count convention.

        Returns
        -------
        CurveConfiguration
            Composite definition referencing its two component indices.
        """
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> CurveConfiguration:
        """Deserialize one native curve configuration.

        Parameters
        ----------
        obj : mapping
            Ordinary curves contain ``market_index``, quote identifiers and
            optional ``day_counter``, ``interpolator`` and ``enable_extrapolation``
            settings. Composite curves identify ``base_index`` and ``spread_index``
            and cannot contain quotes. Typed supported enums and
            indices are normalized; serialized values use native case-sensitive
            names and tags. Unknown fields are rejected. Defaults apply to
            omitted fields, not explicit ``None`` in non-nullable fields.

        Returns
        -------
        CurveConfiguration
            Single configuration; no curve is built until context initialization.
        """
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[CurveConfiguration]:
        """Deserialize one or more curve configurations.

        Parameters
        ----------
        obj : mapping or sequence of mappings
            One native curve configuration, a list of configurations, or an
            object with exactly one key whose value is that list. Each item
            follows the schema accepted by ``from_dict``.

        Returns
        -------
        list of CurveConfiguration
            Configurations in input order, including a one-element list for
            a single unwrapped configuration.
        """
        ...

    @staticmethod
    def from_json(path: str) -> list[CurveConfiguration]:
        """Load curve configurations from disk.

        Parameters
        ----------
        path : str
            JSON file path, not inline JSON. The root may be one native
            configuration, an array, or an object with exactly one key
            wrapping an array. Items use the schema described by ``from_dict``.

        Returns
        -------
        list of CurveConfiguration
            Loaded configurations; the result is always a list.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the curve definition without building a curve.

        Returns
        -------
        dict
            Native serialized configuration, with enum names and tagged
            parameterized variants rather than Python wrapper objects.
        """
        ...

@final
class CreditCurveConfiguration:
    """CDS calibration inputs for a reference entity's survival curve.

    The configuration identifies the credit and discount curves, quote currency,
    assumed recovery, and premium-leg conventions. It does not itself build
    a hazard-rate or survival curve.
    """

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> CreditCurveConfiguration:
        """Deserialize one CDS-based credit-curve definition.

        Parameters
        ----------
        obj : mapping
            Required fields are ``market_index`` (credit identity), ``currency``,
            ``discount_index``, and ``recovery`` (fraction, e.g. ``0.4`` for 40%).
            Use a typed ``MarketIndex.credit`` value or the native tagged credit
            mapping, not its display string. Omitted fields default to
            ``day_counter="Actual360"``, ``premium_frequency="Quarterly"``,
            ``interpolator="LogLinear"``, ``enable_extrapolation=True``, and
            an empty ``quotes`` list of CDS par-spread identifiers. Typed enums
            are normalized; serialized enum names are case-sensitive.

        Returns
        -------
        CreditCurveConfiguration
            Definition to be calibrated during pricing-context initialization.
        """
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[CreditCurveConfiguration]:
        """Deserialize a collection of credit-curve definitions.

        Parameters
        ----------
        obj : mapping or sequence of mappings
            One definition accepted by ``from_dict``, a list of such definitions,
            or an object with exactly one key wrapping that list.

        Returns
        -------
        list of CreditCurveConfiguration
            Definitions in input order; a single definition becomes a one-item list.
        """
        ...

    @staticmethod
    def from_json(path: str) -> list[CreditCurveConfiguration]:
        """Load credit-curve definitions from disk.

        Parameters
        ----------
        path : str
            JSON file path. The root may be a native definition, an array,
            or an object with exactly one key wrapping an array. Each item uses
            the fields and defaults described by ``from_dict``; parameterized
            indices use native tagged JSON, not display strings.

        Returns
        -------
        list of CreditCurveConfiguration
            Loaded definitions; always a list.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the credit calibration inputs.

        Returns
        -------
        dict
            Native definition including recovery, premium conventions, quote
            identifiers, and tagged credit-index data, not calibrated hazards.
        """
        ...

@final
class VolatilitySurfaceConfiguration:
    """Quote selection and conventions for an expiry-by-smile volatility surface."""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> VolatilitySurfaceConfiguration:
        """Deserialize one volatility-surface definition.

        Parameters
        ----------
        obj : mapping
            Required ``market_index`` and ``quotes`` (quote identifiers), plus
            optional ``volatility_type`` (defaults to ``"Black"``),
            ``smile_type`` (defaults to ``"Strike"``), and
            ``extrapolation_policy`` (defaults to ``"Reject"``).
            ``"Flat"`` clamps out-of-grid queries to the boundary. Typed index
            and volatility/smile enums are normalized; serialized values use
            native case-sensitive names. Unknown fields are rejected.

        Returns
        -------
        VolatilitySurfaceConfiguration
            Surface definition for construction during context initialization.
        """
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[VolatilitySurfaceConfiguration]:
        """Deserialize a collection of surface definitions.

        Parameters
        ----------
        obj : mapping or sequence of mappings
            One definition accepted by ``from_dict``, a list of definitions,
            or an object with exactly one key wrapping that list.

        Returns
        -------
        list of VolatilitySurfaceConfiguration
            Definitions in input order, always returned as a list.
        """
        ...

    @staticmethod
    def from_json(path: str) -> list[VolatilitySurfaceConfiguration]:
        """Load volatility-surface definitions from disk.

        Parameters
        ----------
        path : str
            JSON file path. The root may be one native definition, an array,
            or an object with exactly one key wrapping an array. Items use
            ``from_dict`` fields and native serialized enum/index formats.

        Returns
        -------
        list of VolatilitySurfaceConfiguration
            Loaded definitions; always a list, not a constructed surface.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the surface's construction inputs.

        Returns
        -------
        dict
            Native index, quote identifiers, volatility and smile conventions,
            and extrapolation policy, not interpolated volatility values.
        """
        ...

@final
class VolatilityCubeConfiguration:
    """Quote selection and conventions for an expiry-by-tenor-by-smile cube."""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> VolatilityCubeConfiguration:
        """Deserialize one volatility-cube definition.

        Parameters
        ----------
        obj : mapping
            Required ``market_index`` and ``quotes`` (quote identifiers across
            expiries, underlying tenors, and smile coordinates). Optional
            ``volatility_type`` defaults to ``"Black"``, ``smile_type`` to
            ``"Strike"``, and ``extrapolation_policy`` to ``"Reject"``;
            ``"Flat"`` clamps queries at the grid boundary. Typed index and
            volatility/smile enums are normalized; serialized values use native
            case-sensitive names. Unknown fields are rejected.

        Returns
        -------
        VolatilityCubeConfiguration
            Cube definition for construction during context initialization.
        """
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[VolatilityCubeConfiguration]:
        """Deserialize a collection of cube definitions.

        Parameters
        ----------
        obj : mapping or sequence of mappings
            One definition accepted by ``from_dict``, a list of definitions,
            or an object with exactly one key wrapping that list.

        Returns
        -------
        list of VolatilityCubeConfiguration
            Definitions in input order, always returned as a list.
        """
        ...

    @staticmethod
    def from_json(path: str) -> list[VolatilityCubeConfiguration]:
        """Load volatility-cube definitions from disk.

        Parameters
        ----------
        path : str
            JSON file path. The root may be one native definition, an array,
            or an object with exactly one key wrapping an array. Items use
            ``from_dict`` fields and native serialized enum/index formats.

        Returns
        -------
        list of VolatilityCubeConfiguration
            Loaded definitions; always a list, not a constructed cube.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the cube's construction inputs.

        Returns
        -------
        dict
            Native index, quote identifiers, volatility and smile conventions,
            and extrapolation policy, not interpolated volatility values.
        """
        ...

@final
class SimulationConfiguration:
    """Stochastic model and date-grid inputs for Monte Carlo path generation."""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> SimulationConfiguration:
        """Deserialize one Monte Carlo simulation definition.

        Parameters
        ----------
        obj : mapping
            Required ``market_index``, native tagged ``model``, and ``horizon``
            tenor such as ``"5Y"``. Model variants are ``HullWhite``,
            ``BrownianMotion``, and ``Lgm``, with native tagged ``Fixed`` or
            ``Calibrated`` parameter sources. Fixed Gaussian ``sigma`` is in
            rate units per square-root year; fixed lognormal ``volatility``
            is a decimal per square-root year. Omitted ``n_paths``, ``seed``,
            ``frequency``, and ``day_counter`` default to 1000, 42,
            ``"Monthly"``, and ``"Actual365"``, respectively. Typed indices,
            periods, and supported enums are normalized; serialized variants
            must use native names and tags, not display strings.

        Returns
        -------
        SimulationConfiguration
            Definition to be resolved and simulated by a pricing context.
        """
        ...

    @staticmethod
    def from_dict_list(obj: Mapping[str, Any]) -> list[SimulationConfiguration]:
        """Deserialize a collection of simulation definitions.

        Parameters
        ----------
        obj : mapping or sequence of mappings
            One definition accepted by ``from_dict``, a list of definitions,
            or an object with exactly one key wrapping that list.

        Returns
        -------
        list of SimulationConfiguration
            Definitions in input order, always returned as a list.
        """
        ...

    @staticmethod
    def from_json(path: str) -> list[SimulationConfiguration]:
        """Load Monte Carlo simulation definitions from disk.

        Parameters
        ----------
        path : str
            JSON file path. The root may be one native definition, an array,
            or an object with exactly one key wrapping an array. Each item uses
            ``from_dict`` fields, including the native tagged model definition.

        Returns
        -------
        list of SimulationConfiguration
            Loaded definitions; always a list. No paths are generated here.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the stochastic model and simulation-grid inputs.

        Returns
        -------
        dict
            Native model and parameter-source tags, index, path count, seed,
            horizon, frequency, and day counter, not simulated paths.
        """
        ...

@final
class ZeroRateTermStructure:
    """Interpolate annual zero rates on explicitly dated pillars.

    Rates retain their specified compounding and frequency. Discount factors
    are calculated from those rates using the curve's day-count basis.
    """

    def __new__(cls, reference_date: _DateLike, dates: Sequence[_DateLike], rates: Sequence[float], *, day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., frequency: Frequency | str | None=..., interpolator: Interpolator | str | None=..., enable_extrapolation: bool=...) -> Self:
        """Create a dated zero-rate curve.

        Parameters
        ----------
        reference_date : Date, datetime.date, or str
            Origin for discount factors and curve times; strings use ``YYYY-MM-DD``.
        dates : sequence of Date, datetime.date, or str
            Nonempty, strictly increasing pillar dates on or after the reference
            date; strings use ``YYYY-MM-DD``. Resulting curve times must also
            increase strictly under ``day_counter``.
        rates : sequence of float
            Finite annual decimal zero rates, one per date; ``0.05`` means 5%.
        day_counter : DayCounter, str, or None, optional
            Typed convention or case-insensitive member name. Omitted or
            ``None`` uses ``Actual365``.
        compounding : Compounding, str, or None, optional
            Convention for the zero rates, as an enum or case-insensitive name.
            Omitted or ``None`` uses ``Continuous``.
        frequency : Frequency, str, or None, optional
            Compounding frequency as an enum or case-insensitive name. Omitted
            or ``None`` uses ``Annual``; relevant for periodic compounding.
        interpolator : Interpolator, str, or None, optional
            Enum or case-insensitive ``Linear``, ``LogLinear``, or ``CubicSpline``.
            Omitted or ``None`` uses ``Linear``; interpolation is on zero rates.
        enable_extrapolation : bool, optional
            Allow zero-rate queries outside the pillar-time range. Defaults to
            ``False``. Negative curve times remain invalid.
        """
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> ZeroRateTermStructure:
        """Deserialize an explicitly dated zero-rate curve.

        Parameters
        ----------
        obj : mapping
            Native fields ``reference_date``, ``dates``, ``rates``,
            ``rate_definition`` (``day_counter``, ``compounding``, ``frequency``),
            ``interpolator``, and ``enable_extrapolation``, with optional
            ``pillar_labels``. These required fields do not inherit the Python
            constructor's defaults. Typed dates and supported enums are
            normalized; serialized names use native case-sensitive variants.

        Returns
        -------
        ZeroRateTermStructure
            Validated dated curve with the supplied conventions.
        """
        ...

    @staticmethod
    def from_json(path: str) -> ZeroRateTermStructure:
        """Load an explicitly dated zero-rate curve from disk.

        Parameters
        ----------
        path : str
            JSON file path, not JSON text. The root is one dated curve object
            using the required native fields described by ``from_dict``.

        Returns
        -------
        ZeroRateTermStructure
            Validated curve loaded from the file.
        """
        ...

    def zero_rate(self, date: _DateLike) -> float:
        """Interpolate the zero rate at a calendar date.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Query date on or after the reference date; strings use ``YYYY-MM-DD``.
            Dates outside the pillar range require extrapolation to be enabled.

        Returns
        -------
        float
            Annual decimal zero rate under the curve's compounding and
            frequency, not necessarily a continuously compounded rate.
        """
        ...

    def zero_rate_from_time(self, time: float) -> float:
        """Interpolate the zero rate at a curve time.

        Parameters
        ----------
        time : float
            Finite, nonnegative year fraction from the reference date, measured
            on the curve's day-count basis. Outside-pillar queries require
            extrapolation to be enabled.

        Returns
        -------
        float
            Annual decimal zero rate under the curve's compounding and frequency.
        """
        ...

    def discount_factor(self, date: _DateLike) -> float:
        """Calculate a discount factor to a calendar date.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Cashflow date; strings use ``YYYY-MM-DD``. Nonzero curve times must
            be in range unless extrapolation is enabled.

        Returns
        -------
        float
            Dimensionless reference-date value of one unit paid on ``date``.
            The reference-date factor is 1, even if it precedes the first pillar.
        """
        ...

    def discount_factor_from_time(self, time: float) -> float:
        """Calculate a discount factor at a curve time.

        Parameters
        ----------
        time : float
            Finite, nonnegative year fraction from the reference date on the
            curve's day-count basis. Nonzero outside-pillar queries require
            extrapolation to be enabled.

        Returns
        -------
        float
            Positive, dimensionless discount factor; exactly 1 at time zero.
        """
        ...

    def forward_rate(self, start_date: _DateLike, end_date: _DateLike, compounding: Compounding | str | None=..., frequency: Frequency | str | None=...) -> float:
        """Derive a forward rate from the ratio of two discount factors.

        Parameters
        ----------
        start_date : Date, datetime.date, or str
            Start of the forward interval; strings use ``YYYY-MM-DD``.
        end_date : Date, datetime.date, or str
            End of the interval, strictly after ``start_date``; ISO date strings
            use ``YYYY-MM-DD``. Endpoint discount factors must be evaluable.
        compounding : Compounding, str, or None, optional
            Output convention as an enum or case-insensitive name. Omitted or
            ``None`` uses this curve's zero-rate compounding convention.
        frequency : Frequency, str, or None, optional
            Output compounding frequency as an enum or case-insensitive name.
            Omitted or ``None`` uses this curve's zero-rate frequency.

        Returns
        -------
        float
            Annual decimal forward rate using the curve's day-count basis and
            requested output compounding/frequency.
        """
        ...

    def forward_rate_from_time(self, start: float, end: float) -> float:
        """Derive a simple forward rate between two curve times.

        Parameters
        ----------
        start : float
            Finite, nonnegative starting year fraction from the reference date
            on the curve's day-count basis.
        end : float
            Ending year fraction, not less than ``start``. Endpoint discount
            factors must be evaluable. An effectively zero interval is evaluated
            using a small positive time increment.

        Returns
        -------
        float
            Simple annualized decimal forward, ``(D(start)/D(end)-1)/(end-start)``
            for a nonzero interval. Unlike ``forward_rate``, this query does not
            inherit the zero-rate compounding convention.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the dated zero-rate curve.

        Returns
        -------
        dict
            Native ``reference_date``, ``dates``, ``rates``, ``rate_definition``,
            ``interpolator``, and ``enable_extrapolation``, plus ``pillar_labels``
            when present. Dates and enums are serialized values, not wrappers.
        """
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
    """Interpolate annual zero rates at tenors from a fixed reference date.

    Pillar dates are resolved by calendar-date addition, without a business-day
    calendar. Queries use the resulting dated zero-rate curve.
    """

    def __new__(cls, reference_date: _DateLike, tenors: Sequence[_PeriodLike], rates: Sequence[float], *, day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., frequency: Frequency | str | None=..., interpolator: Interpolator | str | None=..., enable_extrapolation: bool=...) -> Self:
        """Create a zero-rate curve from reference-date tenors.

        Parameters
        ----------
        reference_date : Date, datetime.date, or str
            Origin for tenors, curve times, and discount factors. Strings use
            ``YYYY-MM-DD``.
        tenors : sequence of Period or str
            Nonempty pillar intervals, such as ``"6M"`` and ``"5Y"``. Resolved
            dates must increase strictly, remain on or after the reference date,
            and yield strictly increasing curve times. No holiday adjustment occurs.
        rates : sequence of float
            Finite annual decimal zero rates, one per tenor; ``0.05`` means 5%.
        day_counter : DayCounter, str, or None, optional
            Typed convention or case-insensitive member name. Omitted or
            ``None`` uses ``Actual365``.
        compounding : Compounding, str, or None, optional
            Convention for the supplied zero rates, as an enum or case-insensitive
            member name. Omitted or ``None`` uses ``Continuous``.
        frequency : Frequency, str, or None, optional
            Compounding frequency as an enum or case-insensitive member name.
            Omitted or ``None`` uses ``Annual``; relevant for periodic compounding.
        interpolator : Interpolator, str, or None, optional
            Enum or case-insensitive ``Linear``, ``LogLinear``, or ``CubicSpline``.
            Omitted or ``None`` uses ``Linear``; interpolation is on zero rates.
        enable_extrapolation : bool, optional
            Allow zero-rate queries outside the pillar-time range. Defaults to
            ``False``. Negative curve times remain invalid.
        """
        ...

    def __repr__(self) -> str:
        ...

    def zero_rate(self, date: _DateLike) -> float:
        """Interpolate the zero rate at a calendar date.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Query date on or after the reference date; strings use ``YYYY-MM-DD``.
            Outside-pillar queries require extrapolation to be enabled.

        Returns
        -------
        float
            Annual decimal zero rate under the curve's compounding and frequency.
        """
        ...

    def zero_rate_from_time(self, time: float) -> float:
        """Interpolate the zero rate at a curve time.

        Parameters
        ----------
        time : float
            Finite, nonnegative year fraction from the reference date on the
            curve's day-count basis. Outside-pillar queries require extrapolation.

        Returns
        -------
        float
            Annual decimal zero rate under the curve's compounding and frequency.
        """
        ...

    def discount_factor(self, date: _DateLike) -> float:
        """Calculate a discount factor to a calendar date.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Cashflow date; strings use ``YYYY-MM-DD``. Nonzero curve times must
            be in range unless extrapolation is enabled.

        Returns
        -------
        float
            Dimensionless reference-date value of one unit paid on ``date``.
            The reference-date factor is 1, even if it precedes the first pillar.
        """
        ...

    def discount_factor_from_time(self, time: float) -> float:
        """Calculate a discount factor at a curve time.

        Parameters
        ----------
        time : float
            Finite, nonnegative year fraction from the reference date on the
            curve's day-count basis. Nonzero outside-pillar queries require
            extrapolation to be enabled.

        Returns
        -------
        float
            Positive, dimensionless discount factor; exactly 1 at time zero.
        """
        ...

    def forward_rate(self, start_date: _DateLike, end_date: _DateLike, compounding: Compounding | str | None=..., frequency: Frequency | str | None=...) -> float:
        """Derive a forward rate from the ratio of two discount factors.

        Parameters
        ----------
        start_date : Date, datetime.date, or str
            Start of the forward interval; strings use ``YYYY-MM-DD``.
        end_date : Date, datetime.date, or str
            End of the interval, strictly after ``start_date``; strings use
            ``YYYY-MM-DD``. Endpoint discount factors must be evaluable.
        compounding : Compounding, str, or None, optional
            Output convention as an enum or case-insensitive name. Omitted or
            ``None`` uses the curve's zero-rate compounding convention.
        frequency : Frequency, str, or None, optional
            Output compounding frequency as an enum or case-insensitive name.
            Omitted or ``None`` uses the curve's zero-rate frequency.

        Returns
        -------
        float
            Annual decimal forward rate under the curve's day-count basis and
            requested output compounding/frequency.
        """
        ...

    def forward_rate_from_time(self, start: float, end: float) -> float:
        """Derive a simple forward rate between two curve times.

        Parameters
        ----------
        start : float
            Finite, nonnegative starting year fraction from the reference date
            on the curve's day-count basis.
        end : float
            Ending year fraction, not less than ``start``. Endpoint discount
            factors must be evaluable. An effectively zero interval is evaluated
            using a small positive time increment.

        Returns
        -------
        float
            Simple annualized decimal forward, ``(D(start)/D(end)-1)/(end-start)``
            for a nonzero interval. This query does not inherit the curve's
            zero-rate compounding convention.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the equivalent dated zero-rate curve.

        Returns
        -------
        dict
            Native dated curve with ``reference_date``, resolved ``dates``,
            decimal ``rates``, ``rate_definition``, ``interpolator``, and
            ``enable_extrapolation``, plus ``pillar_labels`` when present.
            Tenors are not retained in this representation; it can be loaded
            with ``ZeroRateTermStructure.from_dict``.
        """
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
    """Read-only view of an interest-rate curve built by a pricing context.

    This view exposes numeric discount and forward queries, not automatic
    differentiation objects or an independently mutable set of nodes.
    """

    def __repr__(self) -> str:
        ...

    def discount_factor(self, date: _DateLike) -> float:
        """Evaluate the curve's discount factor at a date.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Cashflow date; strings use ``YYYY-MM-DD``. Valid range and
            extrapolation behavior depend on the constructed curve.

        Returns
        -------
        float
            Dimensionless reference-date value of one unit paid on ``date``.
        """
        ...

    def forward_rate(self, start: _DateLike, end: _DateLike, compounding: Compounding | str | None=..., frequency: Frequency | str | None=...) -> float:
        """Derive a forward interest rate between calendar dates.

        Parameters
        ----------
        start : Date, datetime.date, or str
            Initial forward date; strings use ``YYYY-MM-DD``.
        end : Date, datetime.date, or str
            Final forward date; strings use ``YYYY-MM-DD``. Endpoints must be
            evaluable by the underlying curve.
        compounding : Compounding, str, or None, optional
            Output convention as an enum or case-insensitive name. Omitted or
            ``None`` uses ``Simple``, not the curve's zero-rate convention.
        frequency : Frequency, str, or None, optional
            Output compounding frequency as an enum or case-insensitive name.
            Omitted or ``None`` uses ``Annual``.

        Returns
        -------
        float
            Annual decimal forward rate implied by the endpoint discount-factor
            ratio, under the requested conventions and the curve's time basis.
            ``0.05`` denotes 5%, not 5 basis points.
        """
        ...

    def nodes(self) -> DataFrame:
        """Tabulate the curve's exposed discount-factor nodes.

        Returns
        -------
        pandas.DataFrame
            ``date`` contains ISO node-date strings; ``discount_factor`` is the
            dimensionless factor from the curve reference date to that node.
            Empty with these columns when the underlying curve exposes no nodes.
        """
        ...

    def pillars(self) -> DataFrame:
        """Tabulate the curve's native labeled pillars.

        Returns
        -------
        pandas.DataFrame
            ``label`` identifies a native curve pillar; ``value`` is its current
            numeric value in the underlying curve's pillar units. Pillar values
            are not universally market quote rates: they depend on the curve
            representation. Empty with these columns if no pillars are exposed.
        """
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
        """Read-only identity used to look up this projection or discount curve."""
        ...

@final
class VolatilitySurface:
    """Read-only expiry-by-smile volatility surface built by a pricing context."""

    def __repr__(self) -> str:
        ...

    def volatility(self, expiry: Any, key: float) -> float:
        """Interpolate an implied volatility at an expiry and smile coordinate.

        Parameters
        ----------
        expiry : Date, datetime.date, Period, or str
            Option expiry as an absolute date or a tenor from the surface's
            reference date. Strings use ``YYYY-MM-DD`` or native period text
            such as ``"6M"``. A numeric year fraction is not accepted here.
        key : float
            Coordinate in the surface's ``smile_type``: absolute strike in the
            underlying's quote units, delta, or dimensionless log-moneyness.
            Match the quote grid's convention and scale; this method does not
            convert strikes to deltas or reinterpret the coordinate.

        Returns
        -------
        float
            Annualized volatility under ``volatility_type``. ``Black`` uses
            decimal lognormal volatility (``0.20`` for 20% per square-root year);
            ``Normal`` uses absolute underlying quote units per square-root year
            (for decimal rates, ``0.01`` is 100 bp per square-root year).
            Out-of-grid behavior follows the configured extrapolation policy.
        """
        ...

    def pillars(self) -> DataFrame:
        """Tabulate the surface's native labeled volatility pillars.

        Returns
        -------
        pandas.DataFrame
            ``label`` is the native pillar identifier; ``value`` is the numeric
            volatility pillar in the surface's Black or Normal quotation units.
            Empty with these columns when no pillars are exposed. The table is
            not an expiry-by-strike pivot or a set of option prices.
        """
        ...

    @property
    def smile_type(self) -> Any:
        """Read-only ``SmileType`` defining the query key: Strike, Delta, or LogMoneyness."""
        ...

    @property
    def volatility_type(self) -> Any:
        """Read-only ``VolatilityType``: Black lognormal or Normal absolute volatility."""
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
    """Read-only expiry-by-underlying-tenor-by-smile volatility cube."""

    def __repr__(self) -> str:
        ...

    def volatility(self, expiry: Any, maturity: _PeriodLike, key: float) -> float:
        """Interpolate implied volatility for an expiry and underlying tenor.

        Parameters
        ----------
        expiry : Date, datetime.date, Period, or str
            Option expiry as an absolute date or a tenor from the cube's reference
            date. Strings use ``YYYY-MM-DD`` or native period text such as ``"1Y"``;
            numeric year fractions are not accepted.
        maturity : Period or str
            Underlying instrument's tenor, such as ``"5Y"`` for a five-year swap,
            not an absolute maturity date or an option-expiry year fraction.
        key : float
            Coordinate in the cube's ``smile_type``: absolute strike in underlying
            quote units, delta, or dimensionless log-moneyness. Its convention
            and scale must match the configured quote grid; no axis conversion
            is performed by this method.

        Returns
        -------
        float
            Annualized volatility under ``volatility_type``: decimal lognormal
            volatility for ``Black`` (``0.20`` for 20% per square-root year), or
            absolute underlying quote units per square-root year for ``Normal``.
            Normal rate volatility of ``0.01`` is 100 bp per square-root year.
            Out-of-grid behavior follows the configured extrapolation policy.
        """
        ...

    def pillars(self) -> DataFrame:
        """Tabulate the cube's native labeled volatility pillars.

        Returns
        -------
        pandas.DataFrame
            ``label`` identifies a native cube pillar; ``value`` is its numeric
            volatility in the cube's Black or Normal quotation units. Empty with
            these columns if no pillars are exposed. This is a two-column pillar
            table, not a three-dimensional reshaping of the cube.
        """
        ...

    @property
    def smile_type(self) -> Any:
        """Read-only ``SmileType`` defining the query key: Strike, Delta, or LogMoneyness."""
        ...

    @property
    def volatility_type(self) -> Any:
        """Read-only ``VolatilityType``: Black lognormal or Normal absolute volatility."""
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
    """Read-only view of Monte Carlo paths generated by a pricing context.

    Path values are model outputs, not trade valuations or exposure profiles.
    Queries return materialized numeric data, not AD objects.
    Generated paths expose no market pillars and do not retain differentiated
    calibration, spot, or volatility replay. This differs from model-backed
    LGM paths used by native scripting and XVA.
    """

    def __repr__(self) -> str:
        ...

    def calibration_diagnostics(self) -> DataFrame | None:
        """Tabulate available Hull-White option-calibration records.

        Returns
        -------
        pandas.DataFrame or None
            One row per recorded calibration instrument. ``identifier`` is the
            quote label and ``expiry`` its tenor string. ``t`` is time to expiry
            in years; ``big_t`` is time to caplet payment or the swap's last date
            in years. ``market_vol`` is the quoted Black or Normal volatility;
            ``market_price`` is the target option price implied by that quote,
            and ``model_price`` is the model price on the same pricing basis.
            ``calibrated_sigma`` is absolute short-rate volatility in rate units
            per square-root year. ``forward_rate`` and ``effective_strike`` are
            decimal rates, with ATM or relative strikes already resolved.
            ``price_error`` equals ``model_price - market_price``, not an absolute
            or relative error. Returns ``None`` whenever no Hull-White quality
            record is attached, including fixed-parameter and other-model runs;
            ``None`` does not universally mean the model was uncalibrated.
        """
        ...

    def dates(self) -> list[Date]:
        """Return the date grid associated with path columns.

        Returns
        -------
        list of Date
            Simulation dates in path-column order, strictly after the context's
            reference date. A fresh list is returned; changing it does not
            alter the simulation.
        """
        ...

    def paths(self) -> DataFrame:
        """Tabulate the generated model paths.

        Returns
        -------
        pandas.DataFrame
            One row per Monte Carlo path and one column per simulation date.
            Column labels are ISO ``YYYY-MM-DD`` strings in ``dates()`` order;
            cells contain numeric model outputs at those dates in the model's
            native units. These are not discount factors, cashflows, trade PVs,
            or exposure quantiles unless the model output itself has that meaning.
            The table contains no reference-date column or path-probability column.
        """
        ...

    @property
    def dt(self) -> float:
        """Read-only average simulation step in years.

        Calculated as the last grid time from the reference date divided by
        the number of simulation dates. Calendar intervals may have unequal
        year fractions; this scalar does not describe every adjacent interval.
        """
        ...

    @property
    def n_paths(self) -> int:
        """Number of simulated paths."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Simulated market index."""
        ...

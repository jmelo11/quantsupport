"""Dates, tenors, calendars, and accrual conventions."""
from datetime import date as PythonDate, datetime as PythonDateTime
from typing import Any, ClassVar, Iterator, Mapping, Sequence, final, overload
from typing_extensions import Self, disjoint_base
from pandas import DataFrame

_DateLike = Date | str | PythonDate
_DateTimeLike = DateTime | str | PythonDateTime
_PeriodLike = Period | str

__all__ = ['DateTime', 'Date', 'Period', 'Calendar', 'Schedule', 'IMM', 'UnitedStatesMarket', 'BrazilMarket', 'AustraliaMarket', 'CanadaMarket', 'NewZealandMarket', 'SouthKoreaMarket', 'ChinaMarket', 'JointCalendarRule', 'Compounding', 'Frequency', 'DayCounter', 'TimeUnit', 'BusinessDayConvention', 'DateGenerationRule']

@final
class DateTime:
    """A UTC instant backed by the library's chrono datetime wrapper.

    Parameters
    ----------
    year : int
        UTC calendar year.
    month : int
        UTC calendar month, 1 through 12.
    day : int
        UTC calendar day of the month.
    hour : int, optional
        UTC hour, 0 through 23. Defaults to zero.
    minute : int, optional
        UTC minute, 0 through 59. Defaults to zero.
    second : int, optional
        UTC second, 0 through 59. Defaults to zero.

    Notes
    -----
    Invalid components raise QuantSupportError. Parsed offsets are normalized
    to UTC; naive timestamps are rejected. Values are immutable, comparable,
    and hashable. Native and serialized values retain nanosecond precision.
    """
    def __new__(cls, year: int, month: int, day: int, hour: int = ..., minute: int = ..., second: int = ...) -> Self: ...
    @staticmethod
    def parse(s: str, fmt: str | None = ...) -> DateTime:
        """Parse an offset-aware timestamp.

        Parameters
        ----------
        s : str
            RFC 3339 timestamp, including Z or a numeric timezone offset.
        fmt : str or None, optional
            Chrono strftime format including an offset; None uses RFC 3339.
        """
        ...
    @staticmethod
    def from_date(date: _DateLike, hour: int = ..., minute: int = ..., second: int = ...) -> DateTime:
        """Combine a calendar date with a UTC clock time.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Calendar date interpreted in UTC.
        hour : int, optional
            UTC hour, default zero.
        minute : int, optional
            UTC minute, default zero.
        second : int, optional
            UTC second, default zero.
        """
        ...
    @staticmethod
    def from_datetime(value: _DateTimeLike) -> DateTime:
        """Convert an aware datetime, library DateTime, or RFC 3339 string.

        Parameters
        ----------
        value : DateTime, datetime.datetime, or str
            Offset-aware instant, normalized to UTC.
        """
        ...
    def to_datetime(self) -> PythonDateTime:
        """Return an aware UTC Python datetime, truncating to microseconds."""
        ...
    def to_str(self, fmt: str | None = ...) -> str:
        """Format the UTC instant.

        Parameters
        ----------
        fmt : str or None, optional
            Chrono strftime format; None produces RFC 3339 with full precision.
        """
        ...
    @property
    def date(self) -> Date:
        """UTC calendar date as a library Date."""
        ...
    @property
    def year(self) -> int:
        """UTC calendar year."""
        ...
    @property
    def month(self) -> int:
        """UTC calendar month, 1 through 12."""
        ...
    @property
    def day(self) -> int:
        """UTC calendar day of the month."""
        ...
    @property
    def hour(self) -> int:
        """UTC hour, 0 through 23."""
        ...
    @property
    def minute(self) -> int:
        """UTC minute, 0 through 59."""
        ...
    @property
    def second(self) -> int:
        """UTC second, 0 through 59."""
        ...
    @property
    def nanosecond(self) -> int:
        """Fractional second in nanoseconds, including chrono leap seconds."""
        ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __lt__(self, other: DateTime, /) -> bool: ...
    def __le__(self, other: DateTime, /) -> bool: ...
    def __gt__(self, other: DateTime, /) -> bool: ...
    def __ge__(self, other: DateTime, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Compounding:
    """Interest compounding convention."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Compounding:
        """Parse an interest-compounding convention.

        Parameters
        ----------
        s : str
            Case-insensitive member name: ``Simple``, ``Compounded``,
            ``Continuous``, ``SimpleThenCompounded``, or ``CompoundedThenSimple``.

        Returns
        -------
        Compounding
            Matching convention.
        """
        ...
    Simple: ClassVar[Compounding]
    Compounded: ClassVar[Compounding]
    Continuous: ClassVar[Compounding]
    SimpleThenCompounded: ClassVar[Compounding]
    CompoundedThenSimple: ClassVar[Compounding]

@final
class Frequency:
    """Payment / coupon frequency."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Frequency:
        """Parse a payment or compounding frequency.

        Parameters
        ----------
        s : str
            Case-insensitive ``Frequency`` member name, such as ``Annual``,
            ``Semiannual``, ``Quarterly``, or ``Monthly``.

        Returns
        -------
        Frequency
            Matching frequency, including special members such as ``Once``
            and ``NoFrequency`` when explicitly named.
        """
        ...
    NoFrequency: ClassVar[Frequency]
    Once: ClassVar[Frequency]
    Annual: ClassVar[Frequency]
    Semiannual: ClassVar[Frequency]
    EveryFourthMonth: ClassVar[Frequency]
    Quarterly: ClassVar[Frequency]
    Bimonthly: ClassVar[Frequency]
    Monthly: ClassVar[Frequency]
    EveryFourthWeek: ClassVar[Frequency]
    Biweekly: ClassVar[Frequency]
    Weekly: ClassVar[Frequency]
    Daily: ClassVar[Frequency]
    OtherFrequency: ClassVar[Frequency]

@final
class DayCounter:
    """Day count convention."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> DayCounter:
        """Parse a day-count convention.

        Parameters
        ----------
        s : str
            Case-insensitive member name: ``Actual360``, ``Actual365``,
            ``Thirty360``, ``Thirty360US``, ``ActualActual``, or ``Business252``.

        Returns
        -------
        DayCounter
            Matching accrual convention.
        """
        ...

    def day_count(self, start: _DateLike, end: _DateLike) -> Any:
        """Count days according to this convention.

        Parameters
        ----------
        start : Date, datetime.date, or str
            Initial date; strings use ``YYYY-MM-DD``.
        end : Date, datetime.date, or str
            Final date; strings use ``YYYY-MM-DD``.

        Returns
        -------
        int
            Signed convention-specific day count, not necessarily elapsed
            calendar days for synthetic or business-day conventions.
        """
        ...

    def year_fraction(self, start: _DateLike, end: _DateLike) -> float:
        """Calculate the accrual interval in years.

        Parameters
        ----------
        start : Date, datetime.date, or str
            Initial accrual date; strings use ``YYYY-MM-DD``.
        end : Date, datetime.date, or str
            Final accrual date; strings use ``YYYY-MM-DD``.

        Returns
        -------
        float
            Year fraction under this day-count convention.
        """
        ...
    Actual360: ClassVar[DayCounter]
    Actual365: ClassVar[DayCounter]
    Thirty360: ClassVar[DayCounter]
    Thirty360US: ClassVar[DayCounter]
    ActualActual: ClassVar[DayCounter]
    Business252: ClassVar[DayCounter]

@final
class TimeUnit:
    """Calendar time unit."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> TimeUnit:
        """Parse a calendar time unit.

        Parameters
        ----------
        s : str
            Case-insensitive name: ``Days``, ``Weeks``, ``Months``, or ``Years``.

        Returns
        -------
        TimeUnit
            Matching unit, rather than a duration in seconds.
        """
        ...
    Days: ClassVar[TimeUnit]
    Weeks: ClassVar[TimeUnit]
    Months: ClassVar[TimeUnit]
    Years: ClassVar[TimeUnit]

@final
class BusinessDayConvention:
    """Business day adjustment convention."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> BusinessDayConvention:
        """Parse a business-day adjustment rule.

        Parameters
        ----------
        s : str
            Case-insensitive member name, such as ``Following``,
            ``ModifiedFollowing``, ``Preceding``, or ``Unadjusted``.

        Returns
        -------
        BusinessDayConvention
            Matching adjustment convention.
        """
        ...
    Following: ClassVar[BusinessDayConvention]
    ModifiedFollowing: ClassVar[BusinessDayConvention]
    HalfMonthModifiedFollowing: ClassVar[BusinessDayConvention]
    Preceding: ClassVar[BusinessDayConvention]
    ModifiedPreceding: ClassVar[BusinessDayConvention]
    Unadjusted: ClassVar[BusinessDayConvention]
    Nearest: ClassVar[BusinessDayConvention]

@final
class DateGenerationRule:
    """Schedule date-generation rule."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> DateGenerationRule:
        """Parse a schedule generation rule.

        Parameters
        ----------
        s : str
            Case-insensitive ``DateGenerationRule`` member name, such as
            ``Backward``, ``Forward``, ``TwentiethIMM``, or ``CDS2015``.

        Returns
        -------
        DateGenerationRule
            Matching rule for generating schedule dates.
        """
        ...
    Backward: ClassVar[DateGenerationRule]
    Forward: ClassVar[DateGenerationRule]
    Zero: ClassVar[DateGenerationRule]
    ThirdWednesday: ClassVar[DateGenerationRule]
    ThirdWednesdayInclusive: ClassVar[DateGenerationRule]
    Twentieth: ClassVar[DateGenerationRule]
    TwentiethIMM: ClassVar[DateGenerationRule]
    OldCDS: ClassVar[DateGenerationRule]
    CDS: ClassVar[DateGenerationRule]
    CDS2015: ClassVar[DateGenerationRule]

@final
class Date:
    """A calendar date, independent of a holiday calendar or time zone.

    Date-valued binding inputs accept ``Date``, an ISO ``YYYY-MM-DD`` string,
    or ``datetime.date``. Arithmetic here does not adjust for business days;
    use ``Calendar`` for holiday-aware operations.
    """

    def __new__(cls, year: int, month: int, day: int) -> Self:
        """Create a valid calendar date.

        Parameters
        ----------
        year : int
            Calendar year.
        month : int
            Month number, from 1 (January) to 12 (December).
        day : int
            Day number, starting at 1 and valid for the given month and year.
        """
        ...

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    def __add__(self, value: int | _PeriodLike, /) -> Date:
        """Add calendar days or a tenor without holiday adjustment.

        Parameters
        ----------
        value : int, Period, or str
            Signed calendar-day count or interval such as ``"6M"``.

        Returns
        -------
        Date
            Shifted date.
        """
        ...

    @overload
    def __sub__(self, value: Date, /) -> int:
        """Measure the signed calendar-day difference from another date.

        Parameters
        ----------
        value : Date
            Date to subtract from this date.

        Returns
        -------
        int
            Calendar days from ``value`` to this date.
        """
        ...

    @overload
    def __sub__(self, value: int | _PeriodLike, /) -> Date:
        """Subtract calendar days or a tenor without holiday adjustment.

        Parameters
        ----------
        value : int, Period, or str
            Signed calendar-day count or interval such as ``"6M"``.

        Returns
        -------
        Date
            Shifted date.
        """
        ...

    @staticmethod
    def parse(s: str, fmt: str=...) -> Date:
        """Parse a calendar date from formatted text.

        Parameters
        ----------
        s : str
            Date text matching ``fmt``.
        fmt : str, optional
            Chrono ``strftime`` format; defaults to ``"%Y-%m-%d"``.

        Returns
        -------
        Date
            Parsed date, without business-day adjustment.
        """
        ...

    def weekday(self) -> str:
        """Return the weekday's English name.

        Returns
        -------
        str
            Weekday name, such as ``"Monday"`` or ``"Thursday"``.
        """
        ...

    def days_in_month(self) -> int:
        """Count calendar days in this date's month.

        Returns
        -------
        int
            Month length, from 28 to 31 days, accounting for leap years.
        """
        ...

    def day_of_year(self) -> int:
        """Return the date's ordinal within its calendar year.

        Returns
        -------
        int
            One-based day number, from 1 to 365 or 366 in a leap year.
        """
        ...

    def is_leap_year(self) -> bool:
        """Test whether this date's year is a leap year.

        Returns
        -------
        bool
            Whether the calendar year contains February 29.
        """
        ...

    def end_of_month(self) -> Date:
        """Find the last calendar day of this date's month.

        Returns
        -------
        Date
            Calendar month-end without business-day adjustment; it may be
            a weekend or holiday.
        """
        ...

    def advance(self, n: int, units: TimeUnit | str) -> Date:
        """Shift the date by calendar units without holiday adjustment.

        Parameters
        ----------
        n : int
            Signed number of units; negative values move backward.
        units : TimeUnit or str
            Calendar unit: ``Days``, ``Weeks``, ``Months``, or ``Years``,
            supplied as a ``TimeUnit`` or its name.

        Returns
        -------
        Date
            Shifted calendar date.
        """
        ...

    def add_period(self, period: _PeriodLike) -> Date:
        """Add a tenor without business-day adjustment.

        Parameters
        ----------
        period : Period or str
            Calendar interval, as a ``Period`` or tenor string such as
            ``"6M"`` or ``"1Y6M"``.

        Returns
        -------
        Date
            Date after adding the interval.
        """
        ...

    def to_str(self, fmt: str=...) -> str:
        """Format the date as text.

        Parameters
        ----------
        fmt : str, optional
            Chrono ``strftime`` format; defaults to ``"%Y-%m-%d"``.

        Returns
        -------
        str
            Date rendered with the requested format.
        """
        ...

    def to_datetime(self) -> PythonDate:
        """Convert to the standard Python date type.

        Returns
        -------
        datetime.date
            Date with the same year, month, and day, without a time or time zone.
        """
        ...

    @property
    def year(self) -> int:
        """Read-only calendar year."""
        ...

    @property
    def day(self) -> int:
        """Read-only day of the month, starting at 1."""
        ...

    @property
    def month(self) -> int:
        """Read-only month number, from 1 (January) to 12 (December)."""
        ...

@final
class Period:
    """A financial period such as `5Y`, `6M` or `1Y6M`.

Wherever the bindings expect a period, a [`Period`] or its string form
may be passed."""

    def __new__(cls, length: int, units: TimeUnit | str) -> Self:
        """Create a calendar interval.

        Parameters
        ----------
        length : int
            Signed number of units in the interval.
        units : TimeUnit or str
            Unit enum or case-insensitive name: ``Days``, ``Weeks``,
            ``Months``, or ``Years``.
        """
        ...

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Period:
        """Parse a financial tenor.

        Parameters
        ----------
        s : str
            Native period text, such as ``"5Y"``, ``"6M"``, or ``"1Y6M"``;
            unit suffixes identify years, months, weeks, or days.

        Returns
        -------
        Period
            Parsed interval, without an anchor date or holiday calendar.
        """
        ...

    @staticmethod
    def from_frequency(freq: Frequency) -> Period:
        """Convert a recurring frequency to its calendar interval.

        Parameters
        ----------
        freq : Frequency
            Typed frequency, such as ``Frequency.Quarterly``; this method
            does not accept a string. Frequencies without a period equivalent
            raise an error.

        Returns
        -------
        Period
            Interval corresponding to the frequency, such as three months
            for ``Quarterly``.
        """
        ...

    def frequency(self) -> Frequency:
        """Return the payment frequency represented by this interval.

        Returns
        -------
        Frequency
            Native frequency classification for this period.
        """
        ...

    @property
    def length(self) -> int:
        """Read-only number of time units in the period."""
        ...

    @property
    def units(self) -> TimeUnit:
        """Read-only calendar unit associated with the period's length."""
        ...

@final
class UnitedStatesMarket:
    """United States calendar market."""
    Settlement: ClassVar[UnitedStatesMarket]
    LiborImpact: ClassVar[UnitedStatesMarket]
    Nyse: ClassVar[UnitedStatesMarket]
    GovernmentBond: ClassVar[UnitedStatesMarket]
    Nerc: ClassVar[UnitedStatesMarket]
    FederalReserve: ClassVar[UnitedStatesMarket]
    Sofr: ClassVar[UnitedStatesMarket]

    @staticmethod
    def parse(s: str) -> UnitedStatesMarket:
        """Parse a United States holiday-calendar market.

        Parameters
        ----------
        s : str
            Case-insensitive member name: ``Settlement``, ``LiborImpact``,
            ``Nyse``, ``GovernmentBond``, ``Nerc``, ``FederalReserve``, or ``Sofr``.

        Returns
        -------
        UnitedStatesMarket
            Market selector for ``Calendar.UnitedStates``.
        """
        ...

    def __str__(self) -> str:
        ...

    def __repr__(self) -> str:
        ...

@final
class BrazilMarket:
    """Brazil calendar market."""
    Settlement: ClassVar[BrazilMarket]
    Exchange: ClassVar[BrazilMarket]

    @staticmethod
    def parse(s: str) -> BrazilMarket:
        """Parse a Brazilian holiday-calendar market.

        Parameters
        ----------
        s : str
            Case-insensitive name: ``Settlement`` or ``Exchange``.

        Returns
        -------
        BrazilMarket
            Market selector for ``Calendar.Brazil``.
        """
        ...

    def __str__(self) -> str:
        ...

    def __repr__(self) -> str:
        ...

@final
class JointCalendarRule:
    """Business-day combination rule for a joint calendar."""
    JoinHolidays: ClassVar[JointCalendarRule]
    JoinBusinessDays: ClassVar[JointCalendarRule]

    @staticmethod
    def parse(s: str) -> JointCalendarRule:
        """Parse a joint-calendar combination rule.

        Parameters
        ----------
        s : str
            Case-insensitive ``JoinHolidays`` (all calendars must be open)
            or ``JoinBusinessDays`` (at least one calendar must be open).

        Returns
        -------
        JointCalendarRule
            Matching business-day combination rule.
        """
        ...

    def __str__(self) -> str:
        ...

    def __repr__(self) -> str:
        ...

@final
class Schedule:
    """Immutable native schedule, including business-day adjustment and stubs.

    Defaults mirror ``MakeSchedule``: zero tenor, ``NullCalendar``, both
    conventions ``Unadjusted``, ``Backward`` generation, and no end-of-month
    preservation. A zero tenor forces ``Zero`` and normalizes to zero years.
    Construction failures raise ``QuantSupportError`` rather than escaping
    native panics. This is not a mutable schedule builder.
    """

    def __new__(cls, effective_date: _DateLike, termination_date: _DateLike,
                tenor: _PeriodLike | None = None, *, frequency: Frequency | None = None,
                calendar: Calendar | None = None,
                convention: BusinessDayConvention | None = None,
                termination_date_convention: BusinessDayConvention | None = None,
                rule: DateGenerationRule | None = None, end_of_month: bool = False,
                first_date: _DateLike | None = None,
                next_to_last_date: _DateLike | None = None) -> Self:
        """Generate and adjust a sequence of native dates.

        Parameters
        ----------
        effective_date : Date, datetime.date, or str
            Unadjusted start, strictly before termination, in years
            1902--2198. Strings use ``YYYY-MM-DD``.
        termination_date : Date, datetime.date, or str
            Unadjusted end, in years 1902--2198.
        tenor : Period, str, or None, optional
            Nonnegative interval, such as ``"3M"``. Mutually exclusive with
            ``frequency``. Both omitted means a zero-tenor two-date schedule.
            Trial dates beyond each endpoint must fit 1901--2199: the wrapper
            reserves ceil(length/365) years for days, ceil(7*length/365) for
            weeks, ceil(length/12) for months, or length for years.
        frequency : Frequency or None, optional
            Typed alternative to ``tenor``. ``OtherFrequency`` is rejected;
            ``Once`` and ``NoFrequency`` produce zero-tenor schedules.
        calendar : Calendar or None, optional
            Adjustment calendar, copied with its overrides. Defaults to
            ``Calendar.NullCalendar``.
        convention : BusinessDayConvention or None, optional
            Typed effective/intermediate-date rule; defaults to ``Unadjusted``.
        termination_date_convention : BusinessDayConvention or None, optional
            Independent typed final-date rule; defaults to ``Unadjusted``,
            not to ``convention``. Native CDS/CDS2015 rules leave the final
            date unadjusted regardless of this setting.
        rule : DateGenerationRule or None, optional
            Typed generation rule; defaults to ``Backward``. Zero tenor
            overrides it with ``Zero``. Special IMM/CDS rules follow the
            native builder, including twentieth-date rolling.
        end_of_month : bool, optional
            Preserve month-end when the native generation seed is month-end.
            Defaults to ``False``. Rejected for Zero/zero-tenor schedules;
            native generation also rejects it for rules other than
            ``Backward`` and ``Forward``.
        first_date : Date, datetime.date, str, or None, optional
            Explicit first stub anchor, strictly inside the unadjusted range.
            Must precede ``next_to_last_date`` if both are supplied. The
            native builder checks compatibility with the generation rule.
        next_to_last_date : Date, datetime.date, str, or None, optional
            Explicit final stub anchor, strictly inside the range. Native
            ThirdWednesday stubs must be monthly IMM dates; other special
            rules reject explicit stubs.

        Raises
        ------
        QuantSupportError
            Invalid ordering, tenor, bounds, incompatible settings, or fewer
            than two increasing dates after business-day adjustment.
        """
        ...

    @property
    def dates(self) -> list[Date]:
        """Read the generated, adjusted date sequence.

        Returns
        -------
        list of Date
            Independent list including both generated endpoints, which may
            differ from the supplied endpoints under IMM/CDS rules.
        """
        ...

    @property
    def is_regular(self) -> list[bool]:
        """Read native regularity flags.

        Returns
        -------
        list of bool
            Independent list with one entry per consecutive date interval.
            False identifies a native irregular/stub period.
        """
        ...

    @property
    def tenor(self) -> Period:
        """Read the effective tenor.

        Returns
        -------
        Period
            Configured tenor, normalized to zero years for Zero schedules.
        """
        ...

    @property
    def calendar(self) -> Calendar:
        """Read the adjustment calendar.

        Returns
        -------
        Calendar
            Independent copy, preserving its native holiday overrides.
        """
        ...

    @property
    def convention(self) -> BusinessDayConvention:
        """Read the effective/intermediate-date convention.

        Returns
        -------
        BusinessDayConvention
            Configured rule, defaulting to ``Unadjusted``.
        """
        ...

    @property
    def termination_date_convention(self) -> BusinessDayConvention:
        """Read the separately configured final-date convention.

        Returns
        -------
        BusinessDayConvention
            Configured rule, defaulting independently to ``Unadjusted``.
        """
        ...

    @property
    def rule(self) -> DateGenerationRule:
        """Read the effective generation rule.

        Returns
        -------
        DateGenerationRule
            Configured rule, or ``Zero`` when the tenor has zero length.
        """
        ...

    @property
    def end_of_month(self) -> bool:
        """Read the end-of-month setting.

        Returns
        -------
        bool
            Whether end-of-month preservation was requested.
        """
        ...

    @property
    def first_date(self) -> Date | None:
        """Read the explicit first stub anchor, not the first generated date.

        Returns
        -------
        Date or None
            Unadjusted supplied anchor, or None when omitted.
        """
        ...

    @property
    def next_to_last_date(self) -> Date | None:
        """Read the explicit final stub anchor.

        Returns
        -------
        Date or None
            Unadjusted supplied anchor, or None when omitted.
        """
        ...

    def __len__(self) -> int:
        """Count generated dates.

        Returns
        -------
        int
            Date count, including both endpoints.
        """
        ...

    def __getitem__(self, index: int, /) -> Date:
        """Read a generated date by integer index.

        Parameters
        ----------
        index : int
            Zero-based position; negative values count from the end.
            Slices are not supported. Out-of-range indices raise IndexError.

        Returns
        -------
        Date
            Date at the requested position.
        """
        ...

    def __iter__(self) -> Iterator[Date]:
        """Iterate over a snapshot of generated dates.

        Returns
        -------
        Iterator of Date
            Independent iterator in chronological order.
        """
        ...

@final
class IMM:
    """Static International Monetary Market date and code utilities.

    IMM dates are third Wednesdays. Main-cycle queries default to True and
    restrict months to March, June, September and December. Monthly codes use
    F/G/H/J/K/M/N/Q/U/V/X/Z and one year digit, accepted case-insensitively.
    Invalid conversions raise ``QuantSupportError``. Reference dates are
    explicit; no system date is silently substituted.
    """

    @staticmethod
    def is_imm_date(date: _DateLike, main_cycle: bool = True) -> bool:
        """Test whether a date belongs to an IMM cycle.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Candidate date; strings use ``YYYY-MM-DD``.
        main_cycle : bool, optional
            True checks quarterly months only; False checks all months.

        Returns
        -------
        bool
            Whether the date is the third Wednesday of an allowed month.
        """
        ...

    @staticmethod
    def is_imm_code(code: str, main_cycle: bool = True) -> bool:
        """Validate an IMM code without raising on malformed text.

        Parameters
        ----------
        code : str
            Exactly one ASCII month letter and one ASCII digit. Empty,
            overlong and non-ASCII strings return False.
        main_cycle : bool, optional
            True permits only H/M/U/Z; False permits every monthly letter.

        Returns
        -------
        bool
            Whether the case-insensitive code matches the selected cycle.
        """
        ...

    @staticmethod
    def code(imm_date: _DateLike) -> str:
        """Encode a monthly IMM date.

        Parameters
        ----------
        imm_date : Date, datetime.date, or str
            Third Wednesday in any month, with year 1--9999. Invalid IMM
            dates raise QuantSupportError. Strings use ``YYYY-MM-DD``.

        Returns
        -------
        str
            Uppercase month letter and final year digit; the decade is lost.
        """
        ...

    @staticmethod
    def date(imm_code: str, reference_date: _DateLike) -> Date:
        """Resolve an IMM code using an explicit reference date.

        Parameters
        ----------
        imm_code : str
            Case-insensitive monthly IMM code; need not be main-cycle.
        reference_date : Date, datetime.date, or str
            Nonempty date with year 1--9989, leaving room for decade
            resolution within Python's year range. Strings use ``YYYY-MM-DD``.

        Returns
        -------
        Date
            First matching third Wednesday on or after the reference date.
            The native special case for year-digit zero and references up
            to 1909 is preserved.
        """
        ...

    @staticmethod
    def next_date(reference_date: _DateLike, main_cycle: bool = True) -> Date:
        """Find the next IMM date strictly after a date.

        Parameters
        ----------
        reference_date : Date, datetime.date, or str
            Nonempty date with year 1--9998, allowing a one-year rollover.
            Strings use ``YYYY-MM-DD``.
        main_cycle : bool, optional
            True uses quarterly months; False uses all months.

        Returns
        -------
        Date
            Next third Wednesday in the selected cycle, excluding the input.
        """
        ...

    @staticmethod
    def next_date_with_code(imm_code: str, reference_date: _DateLike,
                            main_cycle: bool = True) -> Date:
        """Find the next IMM date after resolving a code.

        Parameters
        ----------
        imm_code : str
            Case-insensitive monthly code to resolve and step beyond.
        reference_date : Date, datetime.date, or str
            Nonempty resolution date with year 1--9989; strings use
            ``YYYY-MM-DD``. The starting code can be outside the main cycle.
            A next date beyond year 9999 raises QuantSupportError.
        main_cycle : bool, optional
            Select quarterly (True) or monthly (False) output dates.

        Returns
        -------
        Date
            Next IMM date after the resolved contract, not after the reference.
        """
        ...

    @staticmethod
    def next_code(reference_date: _DateLike, main_cycle: bool = True) -> str:
        """Encode the next IMM date strictly after a date.

        Parameters
        ----------
        reference_date : Date, datetime.date, or str
            Nonempty date with year 1--9998; strings use ``YYYY-MM-DD``.
        main_cycle : bool, optional
            Select quarterly (True) or monthly (False) output contracts.

        Returns
        -------
        str
            Uppercase code for ``next_date(reference_date, main_cycle)``.
        """
        ...

    @staticmethod
    def next_code_with_code(imm_code: str, reference_date: _DateLike,
                            main_cycle: bool = True) -> str:
        """Encode the next contract after resolving an IMM code.

        Parameters
        ----------
        imm_code : str
            Case-insensitive monthly code to resolve and step beyond.
        reference_date : Date, datetime.date, or str
            Nonempty resolution date with year 1--9989; strings use
            ``YYYY-MM-DD``.
            A next date beyond year 9999 raises QuantSupportError.
        main_cycle : bool, optional
            Select quarterly (True) or monthly (False) output contracts.

        Returns
        -------
        str
            Uppercase code for the next IMM date after the resolved contract.
        """
        ...

@final
class Calendar:
    """Holiday and weekend rules for business-day calculations.

    Fixed calendars are available as typed attributes, including ``TARGET``,
    ``WeekendsOnly``, and ``NullCalendar``. Country and regional calendar factories
    take typed market selectors; joint calendars combine existing calendars.
    The string constructor accepts exact native calendar names.
    Holiday overrides return independent calendars and never mutate shared
    class attributes or calendars previously supplied to schedules or joints.
    """
    NullCalendar: ClassVar[Calendar]
    WeekendsOnly: ClassVar[Calendar]
    Switzerland: ClassVar[Calendar]
    Norway: ClassVar[Calendar]
    Sweden: ClassVar[Calendar]
    Denmark: ClassVar[Calendar]
    SouthAfrica: ClassVar[Calendar]
    TARGET: ClassVar[Calendar]
    UnitedKingdom: ClassVar[Calendar]
    Japan: ClassVar[Calendar]
    Chile: ClassVar[Calendar]

    @staticmethod
    def Australia(*, market: AustraliaMarket | None = ...) -> Calendar:
        """Create an Australian calendar.

        Parameters
        ----------
        market : AustraliaMarket or None, optional
            Typed market or regional selector. None selects ``Settlement``.

        Returns
        -------
        Calendar
            Calendar observing the selected market's holidays.
        """
        ...
    @staticmethod
    def Canada(*, market: CanadaMarket | None = ...) -> Calendar:
        """Create a Canadian calendar.

        Parameters
        ----------
        market : CanadaMarket or None, optional
            Typed market or regional selector. None selects ``Settlement``.

        Returns
        -------
        Calendar
            Calendar observing the selected market's holidays.
        """
        ...
    @staticmethod
    def NewZealand(*, market: NewZealandMarket | None = ...) -> Calendar:
        """Create a New Zealand calendar.

        Parameters
        ----------
        market : NewZealandMarket or None, optional
            Typed market or regional selector. None selects ``Wellington``.

        Returns
        -------
        Calendar
            Calendar observing the selected market's holidays.
        """
        ...
    @staticmethod
    def SouthKorea(*, market: SouthKoreaMarket | None = ...) -> Calendar:
        """Create a South Korean calendar.

        Parameters
        ----------
        market : SouthKoreaMarket or None, optional
            Typed market or regional selector. None selects ``Settlement``.

        Returns
        -------
        Calendar
            Calendar observing the selected market's holidays.
        """
        ...
    @staticmethod
    def China(*, market: ChinaMarket | None = ...) -> Calendar:
        """Create a Chinese calendar.

        Parameters
        ----------
        market : ChinaMarket or None, optional
            Typed market or regional selector. None selects ``SSE``.

        Returns
        -------
        Calendar
            Calendar observing the selected market's holidays.
        """
        ...
    @staticmethod
    def UnitedStates(*, market: UnitedStatesMarket | None=...) -> Calendar:
        """Create a United States market calendar.

        Parameters
        ----------
        market : UnitedStatesMarket or None, optional
            Typed holiday-market selector. Omitted or ``None`` uses ``Sofr``;
            strings are not accepted by this factory.

        Returns
        -------
        Calendar
            Calendar observing the selected United States market's holidays.
        """
        ...

    @staticmethod
    def Brazil(*, market: BrazilMarket | None=...) -> Calendar:
        """Create a Brazilian market calendar.

        Parameters
        ----------
        market : BrazilMarket or None, optional
            Typed holiday-market selector. Omitted or ``None`` uses
            ``Settlement``; strings are not accepted by this factory.

        Returns
        -------
        Calendar
            Calendar observing the selected Brazilian market's holidays.
        """
        ...

    @staticmethod
    def JointCalendar(*, calendars: Sequence[Calendar], rule: JointCalendarRule | None=...) -> Calendar:
        """Combine two or more business-day calendars.

        Parameters
        ----------
        calendars : sequence of Calendar
            At least two typed calendars; calendar-name strings are not
            accepted as constituent values by this factory.
        rule : JointCalendarRule or None, optional
            Typed combination rule. Omitted or ``None`` uses ``JoinHolidays``:
            all constituents must be open. ``JoinBusinessDays`` requires only
            one constituent to be open.

        Returns
        -------
        Calendar
            Calendar applying the selected rule to all constituents.
        """
        ...

    def __new__(cls, name: str) -> Self:
        """Create a calendar from an exact native serialized name.

        Parameters
        ----------
        name : str
            Case-sensitive native name: ``NullCalendar``, ``WeekendsOnly``,
            ``TARGET``, ``Japan``, ``UnitedKingdom(Settlement)``, ``Chile(SSE)``,
            ``UnitedStates(Sofr)`` (or another supported United States market),
            ``Brazil(Settlement)``, or ``Brazil(Exchange)``. Joint names use
            ``JointCalendar(JoinHolidays,TARGET,Japan)`` or the
            ``JoinBusinessDays`` rule and may contain nested joint calendars.
            Bare ``UnitedStates`` and ``Brazil`` are not serialized names.
        """
        ...

    def __repr__(self) -> str:
        ...

    def name(self) -> str:
        """Return the calendar's native serialized name.

        Returns
        -------
        str
            Name including market or joint-calendar rule where applicable.
        """
        ...

    def is_business_day(self, date: _DateLike) -> bool:
        """Test whether the calendar is open on a date.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Date to test; strings use ``YYYY-MM-DD``.

        Returns
        -------
        bool
            Whether the date is a business day under this calendar.
        """
        ...

    def is_holiday(self, date: _DateLike) -> bool:
        """Test whether a date is a non-business day.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Date to test; strings use ``YYYY-MM-DD``.

        Returns
        -------
        bool
            Whether the calendar is closed, including its weekends.
        """
        ...

    def business_days_between(self, from_: _DateLike, /, to: _DateLike,
                              include_first: bool = True, include_last: bool = False) -> int:
        """Count business days with independently inclusive endpoints.

        Parameters
        ----------
        from_ : Date, datetime.date, or str
            Starting date; strings use ``YYYY-MM-DD``. The runtime keyword
            is ``from``; use positional arguments or a keyword dictionary.
        to : Date, datetime.date, or str
            Ending date in the native calendar's supported holiday range.
        include_first : bool, optional
            Include the starting date when it is a business day; default True.
        include_last : bool, optional
            Include the ending date when it is a business day; default False.

        Returns
        -------
        int
            Signed business-day count; reversed bounds give a negative count.
        """
        ...

    def holiday_list(self, from_: _DateLike, /, to: _DateLike,
                     include_weekends: bool = False) -> list[Date]:
        """List holidays in a closed date interval.

        Parameters
        ----------
        from_ : Date, datetime.date, or str
            First date in the native holiday range. The runtime keyword is
            ``from``; use positional arguments or a keyword dictionary.
        to : Date, datetime.date, or str
            Last date; strings use ``YYYY-MM-DD``. Reversed bounds yield [].
        include_weekends : bool, optional
            Include days classified as weekends by the calendar; default False.

        Returns
        -------
        list of Date
            Native holiday-list result. TARGET and joint calendars account
            for local overrides; WeekendsOnly returns an empty list even
            when weekends or explicit overrides fall within the interval.
        """
        ...

    def business_day_list(self, from_: _DateLike, /, to: _DateLike) -> list[Date]:
        """List business days in a closed date interval.

        Parameters
        ----------
        from_ : Date, datetime.date, or str
            First date in the native holiday range. The runtime keyword is
            ``from``; use positional arguments or a keyword dictionary.
        to : Date, datetime.date, or str
            Last date; strings use ``YYYY-MM-DD``. Reversed bounds yield [].

        Returns
        -------
        list of Date
            Native business-day-list result. TARGET and joint calendars
            account for overrides; WeekendsOnly returns an empty list.
        """
        ...

    def with_added_holiday(self, date: _DateLike) -> Calendar:
        """Return a copy with an explicit holiday override.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Date to close, in years 1902 through 2198; strings use
            ``YYYY-MM-DD``. Adds an explicit closure even for a date that
            was already a holiday. Added holidays take precedence over
            removed holidays in fixed calendars.

        Returns
        -------
        Calendar
            Independent calendar. The original and fixed class attributes
            remain unchanged, even when the date is already a holiday.
        """
        ...

    def with_removed_holiday(self, date: _DateLike) -> Calendar:
        """Return a copy with an explicit business-day override.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Date to open, in years 1902 through 2198; strings use
            ``YYYY-MM-DD``. Can open a weekend or statutory holiday, unless
            the fixed calendar also has an added-holiday override for it.
            Native joint calendars cancel the opposite override; fixed
            calendars retain both sets and added holidays take precedence.

        Returns
        -------
        Calendar
            Independent calendar with the override; the original is unchanged.
        """
        ...

    @property
    def added_holidays(self) -> list[Date]:
        """Inspect explicit holiday overrides on this calendar.

        Returns
        -------
        list of Date
            Fresh, chronologically sorted list. Does not enumerate statutory
            holidays or constituent calendars' overrides in a joint calendar.
        """
        ...

    @property
    def removed_holidays(self) -> list[Date]:
        """Inspect explicit business-day overrides on this calendar.

        Returns
        -------
        list of Date
            Fresh, chronologically sorted list. Changing it has no effect on
            this calendar or the calendars used to construct a joint calendar.
        """
        ...

    def end_of_month(self, date: _DateLike) -> Date:
        """Find the business month-end, accounting for holiday overrides.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Date identifying the month, in years 1902 through 2198; strings
            use ``YYYY-MM-DD``. This range leaves adjustment room inside the
            native 1901--2199 Easter table.

        Returns
        -------
        Date
            Calendar month-end adjusted with ``Preceding``.
        """
        ...

    def is_end_of_month(self, date: _DateLike) -> bool:
        """Apply the native business month-end predicate.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Date to test, in years 1902 through 2198; strings use
            ``YYYY-MM-DD``.

        Returns
        -------
        bool
            Whether following adjustment of the next day crosses the month
            boundary. This can be true for a non-business date after the last
            business day; it is not a business-day check.
        """
        ...

    def is_weekend(self, date: _DateLike) -> bool:
        """Test this calendar's weekend rule for a date's weekday.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Date whose weekday is tested; strings use ``YYYY-MM-DD``.
            No holiday-table lookup or holiday overrides are used.

        Returns
        -------
        bool
            Native weekend classification. ``NullCalendar`` has no weekends;
            joint calendars use their native any/all combination rule.
        """
        ...

    def adjust(self, date: _DateLike, convention: BusinessDayConvention | str | None=...) -> Date:
        """Apply a business-day adjustment rule to a date.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Unadjusted date; strings use ``YYYY-MM-DD``.
        convention : BusinessDayConvention, str, or None, optional
            Typed rule or case-insensitive member name. Omitted or ``None``
            uses ``Following``. ``Unadjusted`` leaves the date unchanged.

        Returns
        -------
        Date
            Date after applying the selected adjustment.
        """
        ...

    def advance(self, date: _DateLike, period: _PeriodLike, convention: BusinessDayConvention | str | None=..., end_of_month: bool=...) -> Date:
        """Advance a date with this calendar's business-day rules.

        Parameters
        ----------
        date : Date, datetime.date, or str
            Starting date; strings use ``YYYY-MM-DD``.
        period : Period or str
            Signed interval, such as ``"2D"``, ``"3M"``, or ``"1Y"``.
            Day units count business days; other units advance the calendar
            date and then apply the adjustment convention.
        convention : BusinessDayConvention, str, or None, optional
            Typed rule or case-insensitive member name; omitted or ``None``
            uses ``Following``.
        end_of_month : bool, optional
            Preserve business month-end when advancing by months or years
            from a business month-end. Defaults to ``False``.

        Returns
        -------
        Date
            Advanced, business-day-adjusted date.
        """
        ...

@final
class AustraliaMarket:
    """Australia market or regional calendar selection."""
    Settlement: ClassVar[AustraliaMarket]
    ASX: ClassVar[AustraliaMarket]
    @staticmethod
    def parse(s: str) -> AustraliaMarket:
        """Parse a market name, case-insensitively.

        Parameters
        ----------
        s : str
            Market or regional variant name.

        Returns
        -------
        AustraliaMarket
            Matching typed market selector.
        """
        ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

@final
class CanadaMarket:
    """Canada market or regional calendar selection."""
    Settlement: ClassVar[CanadaMarket]
    TSX: ClassVar[CanadaMarket]
    @staticmethod
    def parse(s: str) -> CanadaMarket:
        """Parse a market name, case-insensitively.

        Parameters
        ----------
        s : str
            Market or regional variant name.

        Returns
        -------
        CanadaMarket
            Matching typed market selector.
        """
        ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

@final
class NewZealandMarket:
    """NewZealand market or regional calendar selection."""
    Wellington: ClassVar[NewZealandMarket]
    Auckland: ClassVar[NewZealandMarket]
    @staticmethod
    def parse(s: str) -> NewZealandMarket:
        """Parse a market name, case-insensitively.

        Parameters
        ----------
        s : str
            Market or regional variant name.

        Returns
        -------
        NewZealandMarket
            Matching typed market selector.
        """
        ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

@final
class SouthKoreaMarket:
    """SouthKorea market or regional calendar selection."""
    Settlement: ClassVar[SouthKoreaMarket]
    KRX: ClassVar[SouthKoreaMarket]
    @staticmethod
    def parse(s: str) -> SouthKoreaMarket:
        """Parse a market name, case-insensitively.

        Parameters
        ----------
        s : str
            Market or regional variant name.

        Returns
        -------
        SouthKoreaMarket
            Matching typed market selector.
        """
        ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

@final
class ChinaMarket:
    """China market or regional calendar selection."""
    SSE: ClassVar[ChinaMarket]
    IB: ClassVar[ChinaMarket]
    @staticmethod
    def parse(s: str) -> ChinaMarket:
        """Parse a market name, case-insensitively.

        Parameters
        ----------
        s : str
            Market or regional variant name.

        Returns
        -------
        ChinaMarket
            Matching typed market selector.
        """
        ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

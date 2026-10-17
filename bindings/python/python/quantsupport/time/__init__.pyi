"""Dates, tenors, calendars, and accrual conventions."""
from datetime import date as PythonDate
from typing import Any, ClassVar, Mapping, Sequence, final, overload
from typing_extensions import Self, disjoint_base
from pandas import DataFrame

_DateLike = Date | str | PythonDate
_PeriodLike = Period | str

__all__ = ['Date', 'Period', 'Calendar', 'UnitedStatesMarket', 'BrazilMarket', 'JointCalendarRule', 'Compounding', 'Frequency', 'DayCounter', 'TimeUnit', 'BusinessDayConvention', 'DateGenerationRule']

@final
class Compounding:
    """Interest compounding convention."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Compounding:
        """Parses a compounding convention from its name."""
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
        """Parses a frequency from its name."""
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
        """Parses a day counter from its name."""
        ...

    def day_count(self, start: _DateLike, end: _DateLike) -> Any:
        """Number of days between two dates under this convention."""
        ...

    def year_fraction(self, start: _DateLike, end: _DateLike) -> float:
        """Year fraction between two dates under this convention."""
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
        """Parses a time unit from its name."""
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
        """Parses a convention from its name."""
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
        """Parses a schedule date-generation rule from its name."""
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
    """A calendar date.

Wherever the bindings expect a date, a [`Date`], an ISO string
(`"2025-11-11"`) or a `datetime.date` may be passed."""

    def __new__(cls, year: int, month: int, day: int) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    def __add__(self, value: int | _PeriodLike, /) -> Date:
        ...

    @overload
    def __sub__(self, value: Date, /) -> int:
        ...

    @overload
    def __sub__(self, value: int | _PeriodLike, /) -> Date:
        ...

    @staticmethod
    def parse(s: str, fmt: str=...) -> Date:
        """Parses a date string; `fmt` follows chrono's `strftime` syntax."""
        ...

    def weekday(self) -> str:
        """Day of the week (e.g. `"Monday"`)."""
        ...

    def days_in_month(self) -> int:
        """Number of days in this date's month."""
        ...

    def day_of_year(self) -> int:
        """Day of the year (1-based)."""
        ...

    def is_leap_year(self) -> bool:
        """Whether the date falls in a leap year."""
        ...

    def end_of_month(self) -> Date:
        """Last day of this date's month."""
        ...

    def advance(self, n: int, units: TimeUnit | str) -> Date:
        """Advances the date by `n` units (`TimeUnit` or string)."""
        ...

    def add_period(self, period: _PeriodLike) -> Date:
        """Adds a [`Period`] (or period string like `"6M"`)."""
        ...

    def to_str(self, fmt: str=...) -> str:
        """Formats the date; `fmt` follows chrono's `strftime` syntax."""
        ...

    def to_datetime(self) -> PythonDate:
        """Converts to a Python `datetime.date`."""
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
        ...

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Period:
        """Parses a period string such as `"5Y"`, `"6M"` or `"1Y6M"`."""
        ...

    @staticmethod
    def from_frequency(freq: Frequency) -> Period:
        """Builds the period equivalent to a [`Frequency`]."""
        ...

    def frequency(self) -> Frequency:
        """The [`Frequency`] equivalent of this period."""
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
        """Parses a United States holiday-calendar market from its name."""
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
        """Parses a Brazilian holiday-calendar market from its name."""
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
        """Parses the holiday or business-day combination rule from its name."""
        ...

    def __str__(self) -> str:
        ...

    def __repr__(self) -> str:
        ...

@final
class Calendar:
    """A business-day calendar.

Native calendar variants, for example ``Calendar.TARGET``.

Fixed calendars are typed class attributes. UnitedStates and Brazil accept
typed market enums; JointCalendar combines typed calendars and a rule.
The string constructor remains available for loading serialized names."""
    NullCalendar: ClassVar[Calendar]
    WeekendsOnly: ClassVar[Calendar]
    TARGET: ClassVar[Calendar]
    UnitedKingdom: ClassVar[Calendar]
    Japan: ClassVar[Calendar]
    Chile: ClassVar[Calendar]

    @staticmethod
    def UnitedStates(*, market: UnitedStatesMarket | None=...) -> Calendar:
        """United States calendar for the selected market.

Parameters
----------
market : UnitedStatesMarket, optional
    Defaults to Sofr."""
        ...

    @staticmethod
    def Brazil(*, market: BrazilMarket | None=...) -> Calendar:
        """Brazil calendar for the selected market.

Parameters
----------
market : BrazilMarket, optional
    Defaults to Settlement."""
        ...

    @staticmethod
    def JointCalendar(*, calendars: Sequence[Calendar], rule: JointCalendarRule | None=...) -> Calendar:
        """Combine native calendars.

Parameters
----------
calendars : sequence of Calendar
    At least two constituent calendars.
rule : JointCalendarRule, optional
    Defaults to JoinHolidays: every constituent must be open."""
        ...

    def __new__(cls, name: str) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    def name(self) -> str:
        """Calendar name."""
        ...

    def is_business_day(self, date: _DateLike) -> bool:
        """Whether the date is a business day."""
        ...

    def is_holiday(self, date: _DateLike) -> bool:
        """Whether the date is a holiday (including weekends)."""
        ...

    def adjust(self, date: _DateLike, convention: BusinessDayConvention | str | None=...) -> Date:
        """Adjusts a date to a business day under the given convention
(default `Following`)."""
        ...

    def advance(self, date: _DateLike, period: _PeriodLike, convention: BusinessDayConvention | str | None=..., end_of_month: bool=...) -> Date:
        """Advances a date by a period, adjusting for business days."""
        ...

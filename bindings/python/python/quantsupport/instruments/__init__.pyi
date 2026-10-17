"""Product specifications, coupons, loan terms, and payoff scripts."""
from datetime import date as PythonDate
from typing import Any, ClassVar, Mapping, Sequence, final, overload
from typing_extensions import Self, disjoint_base
from pandas import DataFrame
from ..time import BusinessDayConvention, Calendar, Compounding, Date, DateGenerationRule, DayCounter, Frequency, _DateLike, _PeriodLike
from ..market import Currency, MarketIndex, _CurrencyLike, _IndexLike
__all__ = ['Instrument', 'Side', 'OptionType', 'SwaptionType', 'CapFloorType', 'CapletFloorletType', 'PaymentStructure', 'LoanRepaymentStructure', 'PrepaymentModelConfiguration', 'Loan', 'EquityForward', 'EuropeanSwaption', 'Swap', 'CrossCurrencySwap', 'BasisSwap', 'FixFloatCrossCurrencySwap', 'FixedRateBond', 'InterestRate', 'PayoffOps', 'FixedRateCoupon', 'FloatingRateCoupon', 'OptionEmbeddedCoupon', 'CallScheduleEntry', 'SteppedCouponBond', 'FloatingRateNote', 'FixedRateDeposit', 'FxForward', 'FxOption', 'EquityOption', 'CreditDefaultSwap', 'CapFloor', 'CapletFloorlet', 'RateFutures', 'ScriptedProduct']

@final
class Side:
    """Direction of a trade's cashflows."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> Side:
        """Parses a side; accepts `"LongReceive"`/`"Long"`/`"Receive"` and
`"PayShort"`/`"Pay"`/`"Short"`."""
        ...

    def sign(self) -> float:
        """Cashflow sign: +1 for `LongReceive`, -1 for `PayShort`."""
        ...
    LongReceive: ClassVar[Side]
    PayShort: ClassVar[Side]

@final
class OptionType:
    """European option payoff direction (equity and FX options)."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> OptionType:
        """Parses an option type from its name."""
        ...
    Call: ClassVar[OptionType]
    Put: ClassVar[OptionType]

@final
class CapFloorType:
    """Cap or floor direction."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> CapFloorType:
        """Parses a cap/floor type from its name."""
        ...
    Cap: ClassVar[CapFloorType]
    Floor: ClassVar[CapFloorType]

@final
class CapletFloorletType:
    """Caplet or floorlet payoff direction."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> CapletFloorletType:
        """Parses a caplet/floorlet type from its name."""
        ...
    Caplet: ClassVar[CapletFloorletType]
    Floorlet: ClassVar[CapletFloorletType]

@final
class PaymentStructure:
    """Amortization structure of a leg."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> PaymentStructure:
        """Parses a payment structure from its name."""
        ...
    Bullet: ClassVar[PaymentStructure]
    EqualPayments: ClassVar[PaymentStructure]
    EqualRedemptions: ClassVar[PaymentStructure]
    Zero: ClassVar[PaymentStructure]
    Other: ClassVar[PaymentStructure]

@disjoint_base
class Instrument:
    """Optional base class for user-defined financial products.

Subclasses may add arbitrary Python attributes and may override
`identifier`. Plain Python classes can also be registered directly."""

    def __init__(self, identifier: str, *_args: Any, **_kwargs: Any) -> None:
        ...

    def __new__(cls, identifier: str, *_args: Any, **_kwargs: Any) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @property
    def identifier(self) -> str:
        """Stable product identifier used in evaluation results and diagnostics."""
        ...

@final
class LoanRepaymentStructure:
    """Contractual principal amortization, fixed at Loan construction."""

    @staticmethod
    def french() -> LoanRepaymentStructure:
        """Use level initial instalments based on contractual accrual fractions."""
        ...

    @staticmethod
    def bullet() -> LoanRepaymentStructure:
        """Pay interest during the term and repay all principal at maturity."""
        ...

    @staticmethod
    def balloon(residual_fraction: float) -> LoanRepaymentStructure:
        """Reserve a fraction in [0, 1] of original principal for maturity."""
        ...

    @staticmethod
    def from_dict(obj: str | Mapping[str, Any]) -> LoanRepaymentStructure:
        """Deserialize unit-variant strings or tagged repayment terms."""
        ...

    def to_dict(self) -> str | dict[str, Any]:
        """Return a unit-variant string or tagged repayment dictionary."""
        ...

@final
class PrepaymentModelConfiguration:
    """Annual decimal CPR parameters applied after scheduled principal."""

    @staticmethod
    def no_prepayment() -> PrepaymentModelConfiguration:
        """Disable unscheduled principal payments."""
        ...

    @staticmethod
    def constant_annual_cpr(cpr: float) -> PrepaymentModelConfiguration:
        """Use annual CPR in [0, 1]; convert to each interval's conditional probability."""
        ...

    @staticmethod
    def dated_annual_cpr(points: Sequence[tuple[_DateLike, float]]) -> PrepaymentModelConfiguration:
        """Use strictly dated CPR points, zero before the first and flat after the last."""
        ...

    @staticmethod
    def refinancing_incentive(base_cpr: float, incentive_multiplier: float, maximum_cpr: float) -> PrepaymentModelConfiguration:
        """Use capped CPR from positive coupon-minus-refinancing incentive; needs a pricer index."""
        ...

    @staticmethod
    def from_dict(obj: str | Mapping[str, Any]) -> PrepaymentModelConfiguration:
        """Deserialize native prepayment terms; Loan construction validates constraints."""
        ...

    def to_dict(self) -> str | dict[str, Any]:
        """Return a unit-variant string or tagged prepayment dictionary."""
        ...

@disjoint_base
class Loan(Instrument):
    """Contractual cashflows and principal; valuation applies market/default/prepayment effects."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, coupons: Sequence[FixedRateCoupon | FloatingRateCoupon | OptionEmbeddedCoupon | Mapping[str, Any]], currency: _CurrencyLike, repayment: LoanRepaymentStructure | str | Mapping[str, Any] | None=..., *, payment_frequency: Frequency | str | None=..., fixing_frequency: Frequency | str | None=..., day_counter: DayCounter | str | None=..., prepayment: PrepaymentModelConfiguration | str | Mapping[str, Any] | None=..., credit_index: _IndexLike | None=..., recovery: float | None=..., fixing_lag: int | None=..., index_tenor: _PeriodLike | None=..., calendar: Calendar | str | None=..., business_day_convention: BusinessDayConvention | str | None=..., amortization_rate: float | None=...) -> Self:
        """Build a loan; floating-first amortizing schedules require an initial amortization_rate."""
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Construct from keyword-compatible terms, preserving subclasses and constructor validation."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Construct from a JSON file using the from_dict schema, preserving subclasses."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return supplied construction terms, including omitted options as None, without market state."""
        ...

    @property
    def notional(self) -> float:
        """Original principal advanced under the contract."""
        ...

    @property
    def currency(self) -> Currency:
        """Native payment currency."""
        ...

    def coupon_fixing_requests(self) -> DataFrame:
        """Return unique index/fixing-date rows with ISO fixing/start/end dates in a DataFrame."""
        ...

@final
class SwaptionType:
    """Payer or receiver European swaption."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> SwaptionType:
        """Parses the payer or receiver exercise direction from its name."""
        ...
    Payer: ClassVar[SwaptionType]
    Receiver: ClassVar[SwaptionType]

@disjoint_base
class EquityForward(Instrument):
    """Equity forward contract; its position size belongs to `Trade`."""

    def __new__(cls, identifier: str, market_index: _IndexLike, delivery_date: _DateLike, strike: float, currency: _CurrencyLike, day_counter: DayCounter | str | None=...) -> Self:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def delivery_date(self) -> Date:
        """Read-only date on which the equity forward settles."""
        ...

    @property
    def strike(self) -> float:
        """Read-only delivery price per equity unit, in the contract currency."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Read-only contractual market_index. Collections are returned as copies."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only contractual currency. Collections are returned as copies."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only contractual day_counter. Collections are returned as copies."""
        ...

@disjoint_base
class EuropeanSwaption(Instrument):
    """European option to enter a payer or receiver fixed/floating swap at expiry."""

    def __new__(cls, identifier: str, expiry_date: _DateLike, maturity_date: _DateLike, strike: float, notional: float, currency: _CurrencyLike, market_index: _IndexLike, swaption_type: SwaptionType | str | None=..., day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., rate_frequency: Frequency | str | None=..., fixed_leg_frequency: Frequency | str | None=..., floating_leg_frequency: Frequency | str | None=..., calendar: Calendar | str | None=...) -> Self:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def swaption_type(self) -> SwaptionType:
        """Read-only payer or receiver direction of the underlying fixed leg."""
        ...

    @property
    def expiry_date(self) -> Date:
        """Read-only European exercise date and start of the underlying swap."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def strike(self) -> float:
        """Read-only fixed rate of the underlying swap, expressed as an annual decimal."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only final maturity date of the underlying swap."""
        ...

    @property
    def notional(self) -> float:
        """Read-only contractual principal in the swap currency, before any Trade override."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only contractual currency. Collections are returned as copies."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Read-only contractual market_index. Collections are returned as copies."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only contractual day_counter. Collections are returned as copies."""
        ...

    @property
    def compounding(self) -> Compounding:
        """Read-only contractual compounding. Collections are returned as copies."""
        ...

    @property
    def rate_frequency(self) -> Frequency:
        """Read-only contractual rate_frequency. Collections are returned as copies."""
        ...

    @property
    def fixed_leg_frequency(self) -> Frequency:
        """Read-only contractual fixed_leg_frequency. Collections are returned as copies."""
        ...

    @property
    def floating_leg_frequency(self) -> Frequency:
        """Read-only contractual floating_leg_frequency. Collections are returned as copies."""
        ...

    @property
    def calendar(self) -> Calendar | None:
        """Read-only contractual calendar. Collections are returned as copies."""
        ...

@disjoint_base
class Swap(Instrument):
    """A vanilla fixed-vs-floating interest rate swap."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, fixed_rate: float, currency: _CurrencyLike, market_index: _IndexLike, spread: float | None=..., day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., rate_frequency: Frequency | str | None=..., fixed_leg_frequency: Frequency | str | None=..., floating_leg_frequency: Frequency | str | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only payment currency shared by the fixed and floating legs."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def notional(self) -> float:
        """Read-only contractual principal in the payment currency, before any Trade override."""
        ...

    @property
    def fixed_rate(self) -> Any:
        """Read-only annual decimal coupon rate paid on the fixed leg."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Read-only index used to project or fix the floating-leg coupons."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only contractual end date of the swap's accrual schedule."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start date of the swap's accrual schedule."""
        ...

    @property
    def spread(self) -> float | None:
        """Read-only contractual spread. Collections are returned as copies."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only contractual day_counter. Collections are returned as copies."""
        ...

    @property
    def compounding(self) -> Compounding:
        """Read-only contractual compounding. Collections are returned as copies."""
        ...

    @property
    def rate_frequency(self) -> Frequency:
        """Read-only contractual rate_frequency. Collections are returned as copies."""
        ...

    @property
    def fixed_leg_frequency(self) -> Frequency:
        """Read-only contractual fixed_leg_frequency. Collections are returned as copies."""
        ...

    @property
    def floating_leg_frequency(self) -> Frequency:
        """Read-only contractual floating_leg_frequency. Collections are returned as copies."""
        ...

@disjoint_base
class CrossCurrencySwap(Instrument):
    """A float-vs-float cross-currency swap."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, domestic_notional: float, foreign_notional: float, domestic_currency: _CurrencyLike, foreign_currency: _CurrencyLike, domestic_market_index: _IndexLike, foreign_market_index: _IndexLike, domestic_spread: float=..., foreign_spread: float=..., domestic_leg_frequency: Any | None=..., foreign_leg_frequency: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def foreign_market_index(self) -> Any:
        """Read-only index used to project or fix the foreign floating leg."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start date of the two swap legs."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only contractual maturity date of the two swap legs."""
        ...

    @property
    def foreign_currency(self) -> Any:
        """Read-only payment currency of the foreign leg and its notional."""
        ...

    @property
    def domestic_currency(self) -> Any:
        """Read-only payment currency of the domestic leg and its notional."""
        ...

    @property
    def domestic_market_index(self) -> Any:
        """Read-only index used to project or fix the domestic floating leg."""
        ...

    @property
    def domestic_notional(self) -> float:
        """Read-only contractual domestic_notional. Collections are returned as copies."""
        ...

    @property
    def foreign_notional(self) -> float:
        """Read-only contractual foreign_notional. Collections are returned as copies."""
        ...

    @property
    def domestic_spread(self) -> float:
        """Read-only contractual domestic_spread. Collections are returned as copies."""
        ...

    @property
    def foreign_spread(self) -> float:
        """Read-only contractual foreign_spread. Collections are returned as copies."""
        ...

    @property
    def domestic_leg_frequency(self) -> Frequency:
        """Read-only contractual domestic_leg_frequency. Collections are returned as copies."""
        ...

    @property
    def foreign_leg_frequency(self) -> Frequency:
        """Read-only contractual foreign_leg_frequency. Collections are returned as copies."""
        ...

@disjoint_base
class BasisSwap(Instrument):
    """A float-vs-float single-currency basis swap."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, currency: _CurrencyLike, pay_market_index: _IndexLike, receive_market_index: _IndexLike, pay_spread: Any | None=..., receive_spread: Any | None=..., pay_leg_frequency: Any | None=..., receive_leg_frequency: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start_date. Collections are returned as copies."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only contractual maturity_date. Collections are returned as copies."""
        ...

    @property
    def notional(self) -> float:
        """Read-only contractual notional. Collections are returned as copies."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only contractual currency. Collections are returned as copies."""
        ...

    @property
    def pay_market_index(self) -> MarketIndex:
        """Read-only contractual pay_market_index. Collections are returned as copies."""
        ...

    @property
    def receive_market_index(self) -> MarketIndex:
        """Read-only contractual receive_market_index. Collections are returned as copies."""
        ...

    @property
    def pay_spread(self) -> float | None:
        """Read-only contractual pay_spread. Collections are returned as copies."""
        ...

    @property
    def receive_spread(self) -> float | None:
        """Read-only contractual receive_spread. Collections are returned as copies."""
        ...

    @property
    def pay_leg_frequency(self) -> Frequency:
        """Read-only contractual pay_leg_frequency. Collections are returned as copies."""
        ...

    @property
    def receive_leg_frequency(self) -> Frequency:
        """Read-only contractual receive_leg_frequency. Collections are returned as copies."""
        ...

@disjoint_base
class FixFloatCrossCurrencySwap(Instrument):
    """A fixed-vs-float cross-currency swap."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, domestic_notional: float, foreign_notional: float, fixed_rate: float, domestic_currency: _CurrencyLike, foreign_currency: _CurrencyLike, floating_index: _IndexLike, spread: float | None=..., day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., rate_frequency: Frequency | str | None=..., domestic_leg_frequency: Any | None=..., foreign_leg_frequency: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start_date. Collections are returned as copies."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only contractual maturity_date. Collections are returned as copies."""
        ...

    @property
    def domestic_notional(self) -> float:
        """Read-only contractual domestic_notional. Collections are returned as copies."""
        ...

    @property
    def foreign_notional(self) -> float:
        """Read-only contractual foreign_notional. Collections are returned as copies."""
        ...

    @property
    def fixed_rate(self) -> float:
        """Read-only contractual fixed_rate. Collections are returned as copies."""
        ...

    @property
    def spread(self) -> float | None:
        """Read-only contractual spread. Collections are returned as copies."""
        ...

    @property
    def domestic_currency(self) -> Currency:
        """Read-only contractual domestic_currency. Collections are returned as copies."""
        ...

    @property
    def foreign_currency(self) -> Currency:
        """Read-only contractual foreign_currency. Collections are returned as copies."""
        ...

    @property
    def floating_index(self) -> MarketIndex:
        """Read-only contractual floating_index. Collections are returned as copies."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only contractual day_counter. Collections are returned as copies."""
        ...

    @property
    def compounding(self) -> Compounding:
        """Read-only contractual compounding. Collections are returned as copies."""
        ...

    @property
    def rate_frequency(self) -> Frequency:
        """Read-only contractual rate_frequency. Collections are returned as copies."""
        ...

    @property
    def domestic_leg_frequency(self) -> Frequency:
        """Read-only contractual domestic_leg_frequency. Collections are returned as copies."""
        ...

    @property
    def foreign_leg_frequency(self) -> Frequency:
        """Read-only contractual foreign_leg_frequency. Collections are returned as copies."""
        ...

@disjoint_base
class FixedRateBond(Instrument):
    """A fixed-rate bullet (or amortizing) bond."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, rate: float, currency: _CurrencyLike, discount_index: _IndexLike | None=..., day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., rate_frequency: Frequency | str | None=..., payment_frequency: Any | None=..., payment_structure: PaymentStructure | str | None=..., calendar: Calendar | str | None=..., business_day_convention: BusinessDayConvention | str | None=..., date_generation_rule: DateGenerationRule | str | None=..., end_of_month: bool | None=..., first_coupon_date: _DateLike | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serializes bond contract terms for constructor round trips."""
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start_date. Collections are returned as copies."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only contractual maturity_date. Collections are returned as copies."""
        ...

    @property
    def notional(self) -> float:
        """Read-only contractual notional. Collections are returned as copies."""
        ...

    @property
    def rate(self) -> float:
        """Read-only contractual rate. Collections are returned as copies."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only contractual currency. Collections are returned as copies."""
        ...

    @property
    def discount_index(self) -> MarketIndex | None:
        """Read-only contractual discount_index. Collections are returned as copies."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only contractual day_counter. Collections are returned as copies."""
        ...

    @property
    def compounding(self) -> Compounding:
        """Read-only contractual compounding. Collections are returned as copies."""
        ...

    @property
    def rate_frequency(self) -> Frequency:
        """Read-only contractual rate_frequency. Collections are returned as copies."""
        ...

    @property
    def payment_frequency(self) -> Frequency:
        """Read-only contractual payment_frequency. Collections are returned as copies."""
        ...

    @property
    def payment_structure(self) -> PaymentStructure:
        """Read-only contractual payment_structure. Collections are returned as copies."""
        ...

    @property
    def calendar(self) -> Calendar | None:
        """Read-only contractual calendar. Collections are returned as copies."""
        ...

    @property
    def business_day_convention(self) -> BusinessDayConvention | None:
        """Read-only contractual business_day_convention. Collections are returned as copies."""
        ...

    @property
    def date_generation_rule(self) -> DateGenerationRule | None:
        """Read-only contractual date_generation_rule. Collections are returned as copies."""
        ...

    @property
    def end_of_month(self) -> bool | None:
        """Read-only contractual end_of_month. Collections are returned as copies."""
        ...

    @property
    def first_coupon_date(self) -> Date | None:
        """Read-only contractual first_coupon_date. Collections are returned as copies."""
        ...

@final
class InterestRate:
    """An annual interest rate with the native day-count and compounding conventions.

Parameters
----------
rate : float
    Annual decimal rate, for example 0.06 for 6%.
day_counter : DayCounter, optional
    Defaults to Actual360.
compounding : Compounding, optional
    Defaults to Simple.
frequency : Frequency, optional
    Defaults to Annual."""

    def __new__(cls, *, rate: float, day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., frequency: Frequency | str | None=...) -> Self:
        ...

    @property
    def rate(self) -> float:
        """Read-only annual decimal rate; 0.06 represents 6 percent."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only convention used to measure accrual between dates."""
        ...

    @property
    def compounding(self) -> Compounding:
        """Read-only rule converting the annual rate into an accumulation factor."""
        ...

    @property
    def frequency(self) -> Frequency:
        """Read-only compounding frequency used by the rate convention."""
        ...

    def compound_factor(self, start: _DateLike, end: _DateLike) -> float:
        """Accumulation multiplier between dates under this rate's day-count and compounding conventions."""
        ...

    def discount_factor(self, start: _DateLike, end: _DateLike) -> float:
        """Discount multiplier between dates, equal to the reciprocal accumulation factor."""
        ...

@final
class PayoffOps:
    """The native payoff expression evaluated on one index fixing.

Index and Const are leaves. Max, Min, Times, Plus, and Minus compose them.
Arithmetic operators also construct expressions; min() and max() apply bounds.
OptionEmbeddedCoupon pays notional * accrual * (payoff.eval(fixing) + spread)."""
    Index: ClassVar[PayoffOps]

    @staticmethod
    def Const(value: float) -> PayoffOps:
        """Builds a finite constant payoff independent of the index fixing."""
        ...

    @staticmethod
    def Max(left: PayoffOps | float, right: PayoffOps | float) -> PayoffOps:
        """Builds the pointwise maximum of two payoffs; numbers become constants."""
        ...

    @staticmethod
    def Min(left: PayoffOps | float, right: PayoffOps | float) -> PayoffOps:
        """Builds the pointwise minimum of two payoffs; numbers become constants."""
        ...

    @staticmethod
    def Times(left: PayoffOps | float, right: PayoffOps | float) -> PayoffOps:
        """Builds the product of two payoffs; numbers become constants."""
        ...

    @staticmethod
    def Plus(left: PayoffOps | float, right: PayoffOps | float) -> PayoffOps:
        """Builds the sum of two payoffs; numbers become constants."""
        ...

    @staticmethod
    def Minus(left: PayoffOps | float, right: PayoffOps | float) -> PayoffOps:
        """Builds left minus right as a payoff expression; numbers become constants."""
        ...

    def __add__(self, other: PayoffOps | float, /) -> PayoffOps:
        ...

    def __radd__(self, other: PayoffOps | float, /) -> PayoffOps:
        ...

    def __sub__(self, other: PayoffOps | float, /) -> PayoffOps:
        ...

    def __rsub__(self, other: PayoffOps | float, /) -> PayoffOps:
        ...

    def __mul__(self, other: PayoffOps | float, /) -> PayoffOps:
        ...

    def __rmul__(self, other: PayoffOps | float, /) -> PayoffOps:
        ...

    def min(self, other: PayoffOps | float) -> PayoffOps:
        """Returns a new expression taking the minimum of this payoff and other."""
        ...

    def max(self, other: PayoffOps | float) -> PayoffOps:
        """Returns a new expression taking the maximum of this payoff and other."""
        ...

    def eval(self, index_fixing: float) -> float:
        """Evaluates the native expression at a decimal index fixing."""
        ...

    def to_dict(self) -> str | dict[str, Any]:
        """Return the native JSON-compatible representation."""
        ...

    @staticmethod
    def from_dict(obj: str | Mapping[str, Any]) -> PayoffOps:
        """Reconstructs the native expression tree from its serialized representation."""
        ...

@final
class FixedRateCoupon:
    """Native fixed-rate coupon. Omit dates for a schedule-builder template.

Parameters
----------
rate : InterestRate
    Fixed annual rate and conventions.
notional : float, optional
    Coupon principal; defaults to a unit template. Builders supply the schedule balance.
accrual_start_date, accrual_end_date, payment_date : Date, optional
    Supply all three to create a dated coupon. Builders supply generated dates."""

    def __new__(cls, *, rate: InterestRate | float, notional: float=..., accrual_start_date: _DateLike | None=..., accrual_end_date: _DateLike | None=..., payment_date: _DateLike | None=...) -> Self:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> FixedRateCoupon:
        """Loads a tagged coupon specification returned by to_dict()."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return the native JSON-compatible representation."""
        ...

    def amount(self) -> float:
        """Undiscounted interest payment from the stored principal, rate, and accrual dates."""
        ...

    @property
    def notional(self) -> float:
        """Read-only principal used to calculate coupon interest."""
        ...

    @property
    def accrual_start_date(self) -> Date:
        """Read-only interest accrual start; undated templates use 1970-01-01."""
        ...

    @property
    def accrual_end_date(self) -> Date:
        """Read-only interest accrual end; undated templates use 1970-01-01."""
        ...

    @property
    def payment_date(self) -> Date:
        """Read-only interest payment date; undated templates use 1970-01-01."""
        ...

    @property
    def rate(self) -> InterestRate:
        """Read-only copy of the annual fixed rate and its accrual and compounding conventions."""
        ...

@final
class FloatingRateCoupon:
    """Native floating coupon paying the index fixing plus spread.

Parameters
----------
index : MarketIndex
    Index used by the pricer to resolve historical fixings and future forwards.
spread : float, optional
    Decimal annual spread added to the index; defaults to zero.
day_counter : DayCounter, optional
    Accrual basis; defaults to the index convention.
notional : float, optional
    Coupon principal; defaults to a unit schedule template.
accrual_start_date, accrual_end_date, payment_date : Date, optional
    Supply all three for a dated coupon. Builders supply generated dates.
fixing : float, optional
    Resolved observation for a standalone dated coupon."""

    def __new__(cls, *, index: _IndexLike, spread: float=..., day_counter: DayCounter | str | None=..., notional: float=..., accrual_start_date: _DateLike | None=..., accrual_end_date: _DateLike | None=..., payment_date: _DateLike | None=..., fixing: float | None=...) -> Self:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> FloatingRateCoupon:
        """Loads a tagged coupon specification returned by to_dict()."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return the native JSON-compatible representation."""
        ...

    def amount(self) -> float:
        """Undiscounted interest payment using the stored fixing plus spread and accrual year fraction."""
        ...

    @property
    def notional(self) -> float:
        """Read-only principal used to calculate coupon interest."""
        ...

    @property
    def accrual_start_date(self) -> Date:
        """Read-only interest accrual start; undated templates use 1970-01-01."""
        ...

    @property
    def accrual_end_date(self) -> Date:
        """Read-only interest accrual end; undated templates use 1970-01-01."""
        ...

    @property
    def payment_date(self) -> Date:
        """Read-only interest payment date; undated templates use 1970-01-01."""
        ...

    @property
    def index(self) -> MarketIndex:
        """Read-only index whose fixing or projected forward determines the floating rate."""
        ...

    @property
    def spread(self) -> float:
        """Read-only annual decimal margin added to the index fixing."""
        ...

    @property
    def fixing(self) -> float | None:
        """Read-only stored decimal index observation, or None when unresolved."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only convention used to calculate the coupon's accrual year fraction."""
        ...

@final
class OptionEmbeddedCoupon:
    """Native coupon with a custom payoff expression.

Parameters
----------
index : MarketIndex
    Observation supplied to the payoff expression.
payoff : PayoffOps
    Annual rate expression. Include a lending margin inside this expression
    when a cap or floor should apply to the all-in rate.
spread : float, optional
    Additional spread added after evaluating the expression; defaults to zero.
day_counter : DayCounter, optional
    Accrual basis; defaults to the index convention.
notional : float, optional
    Coupon principal; defaults to a unit schedule template.
accrual_start_date, accrual_end_date, payment_date : Date, optional
    Supply all three for a dated coupon. Builders supply generated dates.
fixing : float, optional
    Resolved observation for a standalone dated coupon."""

    def __new__(cls, *, index: _IndexLike, payoff: PayoffOps | str | Mapping[str, Any], spread: float=..., day_counter: DayCounter | str | None=..., notional: float=..., accrual_start_date: _DateLike | None=..., accrual_end_date: _DateLike | None=..., payment_date: _DateLike | None=..., fixing: float | None=...) -> Self:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> OptionEmbeddedCoupon:
        """Loads a tagged coupon specification returned by to_dict()."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return the native JSON-compatible representation."""
        ...

    def amount(self) -> float:
        """Undiscounted interest: notional * accrual * (payoff(fixing) + spread)."""
        ...

    @property
    def notional(self) -> float:
        """Read-only principal used to calculate coupon interest."""
        ...

    @property
    def accrual_start_date(self) -> Date:
        """Read-only interest accrual start; undated templates use 1970-01-01."""
        ...

    @property
    def accrual_end_date(self) -> Date:
        """Read-only interest accrual end; undated templates use 1970-01-01."""
        ...

    @property
    def payment_date(self) -> Date:
        """Read-only interest payment date; undated templates use 1970-01-01."""
        ...

    @property
    def index(self) -> MarketIndex:
        """Read-only index whose fixing supplies the payoff expression's input."""
        ...

    @property
    def spread(self) -> float:
        """Read-only annual decimal margin added after evaluating the payoff expression."""
        ...

    @property
    def fixing(self) -> float | None:
        """Read-only stored decimal payoff input, or None when the index fixing is unresolved."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only convention used to calculate the coupon's accrual year fraction."""
        ...

    @property
    def payoff(self) -> PayoffOps:
        """Read-only copy of the annual-rate expression applied before the additional spread."""
        ...

@final
class CallScheduleEntry:
    """One dated issuer call in a callable bond's schedule."""

    def __new__(cls, date: _DateLike | None=..., strike: float=..., *, start_date: _DateLike | None=..., end_date: _DateLike | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> CallScheduleEntry:
        """Builds a validated native call entry from a dict."""
        ...

    @staticmethod
    def window(start_date: _DateLike, end_date: _DateLike, strike: float | Mapping[str, Any] | str) -> Any:
        """Creates an inclusive issuer-call window."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns the native JSON-compatible schema."""
        ...

    def contains(self, date: _DateLike) -> Any:
        """Whether a date is inside the inclusive exercise window."""
        ...

    @property
    def start_date(self) -> Date:
        """Inclusive first exercise date."""
        ...

    @property
    def date(self) -> Date:
        """Issuer call date."""
        ...

    @property
    def strike(self) -> float:
        """Call strike per bond units, excluding same-day coupon and amortization."""
        ...

    @property
    def end_date(self) -> Date:
        """Inclusive final exercise date."""
        ...

@disjoint_base
class SteppedCouponBond(Instrument):
    """A callable bond with positional coupon and outstanding-notional schedules."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, currency: _CurrencyLike, coupons: Sequence[FixedRateCoupon | FloatingRateCoupon | OptionEmbeddedCoupon | Mapping[str, Any]], notional_percentages: Any, call_schedule: Any | None=..., units: float=..., payment_frequency: Any | None=..., coupon_day_counter: Any | None=..., calendar: Calendar | str | None=..., business_day_convention: BusinessDayConvention | str | None=..., date_generation_rule: DateGenerationRule | str | None=..., end_of_month: bool | None=..., first_coupon_date: _DateLike | None=..., next_to_last_coupon_date: _DateLike | None=..., termination_date_convention: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns constructor fields with native nested coupon and call schemas."""
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start_date. Collections are returned as copies."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only contractual maturity_date. Collections are returned as copies."""
        ...

    @property
    def notional(self) -> float:
        """Read-only contractual notional. Collections are returned as copies."""
        ...

    @property
    def units(self) -> float:
        """Read-only contractual units. Collections are returned as copies."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only contractual currency. Collections are returned as copies."""
        ...

    @property
    def payment_frequency(self) -> Frequency:
        """Read-only contractual payment_frequency. Collections are returned as copies."""
        ...

    @property
    def coupon_day_counter(self) -> DayCounter:
        """Read-only contractual coupon_day_counter. Collections are returned as copies."""
        ...

    @property
    def coupons(self) -> list[FixedRateCoupon | FloatingRateCoupon | OptionEmbeddedCoupon]:
        """Read-only contractual coupons. Collections are returned as copies."""
        ...

    @property
    def notional_percentages(self) -> list[float]:
        """Read-only contractual notional_percentages. Collections are returned as copies."""
        ...

    @property
    def call_schedule(self) -> list[CallScheduleEntry]:
        """Read-only contractual call_schedule. Collections are returned as copies."""
        ...

    @property
    def calendar(self) -> Calendar | None:
        """Read-only contractual calendar. Collections are returned as copies."""
        ...

    @property
    def business_day_convention(self) -> BusinessDayConvention | None:
        """Read-only contractual business_day_convention. Collections are returned as copies."""
        ...

    @property
    def date_generation_rule(self) -> DateGenerationRule | None:
        """Read-only contractual date_generation_rule. Collections are returned as copies."""
        ...

    @property
    def end_of_month(self) -> bool | None:
        """Read-only contractual end_of_month. Collections are returned as copies."""
        ...

    @property
    def first_coupon_date(self) -> Date | None:
        """Read-only contractual first_coupon_date. Collections are returned as copies."""
        ...

    @property
    def next_to_last_coupon_date(self) -> Date | None:
        """Read-only contractual next_to_last_coupon_date. Collections are returned as copies."""
        ...

    @property
    def termination_date_convention(self) -> BusinessDayConvention | None:
        """Read-only contractual termination_date_convention. Collections are returned as copies."""
        ...

@disjoint_base
class FloatingRateNote(Instrument):
    """A floating-rate note."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, spread: float, currency: _CurrencyLike, forward_index: _IndexLike, payment_frequency: Any | None=..., payment_structure: PaymentStructure | str | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start_date. Collections are returned as copies."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only contractual maturity_date. Collections are returned as copies."""
        ...

    @property
    def notional(self) -> float:
        """Read-only contractual notional. Collections are returned as copies."""
        ...

    @property
    def spread(self) -> float:
        """Read-only contractual spread. Collections are returned as copies."""
        ...

    @property
    def forward_index(self) -> MarketIndex:
        """Read-only contractual forward_index. Collections are returned as copies."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only contractual currency. Collections are returned as copies."""
        ...

    @property
    def payment_frequency(self) -> Frequency:
        """Read-only contractual payment_frequency. Collections are returned as copies."""
        ...

    @property
    def payment_structure(self) -> PaymentStructure:
        """Read-only contractual payment_structure. Collections are returned as copies."""
        ...

@disjoint_base
class FixedRateDeposit(Instrument):
    """A fixed-rate deposit."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, rate: float, currency: _CurrencyLike, discount_index: _IndexLike | None=..., day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., rate_frequency: Frequency | str | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start_date. Collections are returned as copies."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only contractual maturity_date. Collections are returned as copies."""
        ...

    @property
    def notional(self) -> float:
        """Read-only contractual notional. Collections are returned as copies."""
        ...

    @property
    def rate(self) -> float:
        """Read-only contractual rate. Collections are returned as copies."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only contractual currency. Collections are returned as copies."""
        ...

    @property
    def discount_index(self) -> MarketIndex | None:
        """Read-only contractual discount_index. Collections are returned as copies."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only contractual day_counter. Collections are returned as copies."""
        ...

    @property
    def compounding(self) -> Compounding:
        """Read-only contractual compounding. Collections are returned as copies."""
        ...

    @property
    def rate_frequency(self) -> Frequency:
        """Read-only contractual rate_frequency. Collections are returned as copies."""
        ...

@disjoint_base
class FxForward(Instrument):
    """A deliverable or non-deliverable FX forward."""

    def __new__(cls, identifier: str, delivery_date: _DateLike, base_currency: _CurrencyLike, quote_currency: _CurrencyLike, forward_price: Any | None=..., forward_points: Any | None=..., day_counter: DayCounter | str | None=..., fixing_date: _DateLike | None=..., settlement_currency: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def delivery_date(self) -> Date:
        """Read-only contractual delivery_date. Collections are returned as copies."""
        ...

    @property
    def base_currency(self) -> Currency:
        """Read-only contractual base_currency. Collections are returned as copies."""
        ...

    @property
    def quote_currency(self) -> Currency:
        """Read-only contractual quote_currency. Collections are returned as copies."""
        ...

    @property
    def forward_price(self) -> float | None:
        """Read-only contractual forward_price. Collections are returned as copies."""
        ...

    @property
    def forward_points(self) -> float | None:
        """Read-only contractual forward_points. Collections are returned as copies."""
        ...

    @property
    def day_counter(self) -> DayCounter | None:
        """Read-only contractual day_counter. Collections are returned as copies."""
        ...

    @property
    def fixing_date(self) -> Date | None:
        """Read-only contractual fixing_date. Collections are returned as copies."""
        ...

    @property
    def settlement_currency(self) -> Currency | None:
        """Read-only contractual settlement_currency. Collections are returned as copies."""
        ...

@disjoint_base
class FxOption(Instrument):
    """A European FX option."""

    def __new__(cls, identifier: str, expiry_date: _DateLike, strike: float | Mapping[str, Any] | str, option_type: OptionType | str, base_currency: _CurrencyLike, quote_currency: _CurrencyLike) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def expiry_date(self) -> Date:
        """Read-only contractual expiry_date. Collections are returned as copies."""
        ...

    @property
    def strike(self) -> float:
        """Read-only contractual strike. Collections are returned as copies."""
        ...

    @property
    def option_type(self) -> OptionType:
        """Read-only contractual option_type. Collections are returned as copies."""
        ...

    @property
    def base_currency(self) -> Currency:
        """Read-only contractual base_currency. Collections are returned as copies."""
        ...

    @property
    def quote_currency(self) -> Currency:
        """Read-only contractual quote_currency. Collections are returned as copies."""
        ...

@disjoint_base
class EquityOption(Instrument):
    """A European equity option."""

    def __new__(cls, identifier: str, market_index: _IndexLike, expiry_date: _DateLike, strike: float | Mapping[str, Any] | str, option_type: OptionType | str) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Read-only contractual market_index. Collections are returned as copies."""
        ...

    @property
    def expiry_date(self) -> Date:
        """Read-only contractual expiry_date. Collections are returned as copies."""
        ...

    @property
    def strike(self) -> str | dict[str, float]:
        """Read-only contractual strike. Collections are returned as copies."""
        ...

    @property
    def option_type(self) -> OptionType:
        """Read-only contractual option_type. Collections are returned as copies."""
        ...

@disjoint_base
class CreditDefaultSwap(Instrument):
    """A single-name credit default swap."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, spread: float, currency: _CurrencyLike, credit_index: _IndexLike, discount_index: _IndexLike, recovery: float=..., premium_frequency: Any | None=..., day_counter: DayCounter | str | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def credit_index(self) -> MarketIndex:
        """Read-only contractual credit_index. Collections are returned as copies."""
        ...

    @property
    def discount_index(self) -> MarketIndex:
        """Read-only contractual discount_index. Collections are returned as copies."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only contractual currency. Collections are returned as copies."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start_date. Collections are returned as copies."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only contractual maturity_date. Collections are returned as copies."""
        ...

    @property
    def spread(self) -> float:
        """Read-only contractual spread. Collections are returned as copies."""
        ...

    @property
    def recovery(self) -> float:
        """Read-only contractual recovery. Collections are returned as copies."""
        ...

    @property
    def premium_frequency(self) -> Frequency:
        """Read-only contractual premium_frequency. Collections are returned as copies."""
        ...

    @property
    def day_counter(self) -> DayCounter:
        """Read-only contractual day_counter. Collections are returned as copies."""
        ...

@disjoint_base
class CapFloor(Instrument):
    """An interest-rate cap or floor."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, strike: float | Mapping[str, Any] | str, notional: float, currency: _CurrencyLike, market_index: _IndexLike, cap_floor_type: CapFloorType | str, frequency: Frequency | str | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start_date. Collections are returned as copies."""
        ...

    @property
    def maturity_date(self) -> Date:
        """Read-only contractual maturity_date. Collections are returned as copies."""
        ...

    @property
    def strike(self) -> float:
        """Read-only contractual strike. Collections are returned as copies."""
        ...

    @property
    def notional(self) -> float:
        """Read-only contractual notional. Collections are returned as copies."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Read-only contractual market_index. Collections are returned as copies."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only contractual currency. Collections are returned as copies."""
        ...

    @property
    def cap_floor_type(self) -> CapFloorType:
        """Read-only contractual cap_floor_type. Collections are returned as copies."""
        ...

    @property
    def frequency(self) -> Frequency:
        """Read-only contractual frequency. Collections are returned as copies."""
        ...

@disjoint_base
class CapletFloorlet(Instrument):
    """A single caplet or floorlet."""

    def __new__(cls, identifier: str, fixing_date: _DateLike, start_accrual_date: _DateLike, end_accrual_date: _DateLike, payment_date: _DateLike, strike: float | Mapping[str, Any] | str, currency: _CurrencyLike, market_index: _IndexLike, payoff_type: CapletFloorletType | str) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Read-only contractual market_index. Collections are returned as copies."""
        ...

    @property
    def currency(self) -> Currency:
        """Read-only contractual currency. Collections are returned as copies."""
        ...

    @property
    def fixing_date(self) -> Date:
        """Read-only contractual fixing_date. Collections are returned as copies."""
        ...

    @property
    def start_accrual_date(self) -> Date:
        """Read-only contractual start_accrual_date. Collections are returned as copies."""
        ...

    @property
    def end_accrual_date(self) -> Date:
        """Read-only contractual end_accrual_date. Collections are returned as copies."""
        ...

    @property
    def payment_date(self) -> Date:
        """Read-only contractual payment_date. Collections are returned as copies."""
        ...

    @property
    def payoff_type(self) -> CapletFloorletType:
        """Read-only contractual payoff_type. Collections are returned as copies."""
        ...

    @property
    def strike(self) -> str | dict[str, float]:
        """Read-only contractual strike. Collections are returned as copies."""
        ...

@disjoint_base
class RateFutures(Instrument):
    """An exchange-traded interest-rate futures position."""

    def __new__(cls, identifier: str, start_date: _DateLike, end_date: _DateLike, futures_price: Any, market_index: _IndexLike, contract_size: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor."""
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file."""
        ...

    @property
    def identifier(self) -> str:
        """Read-only identifier assigned to this instrument."""
        ...

    @property
    def market_index(self) -> MarketIndex:
        """Read-only contractual market_index. Collections are returned as copies."""
        ...

    @property
    def start_date(self) -> Date:
        """Read-only contractual start_date. Collections are returned as copies."""
        ...

    @property
    def end_date(self) -> Date:
        """Read-only contractual end_date. Collections are returned as copies."""
        ...

    @property
    def futures_price(self) -> float:
        """Read-only contractual futures_price. Collections are returned as copies."""
        ...

    @property
    def contract_size(self) -> float | None:
        """Read-only contractual contract_size. Collections are returned as copies."""
        ...

@final
class ScriptedProduct:
    """Dated payoff code evaluated inside an initialized pricing context."""

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> ScriptedProduct:
        """Builds a script from events [{date, code}], currency, and optional model assignments."""
        ...

    @staticmethod
    def from_json(path: str) -> ScriptedProduct:
        """Loads the same script schema from a JSON file."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns the script specification, including its native model schemas."""
        ...

    @property
    def name(self) -> str:
        """Product name."""
        ...

    @property
    def identifier(self) -> str:
        """Product identifier (an alias for name)."""
        ...

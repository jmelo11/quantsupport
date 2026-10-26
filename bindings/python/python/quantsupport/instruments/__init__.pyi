"""Product specifications, coupons, loan terms, and payoff scripts."""
from datetime import date as PythonDate
from typing import Any, ClassVar, Mapping, Sequence, final, overload
from typing_extensions import Self, disjoint_base
from pandas import DataFrame
from ..time import BusinessDayConvention, Calendar, Compounding, Date, DateGenerationRule, DayCounter, Frequency, _DateLike, _PeriodLike
from ..market import Currency, MarketIndex, _CurrencyLike, _IndexLike
__all__ = ['Instrument', 'UnresolvedInstrument', 'Side', 'OptionType', 'SwaptionType', 'CapFloorType', 'CapletFloorletType', 'PaymentStructure', 'LoanRepaymentStructure', 'PrepaymentModelConfiguration', 'Loan', 'EquityForward', 'EuropeanSwaption', 'Swap', 'CrossCurrencySwap', 'BasisSwap', 'FixFloatCrossCurrencySwap', 'FixedRateBond', 'InterestRate', 'PayoffOps', 'FixedRateCoupon', 'FloatingRateCoupon', 'OptionEmbeddedCoupon', 'CallScheduleEntry', 'SteppedCouponBond', 'FloatingRateNote', 'FixedRateDeposit', 'FxForward', 'FxOption', 'EquityOption', 'CreditDefaultSwap', 'CapFloor', 'CapletFloorlet', 'RateFutures', 'ScriptedProduct']

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
        `"PayShort"`/`"Pay"`/`"Short"`.

        Parameters
        ----------
        s : str
            Case-insensitive side name or alias: LongReceive, Long, Receive,
            PayShort, Pay, or Short.

        Returns
        -------
        Side
            Receive/long or pay/short cashflow direction.
        """
        ...

    def sign(self) -> float:
        """Cashflow sign: +1 for `LongReceive`, -1 for `PayShort`.

        Returns
        -------
        float
            Dimensionless cashflow multiplier, +1.0 or -1.0.
        """
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
        """Parses an option type from its name.

        Parameters
        ----------
        s : str
            Case-insensitive Call or Put payoff direction.

        Returns
        -------
        OptionType
            Parsed call or put variant.
        """
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
        """Parses a cap/floor type from its name.

        Parameters
        ----------
        s : str
            Case-insensitive Cap or Floor payoff direction.

        Returns
        -------
        CapFloorType
            Parsed cap or floor variant.
        """
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
        """Parses a caplet/floorlet type from its name.

        Parameters
        ----------
        s : str
            Case-insensitive Caplet or Floorlet payoff direction.

        Returns
        -------
        CapletFloorletType
            Parsed single-period payoff variant.
        """
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
        """Parses a payment structure from its name.

        Parameters
        ----------
        s : str
            Case-insensitive Bullet, EqualPayments, EqualRedemptions, Zero,
            or Other amortization structure.

        Returns
        -------
        PaymentStructure
            Parsed leg amortization structure.
        """
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
        """Set the base product identifier.

        Parameters
        ----------
        identifier : str
            Stable product key used in evaluation results and diagnostics.
        *_args : Any
            Subclass positional construction arguments, ignored by the base.
        **_kwargs : Any
            Subclass keyword construction arguments, ignored by the base.
        """
        ...

    def __new__(cls, identifier: str, *_args: Any, **_kwargs: Any) -> Self:
        """Allocate a product with a stable identifier.

        Parameters
        ----------
        identifier : str
            Stable product key stored on the new instance.
        *_args : Any
            Extra subclass positional arguments, accepted but not stored.
        **_kwargs : Any
            Extra subclass keyword arguments, accepted but not stored.
        """
        ...

    def __repr__(self) -> str:
        ...

    @property
    def identifier(self) -> str:
        """Stable product identifier used in evaluation results and diagnostics."""
        ...

@final
class LoanRepaymentStructure:
    """Contractual principal amortization, fixed when the loan is resolved."""

    @staticmethod
    def french() -> LoanRepaymentStructure:
        """Use level initial instalments based on contractual accrual fractions.

        Returns
        -------
        LoanRepaymentStructure
            French principal schedule based on the initial amortization rate;
            subsequent floating-rate changes do not rebuild principal.
        """
        ...

    @staticmethod
    def bullet() -> LoanRepaymentStructure:
        """Pay interest during the term and repay all principal at maturity.

        Returns
        -------
        LoanRepaymentStructure
            Interest-only schedule with a final principal repayment.
        """
        ...

    @staticmethod
    def balloon(residual_fraction: float) -> LoanRepaymentStructure:
        """Reserve a fraction in [0, 1] of original principal for maturity.

        Parameters
        ----------
        residual_fraction : float
            Finite fraction of original principal reserved for maturity, between
            0 and 1 inclusive; 0.25 means 25%. Validated when building a Loan.

        Returns
        -------
        LoanRepaymentStructure
            French instalments with a residual balloon. Prepayment reduces the
            expected balloon proportionally, not the contractual schedule.
        """
        ...

    @staticmethod
    def from_dict(obj: str | Mapping[str, Any]) -> LoanRepaymentStructure:
        """Deserialize unit-variant strings or tagged repayment terms.

        Parameters
        ----------
        obj : str or Mapping[str, Any]
            Tagged dictionary returned by ``to_dict()``; legacy unit-variant
            strings are also accepted. Economic constraints are
            checked when constructing a Loan, not during deserialization.

        Returns
        -------
        LoanRepaymentStructure
            Deserialized contractual principal amortization terms.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return a tagged repayment dictionary; unit variants map to None.

        Returns
        -------
        dict[str, Any]
            Tagged representation accepted by ``from_dict()`` and Loan.
        """
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

    def __repr__(self) -> str: ...

@final
class PrepaymentModelConfiguration:
    """Annual decimal CPR parameters applied after scheduled principal."""

    @staticmethod
    def no_prepayment() -> PrepaymentModelConfiguration:
        """Disable unscheduled principal payments.

        Returns
        -------
        PrepaymentModelConfiguration
            Configuration with no conditional prepayment.
        """
        ...

    @staticmethod
    def constant_annual_cpr(cpr: float) -> PrepaymentModelConfiguration:
        """Use annual CPR in [0, 1]; convert to each interval's conditional probability.

        Parameters
        ----------
        cpr : float
            Annual conditional prepayment rate as a finite decimal fraction
            in [0, 1], validated by Loan construction. Interval prepayment
            probability is ``1 - (1 - cpr)**year_fraction``.

        Returns
        -------
        PrepaymentModelConfiguration
            Constant annual CPR applied after scheduled principal payments.
        """
        ...

    @staticmethod
    def dated_annual_cpr(points: Sequence[tuple[_DateLike, float]]) -> PrepaymentModelConfiguration:
        """Use strictly dated CPR points, zero before the first and flat after the last.

        Parameters
        ----------
        points : Sequence[tuple[Date or datetime.date or str, float]]
            Effective-date/annual-decimal-CPR pairs, with strictly increasing
            dates and finite CPRs in [0, 1], validated by Loan construction.
            Each interval uses the latest point at or before its start.

        Returns
        -------
        PrepaymentModelConfiguration
            Piecewise-constant CPR curve; zero before the first point and the
            final CPR thereafter.
        """
        ...

    @staticmethod
    def refinancing_incentive(base_cpr: float, incentive_multiplier: float, maximum_cpr: float) -> PrepaymentModelConfiguration:
        """Use capped CPR from positive coupon-minus-refinancing incentive; needs a pricer index.

        Parameters
        ----------
        base_cpr : float
            Finite annual decimal CPR without a positive refinancing incentive;
            must satisfy ``0 <= base_cpr <= maximum_cpr`` at Loan construction.
        incentive_multiplier : float
            Finite nonnegative multiplier of the positive difference between
            contractual coupon rate and refinancing benchmark rate, both decimals.
        maximum_cpr : float
            Finite cap on annual decimal CPR, no greater than 1. The pricer uses
            ``min(maximum_cpr, base_cpr + incentive_multiplier * incentive)``.

        Returns
        -------
        PrepaymentModelConfiguration
            Incentive-based terms requiring refinancing_benchmark_index in the
            context's Loan PricerConfiguration.
        """
        ...

    @staticmethod
    def from_dict(obj: str | Mapping[str, Any]) -> PrepaymentModelConfiguration:
        """Deserialize native prepayment terms; Loan construction validates constraints.

        Parameters
        ----------
        obj : str or Mapping[str, Any]
            Tagged dictionary returned by ``to_dict()``; legacy unit-variant
            strings are also accepted. Annual CPR values are decimals, not percentages.

        Returns
        -------
        PrepaymentModelConfiguration
            Deserialized conditional prepayment terms.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return a tagged prepayment dictionary; unit variants map to None.

        Returns
        -------
        dict[str, Any]
            Tagged representation accepted by ``from_dict()`` and Loan.
        """
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

    def __repr__(self) -> str: ...

@disjoint_base
class Loan(Instrument):
    """Contractual cashflows and principal; valuation applies market/default/prepayment effects."""

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, coupons: Sequence[FixedRateCoupon | FloatingRateCoupon | OptionEmbeddedCoupon | Mapping[str, Any]], currency: _CurrencyLike, repayment: LoanRepaymentStructure | str | Mapping[str, Any] | None=..., payment_frequency: Frequency | str | None=..., fixing_frequency: Frequency | str | None=..., prepayment: PrepaymentModelConfiguration | str | Mapping[str, Any] | None=..., credit_index: _IndexLike | None=..., recovery: float | None=..., fixing_lag: int | None=..., index_tenor: _PeriodLike | None=..., calendar: Calendar | str | None=..., business_day_convention: BusinessDayConvention | str | None=..., amortization_rate: float | None=...) -> Self:
        """Build loan terms; evaluation resolves a missing floating initial amortization_rate.

        Parameters
        ----------
        identifier : str
            Nonempty contract identifier used in pricing results and diagnostics.
        start_date : Date or datetime.date or str
            Origination and first accrual date, before maturity; strings are ISO dates.
        maturity_date : Date or datetime.date or str
            Final contractual accrual and principal repayment date.
        notional : float
            Finite, strictly positive original principal in the payment currency.
        coupons : Sequence[FixedRateCoupon or FloatingRateCoupon or OptionEmbeddedCoupon or Mapping[str, Any]]
            Nonempty positional coupon templates, or tagged coupon dictionaries
            returned by their ``to_dict()`` methods. The last entry repeats to
            maturity; builders supply balances and dates. Each coupon owns its
            accrual day counter; floating coupons default to the index convention.
            Amortization uses the same accrual conventions as coupon interest.
        currency : Currency or str
            Payment currency enum or alphabetic code; pricing policy selects
            the discount curve.
        repayment : LoanRepaymentStructure or str or Mapping[str, Any], optional
            Principal amortization terms or their native serialized form.
            Default None resolves to French amortization.
        payment_frequency : Frequency or str, optional
            Contractual payment interval. Default None resolves to Monthly.
        fixing_frequency : Frequency or str, optional
            Coupon reset interval, which must align with accrual dates. Default
            None uses index metadata, or payment frequency for daily indices.
        prepayment : PrepaymentModelConfiguration or str or Mapping[str, Any], optional
            Conditional prepayment terms applied during valuation after scheduled
            principal. Default None disables prepayment.
        credit_index : MarketIndex or str, optional
            Borrower survival-curve index, which must be a Credit index. Default
            None means no default model; serialized indices use native serde form.
        recovery : float, optional
            Recovery fraction in [0, 1]. Default None resolves to 0.4.
        fixing_lag : int, optional
            Nonnegative business days before coupon reset. Default None uses zero.
        index_tenor : Period or str, optional
            Positive observation tenor, independent of reset and payment frequency.
            Default None uses the index metadata frequency.
        calendar : Calendar or str, optional
            Payment/fixing calendar or serialized calendar name. Default None
            uses the null calendar.
        business_day_convention : BusinessDayConvention or str, optional
            Payment-date adjustment rule. Default None uses Unadjusted;
            contractual accrual dates are not adjusted.
        amortization_rate : float, optional
            Initial annual all-in decimal rate used to build principal. Default
            None uses the first fixed coupon, or resolves the initial floating
            observation when evaluated. Later coupons do not rebuild a resolved
            principal schedule.
        """
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Construct from keyword-compatible terms, preserving subclasses and constructor validation.

        Parameters
        ----------
        obj : Mapping[str, Any]
            Loan constructor keyword fields, including native nested coupon,
            repayment, and prepayment terms. Accepts ``to_dict()`` output;
            omitted options use the constructor's defaults.

        Returns
        -------
        Self
            Validated loan, preserving the class on which this method is called.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Construct from a JSON file using the from_dict schema, preserving subclasses.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of Loan constructor keywords,
            using the same nested representations as ``from_dict()``.

        Returns
        -------
        Self
            Validated loan loaded from the file, preserving Python subclasses.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return supplied construction terms, including omitted options as None, without market state.

        Returns
        -------
        dict[str, Any]
            JSON-compatible constructor inputs for ``from_dict()``. Contains no
            projected cashflows, resolved default schedule, or trade metadata.
        """
        ...

    @property
    def notional(self) -> float:
        """Original principal advanced under the contract."""
        ...

    @property
    def currency(self) -> Currency:
        """Native payment currency."""
        ...

    def coupon_fixing_requests(self, context: Any | None = ...) -> DataFrame:
        """Return unique index/fixing-date rows with ISO fixing/start/end dates in a DataFrame.

        Returns
        -------
        pandas.DataFrame
            Contractual observations in reset order, deduplicated by index and
            fixing date. Columns are market_index, fixing_date, start_date, and
            end_date; dates are ISO strings. Fixed-rate loans have no rows.
            The pricer decides which past fixings are needed at evaluation.
        Parameters
        ----------
        context : PricingContext or None, optional
            Required when the initial floating amortization rate is unresolved.
            None uses an already concrete loan schedule.
        """
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

    def __repr__(self) -> str: ...

@final
class SwaptionType:
    """Payer or receiver European swaption."""

    def __repr__(self) -> str:
        ...

    def __str__(self) -> str:
        ...

    @staticmethod
    def parse(s: str) -> SwaptionType:
        """Parses the payer or receiver exercise direction from its name.

        Parameters
        ----------
        s : str
            Case-insensitive Payer (pay fixed) or Receiver (receive fixed).

        Returns
        -------
        SwaptionType
            Parsed underlying-swap exercise direction.
        """
        ...
    Payer: ClassVar[SwaptionType]
    Receiver: ClassVar[SwaptionType]

@disjoint_base
class EquityForward(Instrument):
    """Equity forward contract; its position size belongs to `Trade`.

    Parameters
    ----------
    identifier : str
        Product key used to identify the forward in pricing results.
    market_index : MarketIndex or str
        Underlying equity index or its native serialized index representation.
    delivery_date : Date or datetime.date or str
        Contract settlement date; strings use ISO YYYY-MM-DD format.
    strike : float
        Finite contractual delivery price per equity unit in currency.
    currency : Currency or str
        Payment currency enum or case-insensitive alphabetic currency code.
    day_counter : DayCounter or str, optional
        Time basis for the forward; default None resolves to Actual360.
    """

    def __new__(cls, identifier: str, market_index: _IndexLike, delivery_date: _DateLike, strike: float, currency: _CurrencyLike, day_counter: DayCounter | str | None=...) -> Self:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            Constructor keyword fields: identifier, market_index, delivery_date,
            strike, currency, and optional day_counter. Strike is a price per
            equity unit; values use the constructor's date/index conventions.

        Returns
        -------
        Self
            Equity forward constructed through cls, preserving Python subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of EquityForward constructor fields,
            as described by ``from_dict()``; dates use ISO strings and indices
            use native serde representations.

        Returns
        -------
        Self
            Equity forward loaded through cls with normal constructor defaults.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

    def __repr__(self) -> str: ...

@disjoint_base
class EuropeanSwaption(Instrument):
    """European option to enter a payer or receiver fixed/floating swap at expiry.

    Parameters
    ----------
    identifier : str
        Product identifier used in valuation results.
    expiry_date : Date or datetime.date or str
        European exercise date and start of the underlying swap; ISO strings accepted.
    maturity_date : Date or datetime.date or str
        Final maturity of the underlying fixed/floating swap, strictly after expiry.
    strike : float
        Finite underlying fixed annual decimal rate, for example 0.04 for 4%.
    notional : float
        Finite nonnegative underlying swap principal in currency; Trade may
        override the amount.
    currency : Currency or str
        Underlying payment currency enum or alphabetic code.
    market_index : MarketIndex or str
        Underlying floating-rate index or native serialized index representation.
    swaption_type : SwaptionType or str, optional
        Payer pays fixed on exercise; Receiver receives fixed. Names are
        case-insensitive. Default None resolves to Payer.
    day_counter : DayCounter or str, optional
        Underlying rate accrual basis; default None resolves to Actual360.
    compounding : Compounding or str, optional
        Underlying fixed-rate compounding rule; default None uses Simple.
    rate_frequency : Frequency or str, optional
        Fixed-rate compounding frequency; default None uses Annual.
    fixed_leg_frequency : Frequency or str, optional
        Underlying fixed-leg payment interval; default None uses Annual.
    floating_leg_frequency : Frequency or str, optional
        Underlying floating-leg payment interval; default None uses Quarterly.
    calendar : Calendar or str, optional
        Underlying schedule calendar or serialized calendar name. Default None
        leaves calendar selection to the native swaption implementation.
    """

    def __new__(cls, identifier: str, expiry_date: _DateLike, maturity_date: _DateLike, strike: float, notional: float, currency: _CurrencyLike, market_index: _IndexLike, swaption_type: SwaptionType | str | None=..., day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., rate_frequency: Frequency | str | None=..., fixed_leg_frequency: Frequency | str | None=..., floating_leg_frequency: Frequency | str | None=..., calendar: Calendar | str | None=...) -> Self:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            Swaption constructor keywords: identifier, expiry_date, maturity_date,
            strike, notional, currency, market_index, and optional direction,
            rate, frequency, and calendar settings. Strike is an annual decimal
            rate; omitted fields follow the constructor defaults.

        Returns
        -------
        Self
            European swaption constructed through cls, preserving subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of EuropeanSwaption constructor
            keywords, using ``from_dict()`` fields, ISO dates, and native
            serialized index/calendar representations.

        Returns
        -------
        Self
            European swaption loaded through cls with constructor validation.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

    def __repr__(self) -> str: ...

@disjoint_base
class Swap(Instrument):
    """A vanilla fixed-vs-floating interest rate swap.

    Parameters
    ----------
    identifier : str
        Product identifier for the fixed/floating contract.
    start_date : Date or datetime.date or str
        First accrual date of both legs; ISO YYYY-MM-DD strings are accepted.
    maturity_date : Date or datetime.date or str
        Contractual end date of the swap schedule.
    notional : float
        Principal of both legs in currency; Trade may override this amount.
    fixed_rate : float
        Annual decimal fixed coupon rate; 0.05 means 5%.
    currency : Currency or str
        Payment currency enum or alphabetic currency code.
    market_index : MarketIndex or str
        Floating-leg reference index or native serialized index representation.
    spread : float, optional
        Annual decimal margin added to the floating rate. Default None leaves
        the spread unset in the native builder.
    day_counter : DayCounter or str, optional
        Fixed-rate day-count convention; default None uses Actual360.
    compounding : Compounding or str, optional
        Fixed-rate accumulation convention; default None uses Simple.
    rate_frequency : Frequency or str, optional
        Fixed-rate compounding frequency; default None uses Semiannual.
    fixed_leg_frequency : Frequency or str, optional
        Fixed-leg payment interval; default None uses Semiannual.
    floating_leg_frequency : Frequency or str, optional
        Floating-leg payment interval; default None uses Semiannual.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, fixed_rate: float, currency: _CurrencyLike, market_index: _IndexLike, spread: float | None=..., day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., rate_frequency: Frequency | str | None=..., fixed_leg_frequency: Frequency | str | None=..., floating_leg_frequency: Frequency | str | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            Swap constructor keywords, including identifier, start_date,
            maturity_date, notional, fixed_rate, currency, and market_index.
            Optional spread/rate/frequency settings retain constructor defaults;
            fixed_rate and spread are annual decimals.

        Returns
        -------
        Self
            Fixed/floating swap constructed through cls, preserving subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of Swap constructor keywords,
            with ISO start/maturity dates and native index representation;
            the schema and defaults are those of ``from_dict()``.

        Returns
        -------
        Self
            Fixed/floating swap loaded through cls, preserving subclasses.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class CrossCurrencySwap(Instrument):
    """A float-vs-float cross-currency swap.

    Parameters
    ----------
    identifier : str
        Product identifier for the two-currency floating-rate contract.
    start_date : Date or datetime.date or str
        Initial exchange and accrual date; ISO date strings are accepted.
    maturity_date : Date or datetime.date or str
        Final exchange and accrual date of both legs.
    domestic_notional : float
        Domestic-leg principal in domestic_currency; Trade can override it.
    foreign_notional : float
        Foreign-leg principal in foreign_currency; Trade can override it.
    domestic_currency : Currency or str
        Currency enum or alphabetic code for the domestic floating leg.
    foreign_currency : Currency or str
        Currency enum or alphabetic code for the foreign floating leg.
    domestic_market_index : MarketIndex or str
        Domestic floating-rate reference index or native index representation.
    foreign_market_index : MarketIndex or str
        Foreign floating-rate reference index or native index representation.
    domestic_spread : float, optional
        Annual decimal margin on the domestic index; default 0.0.
    foreign_spread : float, optional
        Annual decimal margin on the foreign index; default 0.0.
    domestic_leg_frequency : Frequency or str, optional
        Domestic coupon payment interval; default None uses Semiannual.
    foreign_leg_frequency : Frequency or str, optional
        Foreign coupon payment interval; default None uses Semiannual.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, domestic_notional: float, foreign_notional: float, domestic_currency: _CurrencyLike, foreign_currency: _CurrencyLike, domestic_market_index: _IndexLike, foreign_market_index: _IndexLike, domestic_spread: float=..., foreign_spread: float=..., domestic_leg_frequency: Any | None=..., foreign_leg_frequency: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            CrossCurrencySwap constructor keywords: identifier, start_date,
            maturity_date, domestic/foreign notional, currency, and market_index
            fields, plus optional spreads and leg frequencies. Each notional
            is denominated in its own leg currency; spreads are annual decimals.

        Returns
        -------
        Self
            Two-currency floating swap constructed through cls.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of CrossCurrencySwap constructor
            fields, as in ``from_dict()``. Use ISO dates, currency codes, and
            native serialized indices for both legs.

        Returns
        -------
        Self
            Cross-currency swap loaded through cls with constructor defaults.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class BasisSwap(Instrument):
    """A float-vs-float single-currency basis swap.

    Parameters
    ----------
    identifier : str
        Product identifier for the two-index basis contract.
    start_date : Date or datetime.date or str
        First accrual date of both legs; ISO date strings are accepted.
    maturity_date : Date or datetime.date or str
        Final accrual date of the basis swap.
    notional : float
        Principal shared by both legs, in currency; Trade may override it.
    currency : Currency or str
        Single payment currency enum or alphabetic code.
    pay_market_index : MarketIndex or str
        Reference index for the named pay leg; Trade side sets position direction.
    receive_market_index : MarketIndex or str
        Reference index for the named receive leg; native index strings accepted.
    pay_spread : float, optional
        Annual decimal margin on the pay index. Default None leaves this
        spread unset in the native builder.
    receive_spread : float, optional
        Annual decimal margin on the receive index. Default None leaves this
        spread unset in the native builder.
    pay_leg_frequency : Frequency or str, optional
        Pay-leg coupon payment interval; default None uses Quarterly.
    receive_leg_frequency : Frequency or str, optional
        Receive-leg coupon payment interval; default None uses Quarterly.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, currency: _CurrencyLike, pay_market_index: _IndexLike, receive_market_index: _IndexLike, pay_spread: Any | None=..., receive_spread: Any | None=..., pay_leg_frequency: Any | None=..., receive_leg_frequency: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            BasisSwap constructor keywords: identifier, start_date, maturity_date,
            notional, currency, pay_market_index, receive_market_index, and
            optional pay/receive spreads and leg frequencies. Spreads are
            annual decimals; both legs share the payment currency.

        Returns
        -------
        Self
            Single-currency basis swap constructed through cls.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of BasisSwap constructor keywords,
            following ``from_dict()`` with ISO dates and native pay/receive
            index representations.

        Returns
        -------
        Self
            Basis swap loaded through cls, preserving subclasses and defaults.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class FixFloatCrossCurrencySwap(Instrument):
    """A fixed-vs-float cross-currency swap.

    Parameters
    ----------
    identifier : str
        Product identifier for the fixed/floating cross-currency contract.
    start_date : Date or datetime.date or str
        Initial exchange and first accrual date; ISO strings are accepted.
    maturity_date : Date or datetime.date or str
        Final exchange and last accrual date.
    domestic_notional : float
        Principal of the domestic fixed leg in domestic_currency.
    foreign_notional : float
        Principal of the foreign floating leg in foreign_currency.
    fixed_rate : float
        Annual decimal coupon rate on the domestic fixed leg.
    domestic_currency : Currency or str
        Domestic fixed-leg currency enum or alphabetic code.
    foreign_currency : Currency or str
        Foreign floating-leg currency enum or alphabetic code.
    floating_index : MarketIndex or str
        Foreign-leg floating reference index or native index representation.
    spread : float, optional
        Annual decimal margin on the floating index. Default None leaves
        the spread unset in the native builder.
    day_counter : DayCounter or str, optional
        Fixed-rate accrual basis; default None uses Actual360.
    compounding : Compounding or str, optional
        Fixed-rate compounding convention; default None uses Simple.
    rate_frequency : Frequency or str, optional
        Fixed-rate compounding frequency; default None uses Semiannual.
    domestic_leg_frequency : Frequency or str, optional
        Domestic fixed coupon payment interval; default None uses Semiannual.
    foreign_leg_frequency : Frequency or str, optional
        Foreign floating coupon payment interval; default None uses Semiannual.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, domestic_notional: float, foreign_notional: float, fixed_rate: float, domestic_currency: _CurrencyLike, foreign_currency: _CurrencyLike, floating_index: _IndexLike, spread: float | None=..., day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., rate_frequency: Frequency | str | None=..., domestic_leg_frequency: Any | None=..., foreign_leg_frequency: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            FixFloatCrossCurrencySwap constructor keywords, including identifier,
            start_date, maturity_date, both currency/notional pairs, fixed_rate,
            and floating_index. Optional spread and rate/frequency settings
            use constructor defaults; coupon rates and spreads are decimals.

        Returns
        -------
        Self
            Domestic-fixed/foreign-floating swap constructed through cls.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of FixFloatCrossCurrencySwap
            constructor keywords, following ``from_dict()`` with ISO dates,
            currency codes, and native floating-index serialization.

        Returns
        -------
        Self
            Fixed/floating cross-currency swap loaded through cls.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class FixedRateBond(Instrument):
    """A fixed-rate bullet (or amortizing) bond.

    Parameters
    ----------
    identifier : str
        Product identifier used in bond pricing results.
    start_date : Date or datetime.date or str
        Issue/effective date and first accrual date; ISO strings are accepted.
    maturity_date : Date or datetime.date or str
        Final contractual coupon and principal repayment date.
    notional : float
        Original principal in currency; Trade may override the position amount.
    rate : float
        Annual decimal fixed coupon rate; 0.04 denotes 4%.
    currency : Currency or str
        Payment currency enum or alphabetic currency code.
    discount_index : MarketIndex or str, optional
        Explicit discount-curve index. Default None leaves it unset for native
        builder/pricing-policy resolution.
    day_counter : DayCounter or str, optional
        Coupon accrual day-count basis; default None resolves to Actual360.
    compounding : Compounding or str, optional
        Fixed-rate accumulation rule; default None resolves to Simple.
    rate_frequency : Frequency or str, optional
        Fixed-rate compounding frequency; default None uses Semiannual.
    payment_frequency : Frequency or str, optional
        Coupon payment interval; default None uses Semiannual.
    payment_structure : PaymentStructure or str, optional
        Principal schedule: Bullet, EqualPayments, EqualRedemptions, Zero, or
        Other enum/name. Default None uses Bullet; native construction checks
        whether the requested schedule is supported.
    calendar : Calendar or str, optional
        Schedule holiday calendar or serialized name. Default None leaves the
        native schedule builder's calendar unchanged.
    business_day_convention : BusinessDayConvention or str, optional
        Schedule date-adjustment rule; default None defers to the native builder.
    date_generation_rule : DateGenerationRule or str, optional
        Direction/rule for regular schedule generation, such as Backward or
        Forward. Default None leaves the native builder's rule unchanged.
    end_of_month : bool, optional
        Whether regular dates retain month-end alignment. Default None leaves
        the native builder's month-end setting unchanged.
    first_coupon_date : Date or datetime.date or str, optional
        Explicit first coupon date for an initial stub. Default None generates
        the first coupon from the regular schedule.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, rate: float, currency: _CurrencyLike, discount_index: _IndexLike | None=..., day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., rate_frequency: Frequency | str | None=..., payment_frequency: Any | None=..., payment_structure: PaymentStructure | str | None=..., calendar: Calendar | str | None=..., business_day_convention: BusinessDayConvention | str | None=..., date_generation_rule: DateGenerationRule | str | None=..., end_of_month: bool | None=..., first_coupon_date: _DateLike | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serializes bond contract terms for constructor round trips.

        Returns
        -------
        dict[str, Any]
            JSON-compatible constructor fields with resolved rate/payment defaults
            and native optional schedule/index representations; excludes trade state.
        """
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            FixedRateBond constructor keyword fields, including identifier,
            start_date, maturity_date, notional, annual decimal rate, currency,
            and optional rate/payment/schedule settings. Accepts ``to_dict()``
            output with native date, calendar, and index serialization.

        Returns
        -------
        Self
            Bond constructed through cls, preserving Python subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of FixedRateBond constructor fields,
            following ``from_dict()`` or serialized ``to_dict()`` output. Dates
            are ISO strings and nested values retain their native schemas.

        Returns
        -------
        Self
            Fixed-rate bond loaded through cls with constructor defaults.
        """
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

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@final
class InterestRate:
    """An annual interest rate with the native day-count and compounding conventions.

Parameters
----------
rate : float
    Finite annual decimal rate, for example 0.06 for 6%.
day_counter : DayCounter or str, optional
    Day-count convention for the accrual year fraction. Default None uses Actual360.
compounding : Compounding or str, optional
    Rule converting the rate and accrual fraction into accumulation.
    Default None uses Simple.
frequency : Frequency or str, optional
    Compounding frequency used by the rate convention. Default None uses Annual."""

    def __new__(cls, rate: float, day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., frequency: Frequency | str | None=...) -> Self:
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
        """Accumulation multiplier between dates under this rate's day-count and compounding conventions.

        Parameters
        ----------
        start : Date or datetime.date or str
            Initial accumulation date; strings use ISO YYYY-MM-DD format.
        end : Date or datetime.date or str
            Final accumulation date. The stored day counter measures the
            year fraction from start to end.

        Returns
        -------
        float
            Dimensionless accumulated value of one initial currency unit.
        """
        ...

    def discount_factor(self, start: _DateLike, end: _DateLike) -> float:
        """Discount multiplier between dates, equal to the reciprocal accumulation factor.

        Parameters
        ----------
        start : Date or datetime.date or str
            Date to which a unit payment is discounted; ISO strings are accepted.
        end : Date or datetime.date or str
            Payment date used with start to compute the accrual year fraction.

        Returns
        -------
        float
            Dimensionless reciprocal of ``compound_factor(start, end)``.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> InterestRate:
        """Restore a native interest-rate dictionary.

        Parameters
        ----------
        obj : mapping
            Rate and nested rate_definition returned by to_dict().
        """
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

    def __repr__(self) -> str: ...


@final
class PayoffOps:
    """The native payoff expression evaluated on one index fixing.

Index and Const are leaves. Max, Min, Times, Plus, and Minus compose them.
Arithmetic operators also construct expressions; min() and max() apply bounds.
OptionEmbeddedCoupon pays notional * accrual * (payoff.eval(fixing) + spread)."""
    Index: ClassVar[PayoffOps]

    @staticmethod
    def Const(value: float) -> PayoffOps:
        """Builds a finite constant payoff independent of the index fixing.

        Parameters
        ----------
        value : float
            Finite constant expression value. Use decimal annual-rate units
            when this is a coupon rate or strike, for example 0.03 for 3%.

        Returns
        -------
        PayoffOps
            Constant leaf whose evaluation always returns value.
        """
        ...

    @staticmethod
    def Max(left: PayoffOps | float, right: PayoffOps | float) -> PayoffOps:
        """Builds the pointwise maximum of two payoffs; numbers become constants.

        Parameters
        ----------
        left : PayoffOps or float
            First rate expression or finite constant, evaluated at the same fixing.
        right : PayoffOps or float
            Second expression or finite constant in matching units; a constant
            acts as a floor on the first expression.

        Returns
        -------
        PayoffOps
            New expression evaluating to the larger operand.
        """
        ...

    @staticmethod
    def Min(left: PayoffOps | float, right: PayoffOps | float) -> PayoffOps:
        """Builds the pointwise minimum of two payoffs; numbers become constants.

        Parameters
        ----------
        left : PayoffOps or float
            First rate expression or finite constant, evaluated at the same fixing.
        right : PayoffOps or float
            Second expression or finite constant in matching units; a constant
            acts as a cap on the first expression.

        Returns
        -------
        PayoffOps
            New expression evaluating to the smaller operand.
        """
        ...

    @staticmethod
    def Times(left: PayoffOps | float, right: PayoffOps | float) -> PayoffOps:
        """Builds the product of two payoffs; numbers become constants.

        Parameters
        ----------
        left : PayoffOps or float
            Multiplicand expression or finite constant, such as a decimal rate.
        right : PayoffOps or float
            Multiplier expression or finite constant; use a dimensionless
            constant to gear an annual-rate expression.

        Returns
        -------
        PayoffOps
            New expression evaluating to the product of both operands.
        """
        ...

    @staticmethod
    def Plus(left: PayoffOps | float, right: PayoffOps | float) -> PayoffOps:
        """Builds the sum of two payoffs; numbers become constants.

        Parameters
        ----------
        left : PayoffOps or float
            First expression or finite constant, in annual decimal-rate units
            when used for a coupon.
        right : PayoffOps or float
            Addend expression or finite constant in matching units; include
            margin here when subsequent caps/floors should apply to it.

        Returns
        -------
        PayoffOps
            New expression evaluating to the sum at one index fixing.
        """
        ...

    @staticmethod
    def Minus(left: PayoffOps | float, right: PayoffOps | float) -> PayoffOps:
        """Builds left minus right as a payoff expression; numbers become constants.

        Parameters
        ----------
        left : PayoffOps or float
            Minuend expression or finite constant, evaluated at the index fixing.
        right : PayoffOps or float
            Subtrahend expression or finite constant in matching units, such as
            an annual decimal strike subtracted from an index rate.

        Returns
        -------
        PayoffOps
            New expression evaluating to left minus right, preserving operand order.
        """
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
        """Returns a new expression taking the minimum of this payoff and other.

        Parameters
        ----------
        other : PayoffOps or float
            Comparison expression or finite constant in this payoff's units;
            a constant caps the expression.

        Returns
        -------
        PayoffOps
            New pointwise minimum; the original expression is unchanged.
        """
        ...

    def max(self, other: PayoffOps | float) -> PayoffOps:
        """Returns a new expression taking the maximum of this payoff and other.

        Parameters
        ----------
        other : PayoffOps or float
            Comparison expression or finite constant in this payoff's units;
            a constant floors the expression.

        Returns
        -------
        PayoffOps
            New pointwise maximum; the original expression is unchanged.
        """
        ...

    def eval(self, index_fixing: float) -> float:
        """Evaluates the native expression at a decimal index fixing.

        Parameters
        ----------
        index_fixing : float
            Finite observation substituted for every Index leaf. Coupon index
            rates are annual decimal fractions, for example 0.05 for 5%.

        Returns
        -------
        float
            Evaluated expression, before the coupon's separate spread, notional,
            and accrual fraction are applied.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return the native JSON-compatible representation.

        Returns
        -------
        dict[str, Any]
            Tagged expression tree accepted by ``from_dict()``. The Index
            leaf is represented by {"Index": None}.
        """
        ...

    @staticmethod
    def from_dict(obj: str | Mapping[str, Any]) -> PayoffOps:
        """Reconstructs the native expression tree from its serialized representation.

        Parameters
        ----------
        obj : str or Mapping[str, Any]
            Native expression representation from ``to_dict()``, retaining
            operand order and nested nodes rather than a Python expression string.

        Returns
        -------
        PayoffOps
            Reconstructed expression for evaluation at a single index fixing.
        """
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

    def __repr__(self) -> str: ...

@final
class FixedRateCoupon:
    """Native fixed-rate coupon. Omit dates for a schedule-builder template.

Parameters
----------
rate : InterestRate or float
    Fixed annual rate and conventions. A finite decimal float uses Actual360,
    Simple compounding, and Annual frequency; 0.06 means 6%.
notional : float, optional
    Finite positive coupon principal; default 1.0 is a unit template.
    Schedule builders supply the contractual balance.
accrual_start_date : Date or datetime.date or str, optional
    First interest accrual date. Default None creates an undated template only
    when all three dates are omitted; ISO date strings are accepted.
accrual_end_date : Date or datetime.date or str, optional
    Final accrual date, strictly after accrual_start_date. Default None;
    supply all three dates together for a standalone coupon.
payment_date : Date or datetime.date or str, optional
    Interest payment date, on or after accrual_end_date. Default None;
    builders replace the template's 1970-01-01 placeholder dates."""

    def __new__(cls, rate: InterestRate | float, notional: float=..., accrual_start_date: _DateLike | None=..., accrual_end_date: _DateLike | None=..., payment_date: _DateLike | None=...) -> Self:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> FixedRateCoupon:
        """Loads a tagged coupon specification returned by to_dict().

        Parameters
        ----------
        obj : Mapping[str, Any]
            Single-key tagged specification with FixedRateCoupon (or legacy
            Fixed) as key and constructor terms as value. Accepts ``to_dict()``
            output, including the native serialized InterestRate.

        Returns
        -------
        FixedRateCoupon
            Fixed coupon or undated template; other coupon kinds are rejected.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return the native JSON-compatible representation.

        Returns
        -------
        dict[str, Any]
            FixedRateCoupon-tagged constructor terms, including rate conventions;
            placeholder dates are omitted for an undated template.
        """
        ...

    def amount(self) -> float:
        """Undiscounted interest payment from the stored principal, rate, and accrual dates.

        Returns
        -------
        float
            Interest in the same monetary units as notional, using the rate's
            compounding conventions; excludes principal repayment.
        """
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

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

    def __repr__(self) -> str: ...

@final
class FloatingRateCoupon:
    """Native floating coupon paying the index fixing plus spread.

Parameters
----------
index : MarketIndex or str
    Index used by the pricer to resolve historical fixings and future forwards;
    serialized strings use the native index representation.
spread : float, optional
    Finite annual decimal spread added to the index; default 0.0.
day_counter : DayCounter or str, optional
    Accrual year-fraction convention; default None uses the index convention.
notional : float, optional
    Finite positive coupon principal; default 1.0 is a unit schedule template.
accrual_start_date : Date or datetime.date or str, optional
    First accrual date; default None. Omit all three dates for a schedule
    template or supply all three together, using date objects or ISO strings.
accrual_end_date : Date or datetime.date or str, optional
    Final accrual date, strictly after the start; default None. Builders supply
    generated dates in place of the template's 1970-01-01 placeholders.
payment_date : Date or datetime.date or str, optional
    Interest payment date, on or after accrual_end_date; default None.
fixing : float, optional
    Finite annual decimal index observation for a standalone coupon, before
    spread. Default None leaves it unresolved; ``amount()`` requires a fixing."""

    def __new__(cls, index: _IndexLike, spread: float=..., day_counter: DayCounter | str | None=..., notional: float=..., accrual_start_date: _DateLike | None=..., accrual_end_date: _DateLike | None=..., payment_date: _DateLike | None=..., fixing: float | None=...) -> Self:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> FloatingRateCoupon:
        """Loads a tagged coupon specification returned by to_dict().

        Parameters
        ----------
        obj : Mapping[str, Any]
            Single-key FloatingRateCoupon (or legacy Floating) tagged dictionary
            of constructor terms, as returned by ``to_dict()``. The decoded kind
            must remain floating; capped/floored legacy terms may decode to an
            option-embedded coupon and are not accepted by this method.

        Returns
        -------
        FloatingRateCoupon
            Floating coupon or undated template with its stored optional fixing.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return the native JSON-compatible representation.

        Returns
        -------
        dict[str, Any]
            FloatingRateCoupon-tagged constructor terms, including index,
            day count, spread, and fixing; template dates are omitted.
        """
        ...

    def amount(self) -> float:
        """Undiscounted interest payment using the stored fixing plus spread and accrual year fraction.

        Returns
        -------
        float
            Interest in notional units: principal times accrual fraction times
            (fixing + spread). Raises an error if no fixing has been resolved.
        """
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

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

    def __repr__(self) -> str: ...

@final
class OptionEmbeddedCoupon:
    """Native coupon with a custom payoff expression.

Parameters
----------
index : MarketIndex or str
    Index supplying the payoff observation; strings use native index serialization.
payoff : PayoffOps or str or Mapping[str, Any]
    Annual decimal-rate expression or native representation from PayoffOps.to_dict().
    Include a lending margin inside this expression
    when a cap or floor should apply to the all-in rate.
spread : float, optional
    Finite annual decimal spread added after evaluating the expression;
    default 0.0. This margin lies outside any cap or floor in payoff.
day_counter : DayCounter or str, optional
    Accrual year-fraction convention; default None uses the index convention.
notional : float, optional
    Finite positive coupon principal; default 1.0 is a unit schedule template.
accrual_start_date : Date or datetime.date or str, optional
    First accrual date; default None. Supply all three dates together, or omit
    them all for a schedule-builder template; ISO date strings are accepted.
accrual_end_date : Date or datetime.date or str, optional
    Final accrual date, strictly after the start; default None. Builders replace
    the template's 1970-01-01 dates with contractual dates.
payment_date : Date or datetime.date or str, optional
    Interest payment date, on or after accrual_end_date; default None.
fixing : float, optional
    Finite annual decimal index observation passed into payoff before spread.
    Default None leaves the observation unresolved; ``amount()`` requires it."""

    def __new__(cls, index: _IndexLike, payoff: PayoffOps | str | Mapping[str, Any], spread: float=..., day_counter: DayCounter | str | None=..., notional: float=..., accrual_start_date: _DateLike | None=..., accrual_end_date: _DateLike | None=..., payment_date: _DateLike | None=..., fixing: float | None=...) -> Self:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> OptionEmbeddedCoupon:
        """Loads a tagged coupon specification returned by to_dict().

        Parameters
        ----------
        obj : Mapping[str, Any]
            Single-key OptionEmbeddedCoupon-tagged constructor terms, including
            the native payoff tree, as returned by ``to_dict()``. Legacy tagged
            floating terms with cap/floor bounds are accepted when they decode
            to an option-embedded coupon.

        Returns
        -------
        OptionEmbeddedCoupon
            Coupon or undated template with the reconstructed payoff expression.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return the native JSON-compatible representation.

        Returns
        -------
        dict[str, Any]
            OptionEmbeddedCoupon-tagged constructor terms preserving the native
            payoff tree, index, spread, and fixing; template dates are omitted.
        """
        ...

    def amount(self) -> float:
        """Undiscounted interest: notional * accrual * (payoff(fixing) + spread).

        Returns
        -------
        float
            Interest in notional units after evaluating the payoff and adding
            spread. Raises an error when the required fixing is unresolved.
        """
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

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

    def __repr__(self) -> str: ...

@final
class CallScheduleEntry:
    """One dated issuer call in a callable bond's schedule.

    Parameters
    ----------
    date : Date or datetime.date or str, optional
        Single call date or first date of a call window, as a date or ISO string.
        Default None; exactly one of date and start_date must be supplied.
    strike : float, optional
        Finite positive call price per bond units, excluding same-day coupon and
        amortization. Default 100.0; this is not a decimal interest rate.
    start_date : Date or datetime.date or str, optional
        Keyword-only alternative to date for the inclusive first exercise date.
        Default None; cannot be supplied together with date.
    end_date : Date or datetime.date or str, optional
        Inclusive final exercise date, no earlier than the start. Default None
        means exercise on the start date only.
    """

    def __new__(cls, date: _DateLike | None=..., strike: float=..., start_date: _DateLike | None=..., end_date: _DateLike | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> CallScheduleEntry:
        """Builds a validated native call entry from a dict.

        Parameters
        ----------
        obj : Mapping[str, Any]
            Native call terms: start_date (alias date), required numeric strike,
            and optional end_date, as returned by ``to_dict()``. Dates are ISO
            strings; absent/null end_date means a single call date. Unknown
            fields, reversed windows, and nonpositive/nonfinite strikes are rejected.

        Returns
        -------
        CallScheduleEntry
            Validated single-date call or inclusive call window.
        """
        ...

    @staticmethod
    def window(start_date: _DateLike, end_date: _DateLike, strike: float | Mapping[str, Any] | str) -> Any:
        """Creates an inclusive issuer-call window.

        Parameters
        ----------
        start_date : Date or datetime.date or str
            Inclusive first exercise date; ISO date strings are accepted.
        end_date : Date or datetime.date or str
            Inclusive final exercise date, on or after start_date.
        strike : float
            Finite positive call price per bond units, excluding same-day coupon
            and amortization. The runtime requires a number, not a tagged strike.

        Returns
        -------
        CallScheduleEntry
            Validated call window spanning both endpoint dates.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns the native JSON-compatible schema.

        Returns
        -------
        dict[str, Any]
            start_date, end_date, and numeric strike; dates are ISO strings and
            end_date is None for a single-date call. Accepted by ``from_dict()``.
        """
        ...

    def contains(self, date: _DateLike) -> Any:
        """Whether a date is inside the inclusive exercise window.

        Parameters
        ----------
        date : Date or datetime.date or str
            Exercise date to test, as a date object or ISO YYYY-MM-DD string.

        Returns
        -------
        bool
            True when start_date <= date <= end_date, including both endpoints.
        """
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

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class SteppedCouponBond(Instrument):
    """A callable bond with positional coupon and outstanding-notional schedules.

    Parameters
    ----------
    identifier : str
        Product identifier for the stepped-coupon contract.
    start_date : Date or datetime.date or str
        Effective date and start of the coupon schedule; ISO strings are accepted.
    maturity_date : Date or datetime.date or str
        Final contractual accrual and remaining principal repayment date.
    notional : float
        Original principal in currency; Trade may override the position amount.
    currency : Currency or str
        Coupon and principal currency enum or alphabetic code.
    coupons : Sequence[FixedRateCoupon or FloatingRateCoupon or OptionEmbeddedCoupon or Mapping[str, Any]]
        Nonempty positional coupon templates or native tagged coupon dictionaries.
        The last entry repeats through maturity; no more entries than coupon
        periods are allowed. Builders supply each period's balance and dates.
    notional_percentages : Sequence[float]
        Nonempty outstanding fractions of original principal for successive
        coupon periods, starting at 1.0 and non-increasing in (0, 1]. Values
        are fractions, not percentages; 0.8 means 80%. The last entry repeats
        to maturity, when remaining principal is repaid.
    call_schedule : Sequence[CallScheduleEntry or Mapping[str, Any]], optional
        Issuer call dates/windows as typed entries or native dictionaries.
        Default None is an empty schedule, with no issuer calls.
    units : float, optional
        Quote denomination for bond prices and call strikes; default 100.0.
    payment_frequency : Frequency or str, optional
        Regular coupon interval; default None resolves to Semiannual.
    coupon_day_counter : DayCounter or str, optional
        Convention used to rebuild fixed coupons with Simple/Annual rate
        conventions. Default None resolves to Actual365.
    calendar : Calendar or str, optional
        Schedule holiday calendar or serialized name. Default None uses the
        null calendar in the native builder.
    business_day_convention : BusinessDayConvention or str, optional
        Regular schedule adjustment rule; default None resolves to Unadjusted.
    date_generation_rule : DateGenerationRule or str, optional
        Rule/direction used to generate coupon dates. Default None uses Backward.
    end_of_month : bool, optional
        Preserve month-end alignment when generating dates. Default None uses False.
    first_coupon_date : Date or datetime.date or str, optional
        Explicit first coupon boundary for an initial stub; default None uses
        the regular generated boundary.
    next_to_last_coupon_date : Date or datetime.date or str, optional
        Explicit penultimate coupon boundary for a final stub; default None
        uses the regular generated boundary.
    termination_date_convention : BusinessDayConvention or str, optional
        Separate final-date adjustment rule. Default None resolves to Unadjusted.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, currency: _CurrencyLike, coupons: Sequence[FixedRateCoupon | FloatingRateCoupon | OptionEmbeddedCoupon | Mapping[str, Any]], notional_percentages: Any, call_schedule: Any | None=..., units: float=..., payment_frequency: Any | None=..., coupon_day_counter: Any | None=..., calendar: Calendar | str | None=..., business_day_convention: BusinessDayConvention | str | None=..., date_generation_rule: DateGenerationRule | str | None=..., end_of_month: bool | None=..., first_coupon_date: _DateLike | None=..., next_to_last_coupon_date: _DateLike | None=..., termination_date_convention: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns constructor fields with native nested coupon and call schemas.

        Returns
        -------
        dict[str, Any]
            JSON-compatible constructor fields for ``from_dict()``, including
            native coupon/payoff trees, notional fractions, and call windows;
            excludes valuation state and Trade metadata.
        """
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            SteppedCouponBond constructor keywords, including identifier, dates,
            notional, currency, coupons, and notional_percentages. Accepts
            ``to_dict()`` output with native coupon/payoff and call-window
            dictionaries; outstanding values are fractions, not percentages.

        Returns
        -------
        Self
            Stepped-coupon bond constructed through cls, preserving subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of SteppedCouponBond constructor
            fields, following ``from_dict()`` or ``to_dict()`` output. Preserve
            native nested coupon/payoff/call schemas and ISO date strings.

        Returns
        -------
        Self
            Stepped-coupon bond loaded through cls with constructor checks.
        """
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

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class FloatingRateNote(Instrument):
    """A floating-rate note.

    Parameters
    ----------
    identifier : str
        Product identifier for the floating-rate bond.
    start_date : Date or datetime.date or str
        First coupon accrual date; ISO date strings are accepted.
    maturity_date : Date or datetime.date or str
        Final contractual coupon and principal repayment date.
    notional : float
        Original principal in currency; Trade may override the amount.
    spread : float
        Annual decimal margin added to the forward index; 0.001 means 10 bp.
    currency : Currency or str
        Payment currency enum or alphabetic currency code.
    forward_index : MarketIndex or str
        Reference index for projected floating coupons, or native index representation.
    payment_frequency : Frequency or str, optional
        Coupon payment interval; default None resolves to Quarterly.
    payment_structure : PaymentStructure or str, optional
        Principal repayment enum/name, such as Bullet, EqualPayments, or
        EqualRedemptions. Default None uses Bullet; native construction checks
        the requested structure.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, spread: float, currency: _CurrencyLike, forward_index: _IndexLike, payment_frequency: Any | None=..., payment_structure: PaymentStructure | str | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            FloatingRateNote constructor keywords: identifier, start_date,
            maturity_date, notional, spread, currency, forward_index, and
            optional payment_frequency/payment_structure. Spread is an annual
            decimal margin and omitted options retain constructor defaults.

        Returns
        -------
        Self
            Floating-rate note constructed through cls, preserving subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of FloatingRateNote constructor
            keywords, following ``from_dict()`` with ISO dates, a currency code,
            and the native forward-index representation.

        Returns
        -------
        Self
            Floating-rate note loaded through cls with constructor defaults.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class FixedRateDeposit(Instrument):
    """A fixed-rate deposit.

    Parameters
    ----------
    identifier : str
        Product identifier for the deposit contract.
    start_date : Date or datetime.date or str
        Principal advance and accrual start date; ISO date strings are accepted.
    maturity_date : Date or datetime.date or str
        Principal repayment and end-of-interest-accrual date.
    notional : float
        Deposit principal in currency; Trade may override this amount.
    rate : float
        Annual decimal deposit rate, for example 0.05 for 5%.
    currency : Currency or str
        Principal/interest payment currency enum or alphabetic code.
    discount_index : MarketIndex or str, optional
        Explicit discount-curve index. Default None leaves native builder/pricing
        policy selection unchanged.
    day_counter : DayCounter or str, optional
        Deposit accrual basis; default None resolves to Actual360.
    compounding : Compounding or str, optional
        Interest accumulation convention; default None uses Simple.
    rate_frequency : Frequency or str, optional
        Rate compounding frequency; default None resolves to Annual.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, notional: float, rate: float, currency: _CurrencyLike, discount_index: _IndexLike | None=..., day_counter: DayCounter | str | None=..., compounding: Compounding | str | None=..., rate_frequency: Frequency | str | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            FixedRateDeposit constructor keywords: identifier, start_date,
            maturity_date, notional, annual decimal rate, currency, and optional
            discount_index/day_counter/compounding/rate_frequency settings.

        Returns
        -------
        Self
            Deposit constructed through cls with normal defaults and subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of FixedRateDeposit constructor
            keywords, as in ``from_dict()``. Dates are ISO strings and any
            explicit discount index uses native serde representation.

        Returns
        -------
        Self
            Fixed-rate deposit loaded through cls, preserving subclasses.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class FxForward(Instrument):
    """A deliverable or non-deliverable FX forward.

    Parameters
    ----------
    identifier : str
        Product identifier for the currency forward.
    delivery_date : Date or datetime.date or str
        Currency delivery/cash settlement date; ISO strings are accepted.
    base_currency : Currency or str
        Base currency enum or alphabetic code; forward price is quoted per
        unit of this currency. Trade carries the position notional and side.
    quote_currency : Currency or str
        Quote currency enum or alphabetic code, the numerator of the FX price.
    forward_price : float, optional
        Agreed outright price in quote-currency units per base-currency unit.
        Default None leaves it unset; native construction requires this or
        forward_points to be supplied.
    forward_points : float, optional
        Agreed forward-minus-spot price difference in the pair's quoted price
        units, not an annual rate. Default None leaves points unset.
    day_counter : DayCounter or str, optional
        Forward time basis. Default None resolves to Actual360 in the native builder.
    fixing_date : Date or datetime.date or str, optional
        NDF observation date, no later than delivery_date. Default None;
        the wrapper builds an NDF only when settlement_currency is also supplied.
    settlement_currency : Currency or str, optional
        NDF cash settlement currency enum or alphabetic code. Default None;
        if either NDF field is absent, the wrapper builds a deliverable forward.
    """

    def __new__(cls, identifier: str, delivery_date: _DateLike, base_currency: _CurrencyLike, quote_currency: _CurrencyLike, forward_price: Any | None=..., forward_points: Any | None=..., day_counter: DayCounter | str | None=..., fixing_date: _DateLike | None=..., settlement_currency: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            FxForward constructor keywords: identifier, delivery_date,
            base_currency, quote_currency, and optional quote/day-count/NDF
            fields. Supply forward_price or forward_points for native pricing
            construction; both NDF fields are needed for non-deliverable settlement.

        Returns
        -------
        Self
            Currency forward constructed through cls; native trade construction
            subsequently checks quote availability and NDF date ordering.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of FxForward constructor keywords,
            following ``from_dict()`` with ISO delivery/fixing dates and
            alphabetic base, quote, and optional settlement currency codes.

        Returns
        -------
        Self
            Deliverable/NDF forward specification loaded through cls.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class FxOption(Instrument):
    """A European FX option.

    Parameters
    ----------
    identifier : str
        Product identifier for the European currency option.
    expiry_date : Date or datetime.date or str
        Single European exercise date; ISO date strings are accepted.
    strike : float
        Numeric absolute exercise price in quote-currency units per base unit.
        The runtime requires a number; ATM strings and tagged Strike objects
        are not accepted by this wrapper.
    option_type : OptionType or str
        Call or Put on the base currency, as an enum or case-insensitive name.
    base_currency : Currency or str
        Underlying/base currency enum or alphabetic code; Trade carries position size.
    quote_currency : Currency or str
        Quote/payment currency enum or alphabetic code used for the strike.
    """

    def __new__(cls, identifier: str, expiry_date: _DateLike, strike: float | Mapping[str, Any] | str, option_type: OptionType | str, base_currency: _CurrencyLike, quote_currency: _CurrencyLike) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            FxOption constructor keywords: identifier, expiry_date, numeric
            absolute strike, option_type, base_currency, and quote_currency.
            Strike is quote currency per base unit; option_type is Call or Put.

        Returns
        -------
        Self
            European FX option constructed through cls, preserving subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of FxOption constructor fields,
            following ``from_dict()`` with an ISO expiry date, numeric strike,
            Call/Put name, and alphabetic currency codes.

        Returns
        -------
        Self
            European FX option loaded through cls.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class EquityOption(Instrument):
    """A European equity option.

    Parameters
    ----------
    identifier : str
        Product identifier for the European equity option.
    market_index : MarketIndex or str
        Underlying equity index or its native serialized representation.
    expiry_date : Date or datetime.date or str
        Single European exercise date; ISO date strings are accepted.
    strike : float or str or Mapping[str, Any]
        Numeric absolute exercise price per equity unit, case-insensitive
        "ATM", or a native tagged Strike dictionary: {"Absolute": value} or
        {"Relative": offset}. The equity pricer uses Strike.resolve: Relative
        adds this offset to the forward equity price, not a percentage.
        This helper convention is not universal across product pricers.
    option_type : OptionType or str
        Call or Put payoff direction, as an enum or case-insensitive name.
        Trade supplies the equity position size and long/short direction.
    """

    def __new__(cls, identifier: str, market_index: _IndexLike, expiry_date: _DateLike, strike: float | Mapping[str, Any] | str, option_type: OptionType | str) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            EquityOption constructor keywords: identifier, market_index,
            expiry_date, strike, and option_type. Strike accepts a numeric
            absolute price, ATM string, or native tagged Strike dictionary;
            option_type is Call or Put.

        Returns
        -------
        Self
            European equity option constructed through cls, preserving subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of EquityOption constructor fields,
            as in ``from_dict()``. Use ISO expiry and native index/Strike
            representations rather than display strings for tagged variants.

        Returns
        -------
        Self
            European equity option loaded through cls.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class CreditDefaultSwap(Instrument):
    """A single-name credit default swap.

    Parameters
    ----------
    identifier : str
        Product identifier for the single-name protection contract.
    start_date : Date or datetime.date or str
        Protection/premium accrual start; ISO date strings are accepted.
    maturity_date : Date or datetime.date or str
        End of protection and final contractual premium period.
    spread : float
        Annual decimal running premium; 0.01 means 100 basis points, not 1 bp.
    currency : Currency or str
        Premium/protection payment currency enum or alphabetic code.
    credit_index : MarketIndex or str
        Survival-curve index in native index representation. A parsed non-Credit
        index is converted to a Credit index using its display name; arbitrary
        bare borrower names are not parsed as index variants.
    discount_index : MarketIndex or str
        Discount-curve index for premium and protection cashflows.
    recovery : float, optional
        Recovery fraction of protected principal in [0, 1]; default 0.4.
    premium_frequency : Frequency or str, optional
        Premium payment interval; default None resolves to Quarterly.
    day_counter : DayCounter or str, optional
        Running premium accrual basis; default None resolves to Actual360.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, spread: float, currency: _CurrencyLike, credit_index: _IndexLike, discount_index: _IndexLike, recovery: float=..., premium_frequency: Any | None=..., day_counter: DayCounter | str | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            CreditDefaultSwap constructor keywords: identifier, start_date,
            maturity_date, annual decimal spread, currency, credit_index,
            discount_index, and optional recovery/premium_frequency/day_counter.
            Recovery is a fraction, not a percentage; index terms use native serde.

        Returns
        -------
        Self
            Single-name CDS specification constructed through cls; the native
            pricing trade applies the contract's economic validation.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of CreditDefaultSwap constructor
            keywords, following ``from_dict()`` with ISO dates and native
            serialized credit/discount indices.

        Returns
        -------
        Self
            CDS specification loaded through cls with constructor defaults.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class CapFloor(Instrument):
    """An interest-rate cap or floor.

    Parameters
    ----------
    identifier : str
        Product identifier for the strip of interest-rate optionlets.
    start_date : Date or datetime.date or str
        Start of the first covered accrual period; ISO strings are accepted.
    maturity_date : Date or datetime.date or str
        End of the last covered accrual period.
    strike : float
        Numeric annual decimal cap/floor rate, for example 0.04 for 4%.
        This wrapper does not accept ATM strings or tagged Strike dictionaries.
    notional : float
        Covered principal in currency; Trade may override the amount.
    currency : Currency or str
        Optionlet payment currency enum or alphabetic code.
    market_index : MarketIndex or str
        Floating reference index for the covered periods, or native index representation.
    cap_floor_type : CapFloorType or str
        Cap for upside rate protection or Floor for downside protection,
        supplied as an enum or case-insensitive name.
    frequency : Frequency or str, optional
        Optionlet/accrual interval; default None resolves to Quarterly.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, maturity_date: _DateLike, strike: float | Mapping[str, Any] | str, notional: float, currency: _CurrencyLike, market_index: _IndexLike, cap_floor_type: CapFloorType | str, frequency: Frequency | str | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            CapFloor constructor keywords: identifier, start_date, maturity_date,
            numeric annual decimal strike, notional, currency, market_index,
            cap_floor_type, and optional frequency. Direction is Cap or Floor;
            omitted frequency follows the constructor default.

        Returns
        -------
        Self
            Cap/floor strip constructed through cls, preserving subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of CapFloor constructor fields,
            following ``from_dict()`` with ISO dates, numeric rate strike,
            a Cap/Floor name, and native index serialization.

        Returns
        -------
        Self
            Cap/floor strip loaded through cls with constructor defaults.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class CapletFloorlet(Instrument):
    """A single caplet or floorlet.

    Parameters
    ----------
    identifier : str
        Product identifier for the single-period interest-rate option.
    fixing_date : Date or datetime.date or str
        Reference-rate observation date; ISO date strings are accepted.
    start_accrual_date : Date or datetime.date or str
        First day of the underlying interest accrual period.
    end_accrual_date : Date or datetime.date or str
        Final day used to measure the underlying accrual fraction.
    payment_date : Date or datetime.date or str
        Cash settlement date for the optionlet payoff.
    strike : float or str or Mapping[str, Any]
        Annual decimal absolute strike as a number, case-insensitive "ATM",
        or native tagged Strike dictionary: {"Absolute": rate} or
        {"Relative": pct}. The Black and Hull-White caplet pricers interpret
        Relative multiplicatively as forward * (1 + pct), unlike the generic
        Strike.resolve helper's additive forward + offset. For example,
        pct=0.10 raises a 0.04 forward to a 0.044 strike, not 0.14.
        Prefer an absolute numeric strike to avoid this consumer-specific
        convention. Notional belongs to Trade.
    currency : Currency or str
        Payoff currency enum or alphabetic currency code.
    market_index : MarketIndex or str
        Reference rate observed at fixing_date, or native index representation.
    payoff_type : CapletFloorletType or str
        Caplet (positive rate-minus-strike) or Floorlet (positive strike-minus-rate),
        as an enum or case-insensitive name.
    """

    def __new__(cls, identifier: str, fixing_date: _DateLike, start_accrual_date: _DateLike, end_accrual_date: _DateLike, payment_date: _DateLike, strike: float | Mapping[str, Any] | str, currency: _CurrencyLike, market_index: _IndexLike, payoff_type: CapletFloorletType | str) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            CapletFloorlet constructor keywords: identifier, fixing_date,
            start_accrual_date, end_accrual_date, payment_date, strike, currency,
            market_index, and payoff_type. Strike is a decimal absolute rate,
            ATM string, or native Strike dictionary; direction is Caplet/Floorlet.

        Returns
        -------
        Self
            Single-period rate option constructed through cls.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of CapletFloorlet constructor
            keywords, following ``from_dict()`` with ISO dates, native index
            and strike representations, and a Caplet/Floorlet name.

        Returns
        -------
        Self
            Caplet or floorlet specification loaded through cls.
        """
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

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@disjoint_base
class RateFutures(Instrument):
    """An exchange-traded interest-rate futures contract.

    Position side and number of contracts belong to Trade, not this instrument.

    Parameters
    ----------
    identifier : str
        Product/contract identifier for the listed rate future.
    start_date : Date or datetime.date or str
        Contract fixing/start date of the reference accrual period; ISO strings accepted.
    end_date : Date or datetime.date or str
        Final date of the reference rate's accrual period.
    futures_price : float
        Agreed futures index price, for example 95.25, corresponding to a 4.75%
        implied rate under the 100-minus-percent-rate convention; not a decimal rate.
    market_index : MarketIndex or str
        Reference rate index, such as SOFR or TermSOFR3m. Its metadata supplies
        day-count/rate conventions; native serialized index strings are accepted.
    contract_size : float, optional
        Monetary value per one futures price point for one contract. Default
        None uses 2500.0 (25 per basis point). Contract count belongs to Trade.
    """

    def __new__(cls, identifier: str, start_date: _DateLike, end_date: _DateLike, futures_price: Any, market_index: _IndexLike, contract_size: Any | None=...) -> Self:
        ...

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @classmethod
    def from_dict(cls, obj: Mapping[str, Any]) -> Self:
        """Builds a product using the same fields and defaults as its constructor.

        Parameters
        ----------
        obj : Mapping[str, Any]
            RateFutures constructor keywords: identifier, start_date, end_date,
            futures_price, market_index, and optional contract_size. Price uses
            index points, such as 95.25, and contract_size is monetary value
            per point; contract count belongs to Trade.

        Returns
        -------
        Self
            Rate future constructed through cls, preserving subclasses.
        """
        ...

    @classmethod
    def from_json(cls, path: str) -> Self:
        """Loads constructor fields from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object of RateFutures constructor fields,
            following ``from_dict()`` with ISO accrual dates, a numeric futures
            price, and native rate-index serialization.

        Returns
        -------
        Self
            Rate future specification loaded through cls.
        """
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
        """Read-only currency value per one futures price point for one contract.

The default 2500 per point corresponds to 25 per 0.01-point move, not
2500 per basis point. Position contract count belongs to Trade.
"""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible fields as a dictionary."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@final
class ScriptedProduct:
    """Dated payoff code evaluated inside an initialized pricing context."""

    def __repr__(self) -> str:
        ...

    @classmethod
    def from_tenors(cls, **parameters: Any) -> UnresolvedInstrument:
        """Retain product terms with explicit start_tenor/tenor, expiry_tenor,
        or delivery_tenor fields. Trade evaluation supplies actual dates.

        Parameters
        ----------
        parameters : Any
            Constructor keyword terms. Use separate tenor keywords in place of
            dates, with all other fields following the product constructor.
        """
        ...

    @staticmethod
    def from_dict(obj: Mapping[str, Any]) -> ScriptedProduct:
        """Builds a script from events [{date, code}], currency, and optional model assignments.

        Parameters
        ----------
        obj : Mapping[str, Any]
            Native script specification. Required events is a nonempty sequence
            of objects with date (alias event_date) and code (alias script);
            dates are ISO strings or typed dates, and code is payoff source.
            Required currency is a currency enum/code. Optional name (alias
            identifier) defaults to "script"; discount_index and result_variable
            default to None; models defaults to an empty list. discount_index
            uses native index serialization, result_variable names the result
            variable, and models uses native PricingModelConfiguration schemas.
            Unknown product/event fields and duplicate model indices are rejected.

        Returns
        -------
        ScriptedProduct
            Validated dated script specification, not yet priced or executed.
        """
        ...

    @staticmethod
    def from_json(path: str) -> ScriptedProduct:
        """Loads the same script schema from a JSON file.

        Parameters
        ----------
        path : str
            Filesystem path to a JSON object using the ``from_dict()`` script
            schema, with ISO event dates and native serialized currency,
            optional discount index, and pricing-model configurations.

        Returns
        -------
        ScriptedProduct
            Validated script specification loaded from the file.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Returns the script specification, including its native model schemas.

        Returns
        -------
        dict[str, Any]
            Native script fields with canonical name and event date/code keys,
            currency, optional discount_index/result_variable, and model list.
            Suitable for ``from_dict()`` or JSON serialization, not runtime state.
        """
        ...

    @property
    def name(self) -> str:
        """Product name."""
        ...

    @property
    def identifier(self) -> str:
        """Product identifier (an alias for name)."""
        ...

    def keys(self) -> list[str]:
        """Return serialized field names; supports dict(obj)."""
        ...

    def __getitem__(self, key: str, /) -> Any: ...

@final
class UnresolvedInstrument(Instrument):
    """Product terms created by from_tenors before a pricing context exists.

    Put in Trade for automatic resolution, or call resolve_instrument(context)
    once to retain a concrete contract across valuations.
    """

    def resolve_instrument(self, context: Any) -> Instrument:
        """Resolve product terms once against an initialized pricing context.

        Parameters
        ----------
        context : PricingContext
            Initialized market supplying the evaluation date, curves and fixings.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """Return an independent copy of the original product parameters."""
        ...
    def keys(self) -> list[str]:
        """Return the pending specification keys; supports dict(obj)."""
        ...
    def __getitem__(self, key: str, /) -> Any: ...
    def __repr__(self) -> str: ...

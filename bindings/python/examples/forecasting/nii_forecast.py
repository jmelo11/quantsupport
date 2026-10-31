"""Deterministic coupon NII with principal-driven replacement and planned business.

This example layer uses native QS cashflows, not a second loan calculator.
It supports single-currency loans without credit/default and simple-interest
term deposits. Residual principal cash/funding earns zero; fees, effective
interest accounting, non-maturity deposits and overnight compounding are outside
its scope. Run inside an active PricingContext, once per market scenario.
"""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass
from datetime import date, timedelta
import heapq
import math
from typing import Sequence

import pandas as pd
import quantsupport as qs


def _date(value) -> date:
    return date.fromisoformat(str(value))


def _advance(value: date, months: int) -> date:
    return _date(qs.Date.parse(value.isoformat()).advance(months, "Months"))


def _finite(value: float, name: str, *, nonnegative: bool = False) -> None:
    if not math.isfinite(value) or (nonnegative and value < 0):
        raise ValueError(f"{name} must be finite" +
                         (" and nonnegative" if nonnegative else ""))


@dataclass(frozen=True)
class ForecastPosition:
    """A native position plus the product group used for replacement rules."""

    name: str
    segment: str
    trade: qs.Trade


@dataclass(frozen=True)
class ForecastPortfolio:
    """Keep trade metadata alongside the equivalent native QS portfolio.

    QS Portfolio does not expose its member list to Python. Retain the trades
    here when building the book so that forecasts can attribute principal and
    income before native portfolio aggregation compresses compatible rows.
    """

    identifier: str
    positions: Sequence[ForecastPosition]

    def __post_init__(self):
        object.__setattr__(self, "positions", tuple(self.positions))
        names = [position.name for position in self.positions]
        if not self.identifier or not names or len(set(names)) != len(names):
            raise ValueError(
                "Provide a portfolio identifier and unique position names")
        if any(not p.name or not p.segment for p in self.positions):
            raise ValueError("Position names and segments must be nonempty")

    @property
    def native(self) -> qs.Portfolio:
        return qs.Portfolio(self.identifier, [p.trade for p in self.positions])


@dataclass(frozen=True)
class ProductionTemplate:
    """Terms and a transparent curve-plus-margin pricing rule for new business.

    Fixed coupons lock the start-to-maturity simple forward on ``day_counter``
    plus ``margin`` at origination. This is a pricing assumption, not a par
    amortizing-loan solver. Floating loans reset to ``index`` plus ``margin``.
    New instruments use unadjusted dates and zero fixing lag; a template's
    tenor is measured from each actual origination date.
    """

    name: str
    kind: str = "loan"
    tenor_months: int = 24
    side: str = "LongReceive"
    currency: str = "EUR"
    rate_type: str = "fixed"
    index: str = "EURIBOR3m"
    margin: float = 0.015
    day_counter: str = "Actual365"
    repayment: str = "French"
    payment_frequency: str = "Monthly"
    fixing_frequency: str = "Quarterly"
    index_tenor_months: int = 3
    prepayment: qs.PrepaymentModelConfiguration | None = None

    def __post_init__(self):
        if not self.name or self.kind not in ("loan", "deposit"):
            raise ValueError(
                "A template needs a name and kind 'loan' or 'deposit'")
        if self.side not in ("LongReceive", "PayShort"):
            raise ValueError(
                "Use LongReceive for assets or PayShort for liabilities")
        if self.rate_type not in ("fixed", "floating"):
            raise ValueError("rate_type must be fixed or floating")
        if self.kind == "deposit" and (self.rate_type != "fixed" or self.prepayment is not None):
            raise ValueError(
                "Term deposits support fixed rates and no prepayment model")
        for name in ("tenor_months", "index_tenor_months"):
            value = getattr(self, name)
            if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
                raise ValueError(f"{name} must be a positive integer")
        _finite(self.margin, "margin")
        qs.DayCounter.parse(self.day_counter)

    def build(self, context: qs.PricingContext, identifier: str,
              start: date, amount: float) -> tuple[qs.Trade, float]:
        """Return a native trade and its initial quoted all-in coupon."""
        maturity = _advance(start, self.tenor_months)
        curve = context.curve(self.index)
        if self.rate_type == "fixed":
            tau = qs.DayCounter.parse(
                self.day_counter).year_fraction(start, maturity)
            initial_rate = (curve.discount_factor(
                start) / curve.discount_factor(maturity) - 1) / tau + self.margin
        else:
            initial_rate = curve.forward_rate(start, _advance(
                start, self.index_tenor_months)) + self.margin
        common = dict(identifier=identifier, start_date=start, maturity_date=maturity,
                      notional=amount, currency=self.currency)
        if self.kind == "deposit":
            instrument = qs.FixedRateDeposit(
                **common, rate=initial_rate, discount_index=self.index,
                day_counter=self.day_counter, compounding="Simple",
            )
        else:
            coupon = (
                qs.FixedRateCoupon(rate=qs.InterestRate(
                    rate=initial_rate, day_counter=self.day_counter, compounding="Simple",
                )) if self.rate_type == "fixed" else qs.FloatingRateCoupon(
                    index=self.index, spread=self.margin, day_counter=self.day_counter,
                )
            )
            instrument = qs.Loan(
                **common, coupons=[coupon], repayment=self.repayment,
                payment_frequency=self.payment_frequency, amortization_rate=initial_rate,
                fixing_frequency=self.fixing_frequency,
                index_tenor=f"{self.index_tenor_months}M", prepayment=self.prepayment,
                calendar="NullCalendar", business_day_convention="Unadjusted", fixing_lag=0,
            )
        return qs.Trade(instrument, start, self.side), initial_rate


@dataclass(frozen=True)
class RolloverRule:
    """Replace scheduled principal (including maturity) and prepayments separately.

    The rule also applies to its replacement cohorts. With both fractions one,
    zero lag, and the same balance-sheet side, the segment balance stays constant.
    """

    segment: str
    template: ProductionTemplate
    scheduled_fraction: float = 1.0
    prepayment_fraction: float = 1.0
    lag_days: int = 0

    def __post_init__(self):
        if not self.segment:
            raise ValueError("A rollover rule needs a segment")
        for name in ("scheduled_fraction", "prepayment_fraction"):
            value = getattr(self, name)
            _finite(value, name)
            if not 0 <= value <= 1:
                raise ValueError(f"{name} must lie in [0, 1]")
        if not isinstance(self.lag_days, int) or isinstance(self.lag_days, bool) or self.lag_days < 0:
            raise ValueError("lag_days must be a nonnegative integer")


@dataclass(frozen=True)
class NewProduction:
    """An additional origination, independent of principal replacement."""

    date: date | str
    amount: float
    segment: str
    template: ProductionTemplate

    def __post_init__(self):
        object.__setattr__(self, "date", _date(self.date))
        _finite(self.amount, "production amount", nonnegative=True)
        if not self.segment:
            raise ValueError("New production needs a segment")


@dataclass
class ForecastResult:
    """Undiscounted income, positive asset/liability balances and audit tables."""

    monthly: pd.DataFrame
    income_detail: pd.DataFrame
    balances: pd.DataFrame
    productions: pd.DataFrame
    cashflows: pd.DataFrame


class IncomeForecaster:
    """Project each cohort once, then process dated runoff and production events.

    Accrual periods are [start, end). Balance movements are (start, end], with
    production on the forecast date included in the first period. Production on
    the horizon date affects closing balances but earns no in-horizon interest.
    All amounts retain the forecast currency; there is no implicit FX conversion.
    """

    def __init__(self, as_of, horizon, currency="EUR"):
        self.as_of, self.horizon = _date(as_of), _date(horizon)
        self.currency = str(currency)
        if self.horizon <= self.as_of:
            raise ValueError("The forecast horizon must follow as_of")
        self.periods = []
        start = self.as_of
        while start < self.horizon:
            following = date(start.year + start.month //
                             12, start.month % 12 + 1, 1)
            end = min(following, self.horizon)
            self.periods.append((start, end))
            start = end

    def forecast(self, context: qs.PricingContext, portfolio: ForecastPortfolio, *,
                 rollovers: Sequence[RolloverRule] = (),
                 planned: Sequence[NewProduction] = ()) -> ForecastResult:
        """Evaluate inside an active QS context, including an optional Scenario scope.

        Existing trades must already represent current contractual balances and
        actual historical prepayments. We do not reconstruct those from CPR.
        Each call starts fresh; neither input positions nor prior results mutate.
        """
        if _date(context.evaluation_date) != self.as_of:
            raise ValueError(
                "PricingContext evaluation_date must equal forecast as_of")
        rules = {rule.segment: rule for rule in rollovers}
        if len(rules) != len(rollovers):
            raise ValueError("Use at most one rollover rule per segment")
        segments = {p.segment for p in portfolio.positions} | {
            p.segment for p in planned}
        if not set(rules).issubset(segments):
            raise ValueError(
                "Rollover segments must occur in the portfolio or production plan")
        for template in [r.template for r in rollovers] + [p.template for p in planned]:
            if str(template.currency) != self.currency:
                raise ValueError(
                    "Production currency must match the forecast currency")

        dates, events = [], {}
        frames, income, movements, openings, production = [], [], [], [], []
        segment_sides = {}
        cohort_number = 0

        def enqueue(when, kind, payload):
            if when > self.horizon:
                return
            if when not in events:
                events[when] = []
                heapq.heappush(dates, when)
            events[when].append((kind, payload))

        def project(position, origin, start):
            nonlocal cohort_number
            cohort_number += 1
            cohort = f"{origin}-{cohort_number:04d}"
            side = self._validate_trade(position.trade)
            if origin == "existing" and _date(position.trade.instrument.start_date) > self.as_of:
                raise ValueError(
                    "Future originations belong in planned production")
            previous_side = segment_sides.setdefault(position.segment, side)
            if previous_side != side:
                raise ValueError(
                    "A segment must contain only assets or only liabilities")
            rule = rules.get(position.segment)
            if rule and (rule.template.side == "LongReceive") != (side == "asset"):
                raise ValueError(
                    "A rollover must preserve its segment's asset/liability side")
            table = self._cashflows(context, position.trade)
            table = table[table.payment_date > self.as_of.isoformat()].copy()
            if not set(table.currency).issubset({self.currency}):
                raise ValueError(
                    "Cashflow currency must match the forecast currency")
            table["cohort"], table["position"] = cohort, position.name
            table["segment"], table["origin"], table["book_side"] = position.segment, origin, side
            frames.append(table)
            self._accrue(table, income)
            principal = table[table.type == "Redemption"]
            direction = 1 if side == "asset" else -1
            amounts = principal.amount * direction
            if (amounts < -1e-8).any():
                raise ValueError(
                    "Principal direction must agree with the position side")
            balance = math.fsum(amounts)
            if origin == "existing":
                openings.append(dict(segment=position.segment,
                                book_side=side, amount=balance))
            else:
                movements.append(dict(date=start, segment=position.segment, book_side=side,
                                      kind="production", amount=balance))
            for row in principal.itertuples(index=False):
                when = _date(row.payment_date)
                if when <= start and origin != "existing":
                    raise ValueError(
                        "A new cohort must repay principal after its origination")
                is_prepayment = isinstance(
                    position.trade.instrument, qs.Loan) and row.leg_index == 1
                kind = "prepayment" if is_prepayment else "scheduled"
                amount = row.amount * direction
                if when <= self.horizon:
                    movements.append(dict(date=when, segment=position.segment,
                                          book_side=side, kind=kind, amount=amount))
                    enqueue(when, "principal",
                            (position.segment, kind, amount))
            return cohort

        for position in portfolio.positions:
            project(position, "existing", self.as_of)
        for order in planned:
            if not self.as_of <= order.date <= self.horizon:
                raise ValueError(
                    "Planned production must fall within the forecast dates")
            enqueue(order.date, "production", (order.segment,
                    "planned", order.template, order.amount))

        while dates:
            when = heapq.heappop(dates)
            batch = events.pop(when)
            orders = []
            for kind, payload in batch:
                if kind == "production":
                    orders.append(payload)
                    continue
                segment, principal_kind, amount = payload
                if segment not in rules:
                    continue
                rule = rules[segment]
                fraction = rule.prepayment_fraction if principal_kind == "prepayment" else rule.scheduled_fraction
                replacement = (segment, "replacement",
                               rule.template, amount * fraction)
                start = when + timedelta(days=rule.lag_days)
                if start == when:
                    orders.append(replacement)
                else:
                    enqueue(start, "production", replacement)
            # Pool same-day orders to avoid branching once per ancestor cohort.
            grouped = {}
            for segment, origin, template, amount in orders:
                key = (segment, origin, id(template))
                if key not in grouped:
                    grouped[key] = [template, []]
                grouped[key][1].append(amount)
            for (segment, origin, _), (template, amounts) in grouped.items():
                amount = math.fsum(amounts)
                if amount == 0:
                    continue
                identifier = f"{portfolio.identifier}-{origin}-{cohort_number + 1:04d}"
                trade, rate = template.build(context, identifier, when, amount)
                cohort = project(ForecastPosition(
                    identifier, segment, trade), origin, when)
                production.append(dict(date=when, segment=segment, origin=origin,
                                       cohort=cohort, template=template.name, amount=amount,
                                       initial_rate=rate, maturity_date=_date(trade.instrument.maturity_date)))

        detail = pd.DataFrame(income, columns=[
            "period_start", "period_end", "cohort", "position", "segment",
            "origin", "book_side", "interest",
        ])
        balances = self._balances(openings, movements)
        monthly = pd.DataFrame(self.periods, columns=[
                               "period_start", "period_end"])
        for side, column, sign in [("asset", "interest_income", 1), ("liability", "interest_expense", -1)]:
            totals = detail[detail.book_side == side].groupby(
                "period_start").interest.sum()
            monthly[column] = monthly.period_start.map(
                totals).fillna(0.0) * sign
            totals = balances[balances.book_side == side].groupby(
                "period_start").closing_balance.sum()
            monthly[f"{side}_balance"] = monthly.period_start.map(
                totals).fillna(0.0)
        monthly["nii"] = monthly.interest_income - monthly.interest_expense
        monthly["cumulative_nii"] = monthly.nii.cumsum()
        principal_cash = (
            balances.scheduled_principal + balances.prepayment - balances.production
        ) * balances.book_side.map({"asset": 1, "liability": -1})
        cash = balances.assign(cash=principal_cash).groupby(
            "period_start").cash.sum()
        monthly["net_principal_cashflow"] = monthly.period_start.map(
            cash).fillna(0.0)
        monthly["cumulative_principal_cash"] = monthly.net_principal_cashflow.cumsum()
        productions = pd.DataFrame(production, columns=[
            "date", "segment", "origin", "cohort", "template", "amount",
            "initial_rate", "maturity_date",
        ])
        return ForecastResult(monthly, detail, balances, productions, pd.concat(frames, ignore_index=True))

    @staticmethod
    def _cashflows(context: qs.PricingContext, trade: qs.Trade) -> pd.DataFrame:
        """Return signed coupon/principal receipts in the native coupon schema.

        Initial advances belong to the production ledger. Keep full coupon
        accrual dates here; the caller filters payments and allocates only the
        interest earned within the forecast. Loan rows already include native
        prepayment adjustments and position signs.
        """
        instrument = trade.instrument
        if isinstance(instrument, qs.Loan):
            return context.evaluate(trade, [qs.Request.Cashflows]).coupons.copy()
        if not isinstance(instrument, qs.FixedRateDeposit):
            raise ValueError(
                "This example supports Loan and FixedRateDeposit positions")
        # Native DCF valuation includes the deposit's historical advance. For an
        # income forecast, request only its contractual receipt economics using
        # a native coupon, avoiding discounting cash paid before the curve date.
        terms = instrument.to_dict()
        if terms["compounding"] != "Simple":
            raise ValueError("Monthly allocation requires simple-interest coupons")
        amount = instrument.notional if trade.notional is None else trade.notional
        direction = 1 if trade.side == qs.Side.LongReceive else -1
        rate = qs.InterestRate(
            rate=terms["rate"], day_counter=terms["day_counter"],
            compounding=terms["compounding"], frequency=terms["rate_frequency"],
        )
        coupon = qs.FixedRateCoupon(
            rate=rate, notional=1.0, accrual_start_date=terms["start_date"],
            accrual_end_date=terms["maturity_date"], payment_date=terms["maturity_date"],
        )
        common = dict(payment_date=terms["maturity_date"], currency=terms["currency"],
                      leg_index=0, fixing=None)
        accrual = qs.DayCounter.parse(terms["day_counter"]).year_fraction(
            terms["start_date"], terms["maturity_date"],
        )
        details = dict(
            accrual_start=terms["start_date"], accrual_end=terms["maturity_date"],
            market_index=None, notional=amount * direction, rate=terms["rate"],
            spread=None, day_counter=terms["day_counter"],
            rate_definition=rate.to_dict()["rate_definition"],
        )
        # Let the binding supply its standard columns and metadata formatting.
        result = qs.EvaluationResults(
            evaluation_date=context.evaluation_date, identifier=instrument.identifier,
            cashflows=[
                dict(common, type="FixedRateCoupon", amount=coupon.amount() * amount * direction,
                     accrual_period=accrual, coupon_details=details),
                dict(common, type="Redemption", amount=amount * direction),
            ],
        )
        return result.coupons

    def _validate_trade(self, trade):
        instrument = trade.instrument
        if not isinstance(instrument, (qs.Loan, qs.FixedRateDeposit)):
            raise ValueError(
                "This example supports Loan and FixedRateDeposit positions")
        terms = instrument.to_dict()
        if terms["currency"] != self.currency:
            raise ValueError(
                "Position currency must match the forecast currency")
        if isinstance(instrument, qs.Loan):
            if terms.get("credit_index") is not None:
                raise ValueError(
                    "Default/recovery accounting is outside this example")
            fixed = [coupon["FixedRateCoupon"]["rate"]["rate_definition"]["compounding"]
                     for coupon in terms["coupons"] if "FixedRateCoupon" in coupon]
        else:
            fixed = [terms["compounding"]]
        if any(compounding != "Simple" for compounding in fixed):
            raise ValueError(
                "Monthly allocation requires simple-interest coupons")
        return "asset" if trade.side == qs.Side.LongReceive else "liability"

    def _accrue(self, table, output):
        for row in table.itertuples(index=False):
            if row.type not in ("FixedRateCoupon", "FloatingRateCoupon", "OptionEmbeddedCoupon"):
                continue
            start, end = _date(row.accrual_start), _date(row.accrual_end)
            dc = qs.DayCounter.parse(row.day_counter)
            full = dc.year_fraction(start, end)
            if full <= 0:
                raise ValueError(
                    "Coupon accrual must have a positive year fraction")
            for period_start, period_end in self.periods:
                left, right = max(start, period_start), min(end, period_end)
                if right <= left:
                    continue
                # Cumulative fractions telescope even for non-additive day counts.
                fraction = (dc.year_fraction(start, right) -
                            dc.year_fraction(start, left)) / full
                output.append(dict(period_start=period_start, period_end=period_end,
                                   cohort=row.cohort, position=row.position, segment=row.segment,
                                   origin=row.origin, book_side=row.book_side, interest=row.amount * fraction))

    def _balances(self, openings, movements):
        opening = defaultdict(float)
        for row in openings:
            opening[(row["segment"], row["book_side"])] += row["amount"]
        grouped = defaultdict(list)
        for row in movements:
            grouped[(row["segment"], row["book_side"])].append(row)
        rows = []
        for segment, side in sorted(set(opening) | set(grouped)):
            balance = opening[(segment, side)]
            for start, end in self.periods:
                amounts = defaultdict(float)
                for event in grouped[(segment, side)]:
                    if (start < event["date"] <= end or event["date"] == start == self.as_of):
                        amounts[event["kind"]] += event["amount"]
                closing = balance + amounts["production"] - \
                    amounts["scheduled"] - amounts["prepayment"]
                rows.append(dict(period_start=start, period_end=end, segment=segment,
                                 book_side=side, opening_balance=balance,
                                 scheduled_principal=amounts["scheduled"], prepayment=amounts["prepayment"],
                                 production=amounts["production"], closing_balance=closing))
                balance = closing
        return pd.DataFrame(rows)

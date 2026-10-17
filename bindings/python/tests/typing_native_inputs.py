"""Static API contract; run with Pyright against the packaged stubs."""
from typing_extensions import assert_type

import quantsupport as qs

assert_type(qs.Currency.EUR, qs.Currency)
assert_type(qs.MarketIndex.EURIBOR12m, qs.MarketIndex)
assert_type(qs.MarketIndex.other(name="FUNDING"), qs.MarketIndex)
assert_type(qs.Frequency.Monthly, qs.Frequency)
assert_type(qs.Interpolator.Linear, qs.Interpolator)
assert_type(qs.Interpolator.parse(s="Linear"), qs.Interpolator)
assert_type(qs.DayCounter.Actual360, qs.DayCounter)
assert_type(qs.Request.Sensitivities, qs.Request)
start = qs.Date(year=2026, month=2, day=24)
period = qs.Period(length=30, units=qs.TimeUnit.Years)
assert_type(start + period, qs.Date)
assert_type((start + period) - start, int)
assert_type(qs.Period.from_frequency(freq=qs.Frequency.Monthly), qs.Period)
calendar = qs.Calendar.TARGET
assert_type(calendar.advance(date=start, period=period), qs.Date)
assert_type(qs.DayCounter.Actual360.year_fraction(start=start, end=start + period), float)
payoff = (2 * qs.PayoffOps.Index + 0.005).max(0.02).min(0.09)
assert_type(payoff, qs.PayoffOps)
assert_type(payoff.eval(index_fixing=0.04), float)
rate = qs.InterestRate(rate=0.06, day_counter=qs.DayCounter.Actual360)
assert_type(rate.day_counter, qs.DayCounter)
fixed = qs.FixedRateCoupon(rate=rate)
floating = qs.FloatingRateCoupon(index=qs.MarketIndex.EURIBOR12m, spread=0.01)
option = qs.OptionEmbeddedCoupon(index=qs.MarketIndex.EURIBOR12m, payoff=payoff)
assert_type(fixed.rate, qs.InterestRate)
assert_type(floating.index, qs.MarketIndex)
assert_type(option.payoff, qs.PayoffOps)
assert_type(option.amount(), float)


def reconfigure(context: qs.PricingContext, curves: list[qs.CurveConfiguration]) -> None:
    context.curves = curves
    assert_type(context.curves, list[qs.CurveConfiguration])
    assert_type(context.curve(index=qs.MarketIndex.ESTR), qs.DiscountCurve)
    assert_type(context.discount_curves(), list[qs.DiscountCurve])


def inspect_contract(loan: qs.Loan) -> None:
    assert_type(loan.notional, float)
    assert_type(loan.currency, qs.Currency)

assert_type(qs.Calendar.TARGET, qs.Calendar)
assert_type(qs.Calendar.UnitedStates(market=qs.UnitedStatesMarket.Sofr), qs.Calendar)
assert_type(qs.Calendar.JointCalendar(
    calendars=[qs.Calendar.TARGET, qs.Calendar.Japan],
    rule=qs.JointCalendarRule.JoinHolidays,
), qs.Calendar)


def scoped_market(context: qs.PricingContext) -> None:
    scenario = qs.Scenario(shocks=[qs.Shock(target="FUNDINGSPREAD", value=0.0001, shock_type=qs.ScenarioType.Absolute)])
    assert_type(scenario.shocks, list[qs.Shock])
    assert_type(scenario.shocks[0], qs.Shock)
    assert_type(scenario.shocks[0].target, str)
    assert_type(scenario.shocks[0].value, float)
    assert_type(scenario.shocks[0].shock_type, qs.ScenarioType)
    with context, scenario as active:
        assert_type(active, qs.Scenario)
        assert_type(context.curve(index=qs.MarketIndex.ESTR), qs.DiscountCurve)

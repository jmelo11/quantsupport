"""Static API contract checked against the packaged stubs."""
from typing import Any

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
assert_type(qs.PricerConfiguration.loan(), qs.PricerConfiguration)
assert_type(qs.PricerConfiguration.loan(
    refinancing_benchmark_index=qs.MarketIndex.EURIBOR12m,
    rebuild_amortization_for_fair_rate=True,
), qs.PricerConfiguration)
start = qs.Date(year=2026, month=2, day=24)
period = qs.Period(length=30, units=qs.TimeUnit.Years)
assert_type(start + period, qs.Date)
assert_type((start + period) - start, int)
assert_type(qs.Period.from_frequency(freq=qs.Frequency.Monthly), qs.Period)
calendar = qs.Calendar.TARGET
assert_type(calendar.advance(date=start, period=period), qs.Date)
assert_type(qs.DayCounter.Actual360.year_fraction(start=start, end=start + period), float)
assert_type(qs.DayCounter.Actual360.day_count(start=start, end=start + period), int)
payoff = (2 * qs.PayoffOps.Index + 0.005).max(0.02).min(0.09)
assert_type(payoff, qs.PayoffOps)
assert_type(payoff.eval(index_fixing=0.04), float)
rate = qs.InterestRate(rate=0.06, day_counter=qs.DayCounter.Actual360)
assert_type(qs.InterestRate(0.06, qs.DayCounter.Actual360), qs.InterestRate)
assert_type(dict(rate), dict[str, Any])
assert_type(rate.day_counter, qs.DayCounter)
fixed = qs.FixedRateCoupon(rate=rate)
assert_type(qs.FixedRateCoupon(rate), qs.FixedRateCoupon)
assert_type(dict(fixed), dict[str, Any])
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


def transformed_risk(context: qs.PricingContext, trade: qs.Trade) -> None:
    grid = qs.ZeroRateRisk(qs.MarketIndex.ESTR, dates=[start, "2028-02-24"],
                          day_counter=qs.DayCounter.Actual365,
                          compounding="Continuous", frequency=qs.Frequency.Annual)
    assert_type(grid, qs.ZeroRateRisk)
    assert_type(grid.dates, list[qs.Date] | None)
    assert_type(context.with_zero_rate_risk([grid]), qs.RiskContext)
    with context.reproject_risk(["OIS_EUR_ESTR_1Y", "OIS_EUR_ESTR_2Y"]) as view:
        assert_type(view, qs.RiskContext)
        assert_type(view.source_labels, list[str])
        assert_type(view.target_labels, list[str])
        assert_type(view.target_values, list[float])
        assert_type(view.evaluate(trade, [qs.Request.Sensitivities]), qs.EvaluationResults)
        assert_type(view.evaluate(qs.Portfolio("BOOK", [trade])), qs.EvaluationResults)


def inspect_contract(loan: qs.Loan) -> None:
    assert_type(loan.notional, float)
    assert_type(loan.currency, qs.Currency)

assert_type(qs.Calendar.TARGET, qs.Calendar)
assert_type(qs.Calendar.TARGET.business_days_between(from_=start, to=start + period), int)
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


def constant_contracts() -> None:
    """Keep exported native values classified as constants across domains."""
    qs.Frequency.Monthly = qs.Frequency.Annual  # type: ignore[misc]
    qs.Currency.EUR = qs.Currency.USD  # type: ignore[misc]
    qs.OptionType.Call = qs.OptionType.Put  # type: ignore[misc]
    qs.Request.Value = qs.Request.Cashflows()  # type: ignore[misc]
    qs.ScenarioType.Relative = qs.ScenarioType.Absolute  # type: ignore[misc]


def enum_parameter_contracts(fields: Any) -> None:
    """Accept native enum/string inputs and reject unrelated enum families.

    CI uses --warn-unused-ignores: a broadened parameter would make its
    negative example's suppression unused and fail the type check.
    """
    qs.CrossCurrencySwap(**fields, domestic_leg_frequency=qs.Frequency.Semiannual)
    qs.CrossCurrencySwap(**fields, domestic_leg_frequency="Semiannual")
    qs.CrossCurrencySwap(**fields, domestic_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.CrossCurrencySwap(**fields, foreign_leg_frequency=qs.Frequency.Semiannual)
    qs.CrossCurrencySwap(**fields, foreign_leg_frequency="Semiannual")
    qs.CrossCurrencySwap(**fields, foreign_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.BasisSwap(**fields, pay_leg_frequency=qs.Frequency.Semiannual)
    qs.BasisSwap(**fields, pay_leg_frequency="Semiannual")
    qs.BasisSwap(**fields, pay_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.BasisSwap(**fields, receive_leg_frequency=qs.Frequency.Semiannual)
    qs.BasisSwap(**fields, receive_leg_frequency="Semiannual")
    qs.BasisSwap(**fields, receive_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.FixFloatCrossCurrencySwap(**fields, domestic_leg_frequency=qs.Frequency.Semiannual)
    qs.FixFloatCrossCurrencySwap(**fields, domestic_leg_frequency="Semiannual")
    qs.FixFloatCrossCurrencySwap(**fields, domestic_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.FixFloatCrossCurrencySwap(**fields, foreign_leg_frequency=qs.Frequency.Semiannual)
    qs.FixFloatCrossCurrencySwap(**fields, foreign_leg_frequency="Semiannual")
    qs.FixFloatCrossCurrencySwap(**fields, foreign_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.FixedRateBond(**fields, payment_frequency=qs.Frequency.Semiannual)
    qs.FixedRateBond(**fields, payment_frequency="Semiannual")
    qs.FixedRateBond(**fields, payment_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.SteppedCouponBond(**fields, payment_frequency=qs.Frequency.Semiannual)
    qs.SteppedCouponBond(**fields, payment_frequency="Semiannual")
    qs.SteppedCouponBond(**fields, payment_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.SteppedCouponBond(**fields, coupon_day_counter=qs.DayCounter.Actual360)
    qs.SteppedCouponBond(**fields, coupon_day_counter="Actual360")
    qs.SteppedCouponBond(**fields, coupon_day_counter=qs.Frequency.Monthly)  # type: ignore[arg-type]
    qs.SteppedCouponBond(**fields, termination_date_convention=qs.BusinessDayConvention.ModifiedFollowing)
    qs.SteppedCouponBond(**fields, termination_date_convention="ModifiedFollowing")
    qs.SteppedCouponBond(**fields, termination_date_convention=qs.Frequency.Monthly)  # type: ignore[arg-type]
    qs.FloatingRateNote(**fields, payment_frequency=qs.Frequency.Semiannual)
    qs.FloatingRateNote(**fields, payment_frequency="Semiannual")
    qs.FloatingRateNote(**fields, payment_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.CreditDefaultSwap(**fields, premium_frequency=qs.Frequency.Semiannual)
    qs.CreditDefaultSwap(**fields, premium_frequency="Semiannual")
    qs.CreditDefaultSwap(**fields, premium_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeFloatFloatCrossCurrencySwap(**fields, domestic_leg_frequency=qs.Frequency.Semiannual)
    qs.MakeFloatFloatCrossCurrencySwap(**fields, domestic_leg_frequency="Semiannual")
    qs.MakeFloatFloatCrossCurrencySwap(**fields, domestic_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeFloatFloatCrossCurrencySwap(**fields, foreign_leg_frequency=qs.Frequency.Semiannual)
    qs.MakeFloatFloatCrossCurrencySwap(**fields, foreign_leg_frequency="Semiannual")
    qs.MakeFloatFloatCrossCurrencySwap(**fields, foreign_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeCrossCurrencySwap(**fields, domestic_leg_frequency=qs.Frequency.Semiannual)
    qs.MakeCrossCurrencySwap(**fields, domestic_leg_frequency="Semiannual")
    qs.MakeCrossCurrencySwap(**fields, domestic_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeCrossCurrencySwap(**fields, foreign_leg_frequency=qs.Frequency.Semiannual)
    qs.MakeCrossCurrencySwap(**fields, foreign_leg_frequency="Semiannual")
    qs.MakeCrossCurrencySwap(**fields, foreign_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeBasisSwap(**fields, pay_leg_frequency=qs.Frequency.Semiannual)
    qs.MakeBasisSwap(**fields, pay_leg_frequency="Semiannual")
    qs.MakeBasisSwap(**fields, pay_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeBasisSwap(**fields, receive_leg_frequency=qs.Frequency.Semiannual)
    qs.MakeBasisSwap(**fields, receive_leg_frequency="Semiannual")
    qs.MakeBasisSwap(**fields, receive_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeFixFloatCrossCurrencySwap(**fields, domestic_leg_frequency=qs.Frequency.Semiannual)
    qs.MakeFixFloatCrossCurrencySwap(**fields, domestic_leg_frequency="Semiannual")
    qs.MakeFixFloatCrossCurrencySwap(**fields, domestic_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeFixFloatCrossCurrencySwap(**fields, foreign_leg_frequency=qs.Frequency.Semiannual)
    qs.MakeFixFloatCrossCurrencySwap(**fields, foreign_leg_frequency="Semiannual")
    qs.MakeFixFloatCrossCurrencySwap(**fields, foreign_leg_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeFixedRateBond(**fields, payment_frequency=qs.Frequency.Semiannual)
    qs.MakeFixedRateBond(**fields, payment_frequency="Semiannual")
    qs.MakeFixedRateBond(**fields, payment_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeSteppedCouponBond(**fields, payment_frequency=qs.Frequency.Semiannual)
    qs.MakeSteppedCouponBond(**fields, payment_frequency="Semiannual")
    qs.MakeSteppedCouponBond(**fields, payment_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeSteppedCouponBond(**fields, coupon_day_counter=qs.DayCounter.Actual360)
    qs.MakeSteppedCouponBond(**fields, coupon_day_counter="Actual360")
    qs.MakeSteppedCouponBond(**fields, coupon_day_counter=qs.Frequency.Monthly)  # type: ignore[arg-type]
    qs.MakeSteppedCouponBond(**fields, termination_date_convention=qs.BusinessDayConvention.ModifiedFollowing)
    qs.MakeSteppedCouponBond(**fields, termination_date_convention="ModifiedFollowing")
    qs.MakeSteppedCouponBond(**fields, termination_date_convention=qs.Frequency.Monthly)  # type: ignore[arg-type]
    qs.MakeFloatingRateNote(**fields, payment_frequency=qs.Frequency.Semiannual)
    qs.MakeFloatingRateNote(**fields, payment_frequency="Semiannual")
    qs.MakeFloatingRateNote(**fields, payment_frequency=qs.Currency.EUR)  # type: ignore[arg-type]
    qs.MakeCreditDefaultSwap(**fields, premium_frequency=qs.Frequency.Semiannual)
    qs.MakeCreditDefaultSwap(**fields, premium_frequency="Semiannual")
    qs.MakeCreditDefaultSwap(**fields, premium_frequency=qs.Currency.EUR)  # type: ignore[arg-type]

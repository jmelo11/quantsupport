"""Typed market inputs and public context reassignment."""
import inspect
import math
import unittest
from datetime import date

import quantsupport as qs

REFERENCE = "2026-02-24"
SPREAD = qs.MarketIndex.other(name="SPREAD")
FUNDING = qs.MarketIndex.other(name="FUNDING")


def zero_curve(index, rates):
    return qs.CurveConfiguration(
        market_index=index,
        day_counter=qs.DayCounter.Actual360,
        interpolator=qs.Interpolator.Linear,
        construction=qs.CurveConstruction.zero_rate(
            pillars=[qs.Period.parse("0D"), date(2027, 2, 24), "2029-02-24"],
            rates=rates,
            compounding=qs.Compounding.Continuous,
            frequency=qs.Frequency.Annual,
        ),
    )


def context_inputs():
    tenors = ["1D", "1Y", "2Y", "3Y"]
    identifiers = [f"FixedRateDeposit_EUR_ESTR_{tenor}" for tenor in tenors]
    return qs.PricingContext(
        quotes=qs.QuoteStore.from_dict({"reference_date": REFERENCE, "quotes": [
            {"identifier": identifier, "mid": 0.03} for identifier in identifiers
        ]}),
        curves=[qs.CurveConfiguration(
            market_index=qs.MarketIndex.ESTR,
            quotes=identifiers,
            construction=qs.CurveConstruction.bootstrapped(),
        )],
        csa_policy=qs.SingleCurveCSADiscountPolicy(
            collateral_currency=qs.Currency.EUR,
            collateral_index=qs.MarketIndex.ESTR,
        ),
    )


class PythonMarketInputTests(unittest.TestCase):
    def test_typed_curves_price_composition_with_dates_and_tenors(self):
        context = context_inputs()
        context.curves += [
            qs.CurveConfiguration(
                market_index=FUNDING,
                construction=qs.CurveConstruction.composite(
                    base_index=qs.MarketIndex.ESTR, spread_index=SPREAD,
                ),
            ),
            zero_curve(SPREAD, [0.01] * 3),
        ]
        with context:
            tau = qs.DayCounter.Actual360.year_fraction(start=REFERENCE, end="2028-02-24")
            benchmark = context.curve(index=qs.MarketIndex.ESTR).discount_factor("2028-02-24")
            self.assertAlmostEqual(
                context.curve(index=FUNDING).discount_factor("2028-02-24"),
                benchmark * math.exp(-0.01 * tau), places=13,
            )

    def test_public_scenario_reuses_the_base_context(self):
        context = context_inputs()
        scenario = qs.Scenario(shocks=[qs.Shock(
                                           target="ESTR", value=0.01,
                                           shock_type=qs.ScenarioType.Absolute,
                                       )])
        with context:
            base = context.curve(index=qs.MarketIndex.ESTR).discount_factor("2028-02-24")
            with scenario:
                stressed = context.curve(index=qs.MarketIndex.ESTR).discount_factor("2028-02-24")
                self.assertLess(stressed, base)
                self.assertAlmostEqual(context.quotes.value("FixedRateDeposit_EUR_ESTR_1Y"), 0.04)
            self.assertAlmostEqual(context.curve("ESTR").discount_factor("2028-02-24"), base)
            with scenario:
                self.assertAlmostEqual(context.curve("ESTR").discount_factor("2028-02-24"), stressed)

    def test_curve_reassignment_rebuilds_and_failed_input_cannot_use_stale_market(self):
        context = context_inputs()
        original = context.curves
        with context:
            context.curves = original + [zero_curve(SPREAD, [0.01] * 3)]
            before = context.curve(SPREAD).discount_factor("2028-02-24")
            context.curves = original + [zero_curve(SPREAD, [0.02] * 3)]
            after = context.curve(SPREAD).discount_factor("2028-02-24")
            self.assertAlmostEqual(after, before ** 2)
            context.curves = original + [qs.CurveConfiguration(
                market_index=FUNDING,
                construction=qs.CurveConstruction.composite(base_index="ESTR", spread_index=SPREAD),
            )]
            with self.assertRaisesRegex(qs.QuantSupportError, "SPREAD"):
                context.curve(FUNDING)
            context.curves = original
            context.initialize()
            self.assertEqual(len(context.discount_curves()), 1)

    def test_public_fixings_and_policy_are_available_after_reassignment(self):
        context = context_inputs()
        fixings = qs.FixingStore()
        fixings.add(index="EURIBOR12m", date=REFERENCE, value=0.04)
        context.fixings = fixings
        self.assertAlmostEqual(context.fixings.fixing(index="EURIBOR12m", date=REFERENCE), 0.04)
        policy = qs.FixedIncomeDiscountPolicy(discount_curve_by_currency={qs.Currency.EUR: qs.MarketIndex.ESTR})
        context.fixed_income_policy = policy
        self.assertIsInstance(context.fixed_income_policy, qs.FixedIncomeDiscountPolicy)
        context.fixed_income_policy = None
        self.assertIsNone(context.fixed_income_policy)

    def test_constructor_signatures_and_parameter_help(self):
        for constructor, parameter in [
            (qs.CurveConfiguration, "market_index"),
            (qs.CurveConstruction.zero_rate, "pillars"),
            (qs.CurveConstruction.composite, "base_index"),
            (qs.FixedIncomeDiscountPolicy, "discount_curve_by_currency"),
            (qs.Shock, "target"),
            (qs.Shock, "value"),
            (qs.Shock, "shock_type"),
            (qs.Scenario, "shocks"),
        ]:
            with self.subTest(constructor=constructor):
                self.assertIn(parameter, inspect.signature(constructor).parameters)
                self.assertIn(parameter, constructor.__doc__)
                self.assertIn("Parameters", constructor.__doc__)

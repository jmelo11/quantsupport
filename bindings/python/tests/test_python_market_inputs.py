"""Typed market inputs and public context reassignment."""
import inspect
import math
import unittest

import quantsupport as qs

from market_fixtures import append_quotes, zero_rate_quotes

REFERENCE = "2026-02-24"
SPREAD = qs.MarketIndex.other(name="SPREAD")
FUNDING = qs.MarketIndex.other(name="FUNDING")


PILLARS = ["0D", "2027-02-24", "2029-02-24"]


def zero_curve(index):
    return qs.CurveConfiguration(
        market_index=index, day_counter=qs.DayCounter.Actual360,
        interpolator=qs.Interpolator.Linear,
        quotes=[f"ZeroRate_EUR_{index}_{pillar}" for pillar in PILLARS],
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
        )],
        csa_policy=qs.SingleCurveCSADiscountPolicy(
            collateral_currency=qs.Currency.EUR,
            collateral_index=qs.MarketIndex.ESTR,
        ),
    )


class PythonMarketInputTests(unittest.TestCase):
    def test_typed_loan_pricer_settings_accept_indices_and_roundtrip(self):
        defaults = qs.PricerConfiguration.loan().to_dict()
        self.assertEqual(defaults, {
            "instrument_type": "Loan",
            "pricer": {"Loan": {
                "refinancing_benchmark_index": None,
                "rebuild_amortization_for_fair_rate": False,
            }},
            "models": [],
        })
        for index in [qs.MarketIndex.EURIBOR12m, "EURIBOR12m", qs.MarketIndex.other("REFINANCING")]:
            with self.subTest(index=index):
                configuration = qs.PricerConfiguration.loan(
                    refinancing_benchmark_index=index,
                    rebuild_amortization_for_fair_rate=True,
                )
                serialized = configuration.to_dict()
                self.assertEqual(qs.PricerConfiguration.from_dict(serialized).to_dict(), serialized)
                self.assertTrue(serialized["pricer"]["Loan"]["rebuild_amortization_for_fair_rate"])
                self.assertIsNotNone(serialized["pricer"]["Loan"]["refinancing_benchmark_index"])
        self.assertEqual(qs.PricerConfiguration.loan(qs.MarketIndex.EURIBOR12m, True).to_dict(),
                         qs.PricerConfiguration.loan("EURIBOR12m", True).to_dict())
        with self.assertRaises(TypeError):
            qs.PricerConfiguration.loan(refinancing_benchmark_index=42)

    def test_typed_curves_price_composition_with_dates_and_tenors(self):
        context = context_inputs()
        context.quotes = append_quotes(context.quotes, zero_rate_quotes(SPREAD, PILLARS, [0.01] * 3))
        context.curves += [
            qs.CurveConfiguration.composite(market_index=FUNDING, base_index=qs.MarketIndex.ESTR, spread_index=SPREAD),
            zero_curve(SPREAD),
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
            context.quotes = append_quotes(context.quotes, zero_rate_quotes(SPREAD, PILLARS, [0.01] * 3))
            context.curves = original + [zero_curve(SPREAD)]
            before = context.curve(SPREAD).discount_factor("2028-02-24")
            context.quotes = append_quotes(context.quotes, zero_rate_quotes(SPREAD, PILLARS, [0.02] * 3))
            after = context.curve(SPREAD).discount_factor("2028-02-24")
            self.assertAlmostEqual(after, before ** 2)
            context.curves = original + [qs.CurveConfiguration.composite(market_index=FUNDING, base_index='ESTR', spread_index=SPREAD)]
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

    def test_curve_values_and_construction_shortcut_are_not_configuration_arguments(self):
        self.assertFalse(hasattr(qs, "CurveConstruction"))
        self.assertNotIn("construction", inspect.signature(qs.CurveConfiguration).parameters)
        with self.assertRaises(TypeError):
            qs.CurveConfiguration(market_index="SOFR", construction=None)
        with self.assertRaisesRegex(qs.QuantSupportError, "unknown field.*rates"):
            qs.CurveConfiguration.from_dict({"market_index": "SOFR", "rates": [0.03]})

    def test_constructor_signatures_and_parameter_help(self):
        for constructor, parameter in [
            (qs.CurveConfiguration, "market_index"),
            (qs.CurveConfiguration.composite, "base_index"),
            (qs.PricerConfiguration.loan, "refinancing_benchmark_index"),
            (qs.PricerConfiguration.loan, "rebuild_amortization_for_fair_rate"),
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

"""Temporary quoted and explicit-pillar shocks on reusable pricing contexts."""
import unittest
import math
from pathlib import Path

import quantsupport as qs
from test_composite_loan_funding import FUNDING, funding_context, loan
from test_python_market_inputs import context_inputs


class TypedCalendarTests(unittest.TestCase):
    def test_native_calendar_variants_and_market_enums(self):
        calendar = qs.Calendar.TARGET
        holiday = qs.Date(year=2026, month=12, day=25)
        self.assertTrue(calendar.is_holiday(date=holiday))
        self.assertEqual(calendar.adjust(date=holiday), qs.Date(year=2026, month=12, day=28))
        for typed, name in [
            (qs.Calendar.NullCalendar, "NullCalendar"),
            (qs.Calendar.WeekendsOnly, "WeekendsOnly"),
            (qs.Calendar.UnitedKingdom, "UnitedKingdom(Settlement)"),
            (qs.Calendar.Japan, "Japan"), (qs.Calendar.Chile, "Chile(SSE)"),
            (qs.Calendar.UnitedStates(market=qs.UnitedStatesMarket.Nyse), "UnitedStates(Nyse)"),
            (qs.Calendar.Brazil(market=qs.BrazilMarket.Exchange), "Brazil(Exchange)"),
        ]:
            self.assertEqual(typed.name(), name)
        joint = qs.Calendar.JointCalendar(
            calendars=[qs.Calendar.TARGET, qs.Calendar.Japan],
            rule=qs.JointCalendarRule.JoinHolidays)
        self.assertTrue(joint.is_holiday(date=holiday))
        with self.assertRaises(qs.QuantSupportError):
            qs.Calendar.JointCalendar(calendars=[calendar])
        with self.assertRaises(TypeError):
            qs.Calendar.UnitedStates(market="Nyse")


class ScopedScenarioTests(unittest.TestCase):
    def test_single_public_class_and_typed_shock_dictionaries(self):
        self.assertFalse(hasattr(qs, "Scenarios"))
        shock = {"target": "ESTR", "value": 0.01, "type": qs.ScenarioType.Absolute}
        scenario = qs.Scenario(shocks=[shock])
        shock["value"] = 0.02
        self.assertEqual(scenario.shocks[0]["value"], 0.01)
        copied = scenario.shocks
        copied[0]["value"] = 0.03
        self.assertEqual(scenario.shocks[0]["value"], 0.01)
        for invalid in [
            {"target": "ESTR", "value": 0.01},
            {"target": "", "value": 0.01, "type": qs.ScenarioType.Absolute},
            {"target": "ESTR", "value": float("inf"), "type": qs.ScenarioType.Absolute},
            {"target": "ESTR", "value": 0.01, "type": qs.ScenarioType.Absolute, "name": "unused"},
        ]:
            with self.subTest(shock=invalid), self.assertRaises(ValueError):
                qs.Scenario(shocks=[invalid])

    def test_unrelated_invalid_curves_and_simulations_are_not_constructed(self):
        context = context_inputs()
        context.curves += [qs.CurveConfiguration(
            market_index=qs.MarketIndex.other(name="UNRELATED"), quotes=["MISSING_QUOTE"],
        )]
        context.simulation_configurations = [qs.SimulationConfiguration.from_dict({
            "market_index": {"Other": "MISSING_SIMULATION_CURVE"},
            "model": {"HullWhite": {"alpha": 0.1, "parameter_source": {"Fixed": {"sigma": 0.01}}}},
            "n_paths": 1_000_000, "horizon": "30Y",
        })]
        scenario = qs.Scenario(shocks=[{"target": "ESTR", "value": 0.01, "type": qs.ScenarioType.Absolute}])
        with context:
            base_curve = context.curve(index=qs.MarketIndex.ESTR)
            base = self.factor(context)
            with scenario:
                self.assertLess(self.factor(context), base)
                self.assertAlmostEqual(base_curve.discount_factor(date="2028-02-24"), base)
            self.assertAlmostEqual(self.factor(context), base)
            with self.assertRaisesRegex(qs.QuantSupportError, "MISSING_QUOTE"):
                context.curve(index=qs.MarketIndex.other(name="UNRELATED"))

    def test_active_scenario_cache_tracks_public_input_reassignment(self):
        context = funding_context()
        replacement = funding_context(funding_shift=0.002).curves
        scenario = qs.Scenario(shocks=[{"target": "FUNDINGSPREAD", "value": 0.005, "type": qs.ScenarioType.Absolute}])
        with context, scenario:
            before = context.curve(index=FUNDING).discount_factor(date="2028-02-24")
            context.curves = replacement
            after = context.curve(index=FUNDING).discount_factor(date="2028-02-24")
            tau = qs.DayCounter.Actual360.year_fraction(start="2026-02-24", end="2028-02-24")
            self.assertAlmostEqual(after / before, math.exp(-0.002 * tau))
        self.assertEqual([curve.to_dict() for curve in context.curves], [curve.to_dict() for curve in replacement])

    def factor(self, context):
        return context.curve(index=qs.MarketIndex.ESTR).discount_factor(date="2028-02-24")

    def test_reuse_restores_quotes_inputs_and_composite_curves(self):
        context = funding_context()
        scenarios = qs.Scenario(shocks=[
            {"target": "EURIBOR12m", "value": 0.01, "type": qs.ScenarioType.Absolute},
            {"target": "FUNDINGSPREAD", "value": 0.005, "type": qs.ScenarioType.Absolute},
        ])
        original = [curve.to_dict() for curve in context.curves]
        for _ in range(2):
            with context:
                base = context.curve(index=FUNDING).discount_factor(date="2028-02-24")
                with scenarios:
                    shocked = context.curve(index=FUNDING).discount_factor(date="2028-02-24")
                    self.assertLess(shocked, base)
                    self.assertAlmostEqual(context.quotes.value("FixedRateDeposit_EUR_EURIBOR12m_1Y"), 0.05)
                self.assertAlmostEqual(context.curve(index=FUNDING).discount_factor(date="2028-02-24"), base)
                self.assertAlmostEqual(context.quotes.value("FixedRateDeposit_EUR_EURIBOR12m_1Y"), 0.04)
                self.assertEqual([curve.to_dict() for curve in context.curves], original)
            with context, scenarios:
                self.assertAlmostEqual(context.curve(index=FUNDING).discount_factor(date="2028-02-24"), shocked)

    def test_exception_and_failed_entry_leave_previous_market_usable(self):
        context = context_inputs()
        with context:
            base = self.factor(context)
            with self.assertRaisesRegex(ValueError, "body failure"):
                with qs.Scenario(shocks=[{"target": "ESTR", "value": 0.01, "type": qs.ScenarioType.Absolute}]):
                    self.assertLess(self.factor(context), base)
                    raise ValueError("body failure")
            self.assertAlmostEqual(self.factor(context), base)
            for target in ["MISSING", "ESTR"]:
                shock = 0.01 if target == "MISSING" else float("nan")
                with self.assertRaises((qs.QuantSupportError, ValueError)):
                    with qs.Scenario(shocks=[{"target": target, "value": shock, "type": qs.ScenarioType.Absolute}]):
                        self.fail("invalid scenario entered")
                self.assertAlmostEqual(self.factor(context), base)

    def test_nested_scopes_compose_and_restore_parent_assumptions(self):
        context = context_inputs()
        outer = qs.Scenario(shocks=[{"target": "ESTR", "value": 0.01, "type": qs.ScenarioType.Absolute}])
        inner = qs.Scenario(shocks=[{"target": "ESTR", "value": 0.10, "type": qs.ScenarioType.Relative}])
        with context:
            base = self.factor(context)
            with outer:
                outer_factor = self.factor(context)
                with inner:
                    self.assertAlmostEqual(context.quotes.value("FixedRateDeposit_EUR_ESTR_1Y"), 0.04 * 1.10)
                    self.assertLess(self.factor(context), outer_factor)
                self.assertAlmostEqual(self.factor(context), outer_factor)
                with context:
                    self.assertAlmostEqual(self.factor(context), outer_factor)
                self.assertAlmostEqual(self.factor(context), outer_factor)
            self.assertAlmostEqual(self.factor(context), base)
            self.assertAlmostEqual(context.quotes.value("FixedRateDeposit_EUR_ESTR_1Y"), 0.03)

    def test_shocks_bind_to_innermost_context_and_require_an_active_scope(self):
        scenarios = qs.Scenario(shocks=[{"target": "ESTR", "value": 0.01, "type": qs.ScenarioType.Absolute}])
        with self.assertRaisesRegex(RuntimeError, "active PricingContext"):
            with scenarios:
                self.fail("unbound scenarios entered")
        first, second = context_inputs(), context_inputs()
        with first:
            base = self.factor(first)
            with second, scenarios:
                self.assertLess(self.factor(second), base)
                self.assertAlmostEqual(self.factor(first), base)
                with self.assertRaisesRegex(RuntimeError, "already active"):
                    with scenarios:
                        self.fail("same object entered twice")
            self.assertAlmostEqual(self.factor(first), base)

    def test_calibrated_quote_sensitivities_survive_scenario_restoration(self):
        data = Path(__file__).resolve().parents[3] / "examples" / "valuation" / "loans" / "data"
        context = funding_context()
        context.quotes = qs.QuoteStore.from_json(path=str(data / "loan_quotes.json"))
        context.curves = qs.CurveConfiguration.from_json(path=str(data / "loan_curves.json")) + context.curves[-2:]
        pricer = qs.LoanPricer()
        trade = qs.Trade(instrument=loan(floating=True), trade_date="2026-02-24")
        requests = [qs.Request.Value, qs.Request.Sensitivities]
        with context:
            base = pricer.evaluate(trade=trade, context=context, requests=requests)
            with qs.Scenario(shocks=[
                {"target": "EURIBOR12m", "value": 0.01, "type": qs.ScenarioType.Absolute},
                {"target": "FUNDINGSPREAD", "value": 0.005, "type": qs.ScenarioType.Absolute},
            ]):
                stressed = pricer.evaluate(trade=trade, context=context, requests=requests)
                self.assertNotAlmostEqual(stressed.price, base.price)
            restored = pricer.evaluate(trade=trade, context=context, requests=requests)
            self.assertAlmostEqual(restored.price, base.price)
            before = dict(zip(base.sensitivities.pillar, base.sensitivities.value))
            after = dict(zip(restored.sensitivities.pillar, restored.sensitivities.value))
            self.assertEqual(before.keys(), after.keys())
            for label, value in before.items():
                self.assertAlmostEqual(value, after[label], delta=max(0.02, abs(value) * 1e-7))

    def test_funding_pillar_sensitivities_match_scoped_repricing(self):
        context = funding_context()
        pricer = qs.LoanPricer()
        epsilon = 1e-6
        for floating in [False, True]:
            trade = qs.Trade(instrument=loan(floating=floating), trade_date="2026-02-24")
            with context:
                base = pricer.evaluate(trade=trade, context=context,
                    requests=[qs.Request.Value, qs.Request.Sensitivities, qs.Request.Cashflows])
                funding_risk = base.sensitivities.loc[base.sensitivities.pillar.str.startswith("FUNDINGSPREAD_")]
                self.assertEqual(len(funding_risk), 4)
                for row in funding_risk.itertuples(index=False):
                    prices = []
                    for shift in [-epsilon, epsilon]:
                        with qs.Scenario(shocks=[{"target": row.pillar, "value": shift, "type": qs.ScenarioType.Absolute}]):
                            result = pricer.evaluate(trade=trade, context=context,
                                requests=[qs.Request.Value, qs.Request.Cashflows])
                            prices.append(result.price)
                            self.assertEqual(result.cashflows.amount.tolist(), base.cashflows.amount.tolist())
                    numerical = (prices[1] - prices[0]) / (2 * epsilon)
                    self.assertAlmostEqual(row.value, numerical, delta=max(0.02, abs(numerical) * 1e-6))
                restored = pricer.evaluate(trade=trade, context=context,
                    requests=[qs.Request.Value, qs.Request.Sensitivities])
                self.assertAlmostEqual(restored.price, base.price)
                self.assertEqual(restored.sensitivities.pillar.tolist(), base.sensitivities.pillar.tolist())
                for before, after in zip(base.sensitivities.value, restored.sensitivities.value):
                    self.assertAlmostEqual(before, after, delta=max(0.02, abs(before) * 1e-7))

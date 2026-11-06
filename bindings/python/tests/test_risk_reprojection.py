"""Python curve-risk views preserve valuation and native lifecycle behavior."""

import datetime
import math
import unittest

import pandas as pd
import quantsupport as qs


DEPOSITS = ["FixedRateDeposit_EUR_ESTR_1Y", "FixedRateDeposit_EUR_ESTR_2Y"]
OIS = ["OIS_EUR_ESTR_1Y", "OIS_EUR_ESTR_2Y"]
REQUESTS = [qs.Request.Value, qs.Request.Sensitivities]


def context(level="Mid", bump=0.0):
    quotes = qs.QuoteStore.from_dict({
        "reference_date": "2026-01-15",
        "quotes": [
            {"identifier": label, "mid": rate + bump,
             "bid": rate - 0.001 + bump, "ask": rate + 0.001 + bump}
            for label, rate in zip(DEPOSITS, [0.03, 0.04])
        ] + [{"identifier": "ZeroRate_EUR_UNUSED_1Y", "mid": 0.05}],
    })
    curves = [qs.CurveConfiguration("ESTR", DEPOSITS, "Actual365", "LogLinear", True)]
    return qs.PricingContext(
        quotes, curves, csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR"),
        quote_level=level,
    )


def trade():
    instrument = qs.FixedRateDeposit(
        "DEPOSIT-18M", "2026-01-15", "2027-07-15", 1_000_000, 0.035, "EUR",
        discount_index="ESTR",
    )
    return qs.Trade(instrument, "2026-01-15")


def exposures(result):
    return result.sensitivities.set_index("pillar")["value"].sort_index()


class RiskReprojectionTests(unittest.TestCase):
    def test_trade_evaluate_uses_the_innermost_risk_context(self):
        position = trade()
        with context() as original:
            native = position.evaluate(REQUESTS)
            view = original.with_zero_rate_risk([qs.ZeroRateRisk("ESTR")])
            with view:
                projected = position.evaluate(REQUESTS)
                self.assertEqual(projected.price, native.price)
                self.assertEqual(set(exposures(projected).index), set(view.target_labels))
                with qs.Scenario([qs.Shock("ESTR", 0.01, qs.ScenarioType.Absolute)]):
                    actual = position.evaluate(REQUESTS)
                    expected = view.evaluate(position, REQUESTS)
                    self.assertEqual(actual.price, expected.price)
                    pd.testing.assert_series_equal(exposures(actual), exposures(expected))
            pd.testing.assert_series_equal(exposures(position.evaluate(REQUESTS)), exposures(native))

    def test_native_and_transformed_views_preserve_value_and_original_labels(self):
        position = trade()
        with context() as original:
            native = original.evaluate(position, REQUESTS)
            projected = original.reproject_risk(OIS)
            zeros = original.with_zero_rate_risk([qs.ZeroRateRisk("ESTR")])
            self.assertIsInstance(projected, qs.RiskContext)
            self.assertIsInstance(projected, qs.PricingContext)
            self.assertEqual(projected.source_labels, DEPOSITS)
            self.assertEqual(projected.target_labels, OIS)
            self.assertTrue(all(0.0 < value < 0.1 for value in projected.target_values))
            for view in [projected, zeros]:
                for _ in range(2):
                    result = view.evaluate(position, REQUESTS)
                    self.assertEqual(result.price, native.price)
                    self.assertEqual(set(exposures(result).index), set(view.target_labels))
                    self.assertGreater(exposures(result).abs().sum(), 0)
                    self.assertEqual(view.curve("ESTR").discount_factor("2027-07-15"),
                                     original.curve("ESTR").discount_factor("2027-07-15"))
            self.assertTrue(all(label.startswith("ZeroRateRisk_ESTR_") for label in zeros.target_labels))
            pd.testing.assert_series_equal(exposures(native), exposures(original.evaluate(position, REQUESTS)))
            labels = projected.target_labels
            labels.clear()
            self.assertEqual(projected.target_labels, OIS)

        # Views own their inputs and survive the original context's tape/cache cleanup.
        self.assertEqual(projected.evaluate(position, REQUESTS).price, native.price)
        with projected:
            self.assertEqual(projected.evaluate(position, REQUESTS).price, native.price)
        with projected:
            self.assertEqual(projected.evaluate(position, REQUESTS).price, native.price)

    def test_zero_grid_dates_conventions_and_independent_bumps(self):
        dates = [datetime.date(2027, 1, 15), qs.Date(2028, 1, 15)]
        grid = qs.ZeroRateRisk(qs.MarketIndex.ESTR, dates, qs.DayCounter.Actual360,
                              qs.Compounding.Compounded, qs.Frequency.Semiannual)
        self.assertEqual(grid.market_index, qs.MarketIndex.ESTR)
        self.assertEqual([str(date) for date in grid.dates], ["2027-01-15", "2028-01-15"])
        self.assertEqual(grid.day_counter, qs.DayCounter.Actual360)
        self.assertEqual(grid.compounding, qs.Compounding.Compounded)
        self.assertEqual(grid.frequency, qs.Frequency.Semiannual)
        self.assertIsNone(qs.ZeroRateRisk("ESTR").dates)
        with context() as original:
            view = original.with_zero_rate_risk([grid])
            for date, rate in zip(dates, view.target_values):
                time = (datetime.date.fromisoformat(str(date)) - datetime.date(2026, 1, 15)).days / 360
                self.assertAlmostEqual(original.curve("ESTR").discount_factor(date),
                                       (1 + rate / 2) ** (-2 * time), places=12)

            continuous = original.with_zero_rate_risk([qs.ZeroRateRisk("ESTR")])
            actual = exposures(continuous.evaluate(trade(), REQUESTS))
            factors = original.curve("ESTR").nodes()
            # Bump each zero, preserving the original log-linear DF interpolation.
            for column, label in enumerate(continuous.target_labels):
                bump = 1e-6
                prices = []
                for shift in [bump, -bump]:
                    records = []
                    labels = []
                    for row in range(1, len(factors)):
                        date = factors.iloc[row]["date"].date()
                        time = (date - datetime.date(2026, 1, 15)).days / 365
                        factor = float(factors.iloc[row]["discount_factor"])
                        if row == column + 1:
                            factor *= math.exp(-shift * time)
                        identifier = f"DiscountFactor_EUR_ESTR_{date}"
                        labels.append(identifier)
                        records.append({"identifier": identifier, "mid": factor})
                    quotes = qs.QuoteStore.from_dict({"reference_date": "2026-01-15", "quotes": records})
                    with qs.PricingContext(quotes, [qs.CurveConfiguration("ESTR", labels, "Actual365", "LogLinear", True)],
                                           csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR")) as bumped:
                        prices.append(bumped.evaluate(trade()).price)
                finite_difference = (prices[0] - prices[1]) / (2 * bump)
                self.assertAlmostEqual(actual[label], finite_difference, delta=0.001)

    def test_identity_basis_and_selected_quote_sides(self):
        for level in [qs.Level.Mid, "Bid", "Ask"]:
            with self.subTest(level=level), context(level) as original:
                view = original.reproject_risk(DEPOSITS)
                pd.testing.assert_series_equal(exposures(original.evaluate(trade(), REQUESTS)),
                                               exposures(view.evaluate(trade(), REQUESTS)),
                                               rtol=1e-5, atol=0.01)

    def test_portfolio_and_registered_python_pricers_use_transformed_market(self):
        class CustomTrade:
            pass

        seen = []

        class CustomPricer:
            def evaluate(self, position, requests, market):
                seen.append((isinstance(market, qs.RiskContext),
                             list(market.curve("ESTR").pillars()["label"])))
                return qs.EvaluationResults(market.evaluation_date, "CUSTOM", price=7.0)

        with context() as original:
            original.register_pricer(CustomTrade, CustomPricer())
            view = original.reproject_risk(OIS)
            individual = view.evaluate(trade(), REQUESTS)
            portfolio = view.evaluate(qs.Portfolio("TWO", [trade(), trade()]), REQUESTS)
            self.assertAlmostEqual(portfolio.price, 2 * individual.price)
            pd.testing.assert_series_equal(exposures(portfolio), 2 * exposures(individual))
            self.assertEqual(view.evaluate(CustomTrade()).price, 7.0)
            self.assertEqual(seen, [(True, OIS)])

    def test_scenarios_recompute_basis_and_restore_original_and_derived_contexts(self):
        scenario = qs.Scenario([qs.Shock("ESTR", 0.001, qs.ScenarioType.Absolute)])
        position = trade()
        with context() as original:
            base_value = original.evaluate(position).price
            view = original.reproject_risk(OIS)
            base_targets = view.target_values
            with view:
                with scenario:
                    shocked = view.evaluate(position, REQUESTS)
                    self.assertNotEqual(view.target_values, base_targets)
                    self.assertEqual(original.evaluate(position).price, base_value)
                    with context(bump=0.001) as expected:
                        self.assertEqual(shocked.price, expected.evaluate(position).price)
                        expected_view = expected.reproject_risk(OIS)
                        pd.testing.assert_series_equal(exposures(shocked), exposures(expected_view.evaluate(position, REQUESTS)))
                self.assertEqual(view.target_values, base_targets)
                self.assertEqual(view.evaluate(position).price, base_value)
            with scenario:
                snapshot = original.with_zero_rate_risk([qs.ZeroRateRisk("ESTR")])
                shocked_price = original.evaluate(position).price
            self.assertEqual(snapshot.evaluate(position).price, shocked_price)
            self.assertEqual(original.evaluate(position).price, base_value)

    def test_input_mutation_and_invalid_bases_cannot_leave_stale_risk(self):
        uninitialized = context()
        with self.assertRaises(qs.QuantSupportStateError):
            uninitialized.reproject_risk(OIS)
        with context() as original:
            for identifiers in [[], OIS[:1], [OIS[0], OIS[0]]]:
                with self.subTest(identifiers=identifiers), self.assertRaises(qs.QuantSupportError):
                    original.reproject_risk(identifiers)
            for grids in [[], [qs.ZeroRateRisk("ESTR", [])], [qs.ZeroRateRisk("ESTR", ["2026-01-15"])]]:
                with self.assertRaises(qs.QuantSupportError):
                    original.with_zero_rate_risk(grids)
            view = original.reproject_risk(OIS)
            for attribute, value in [("quotes", original.quotes), ("anchor_fixings", False),
                                     ("quote_level", "Bid"), ("curves", original.curves)]:
                with self.subTest(attribute=attribute), self.assertRaises(qs.QuantSupportStateError):
                    setattr(view, attribute, value)
            value = view.evaluate(trade()).price
            original.quote_level = "Bid"
            self.assertEqual(view.evaluate(trade()).price, value)
            self.assertNotEqual(original.evaluate(trade()).price, value)
        with self.assertRaises(TypeError):
            qs.RiskContext()


if __name__ == "__main__":
    unittest.main()

"""Horizon contexts rebuild from generated inputs and preserve scenario scope."""

import unittest

import quantsupport as qs


def source_context():
    identifiers = [f"ZeroRate_USD_SOFR_{tenor}" for tenor in ["0D", "1Y", "3Y", "10Y"]]
    quotes = qs.QuoteStore.from_dict({
        "reference_date": "2026-01-05",
        "quotes": [dict(identifier=identifier, mid=value) for identifier, value in
                   zip(identifiers, [0.025, 0.03, 0.04, 0.05])],
    })
    return qs.PricingContext(quotes, [qs.CurveConfiguration(
        "SOFR", identifiers, day_counter="Actual365", interpolator="Linear",
        enable_extrapolation=True,
    )])


class ForwardMarketTests(unittest.TestCase):
    def test_configuration_round_trip_and_horizon_curve(self):
        configuration = qs.ForwardMarketConfiguration("2026-07-06")
        restored = qs.ForwardMarketConfiguration.from_dict(configuration.to_dict())
        source = source_context()
        future = source.advance_to(restored)
        self.assertIsInstance(future, qs.PricingContext)
        self.assertEqual(str(future.reference_date), "2026-07-06")
        with source:
            expected = (source.curve("SOFR").discount_factor("2029-07-06") /
                        source.curve("SOFR").discount_factor("2026-07-06"))
        with future:
            self.assertAlmostEqual(future.curve("SOFR").discount_factor("2029-07-06"), expected)
        self.assertEqual(future.market_advancement_details["source_date"], "2026-01-05")
        self.assertEqual(str(source.reference_date), "2026-01-05")

    def test_active_source_scenarios_and_horizon_scenarios(self):
        configuration = qs.ForwardMarketConfiguration("2026-07-06")
        source = source_context()
        scenario = qs.Scenario([qs.Shock("SOFR", 0.01, qs.ScenarioType.Absolute)])
        baseline = source.advance_to(configuration)
        with source, scenario:
            advanced = source.advance_to(configuration)
        identifier = "ZeroRate_USD_SOFR_3Y"
        self.assertGreater(advanced.quotes.value(identifier), baseline.quotes.value(identifier))
        with baseline:
            before = baseline.curve("SOFR").discount_factor("2029-07-06")
            with scenario:
                self.assertLess(baseline.curve("SOFR").discount_factor("2029-07-06"), before)
        self.assertEqual(source.quotes.value(identifier), 0.04)

    def test_projected_fixings_and_backward_dates(self):
        source = source_context()
        future = source.advance_to(qs.ForwardMarketConfiguration("2026-01-13"))
        self.assertTrue(0.02 < future.fixings.fixing("SOFR", "2026-01-06") < 0.05)
        restored = qs.FixingStore.from_dict(future.fixings.to_dict())
        self.assertEqual(restored.fixing("SOFR", "2026-01-06"), future.fixings.fixing("SOFR", "2026-01-06"))
        with self.assertRaises(qs.QuantSupportError):
            source.advance_to(qs.ForwardMarketConfiguration("2026-01-04"))
        with self.assertRaises(ValueError):
            qs.ForwardMarketConfiguration("2026-07-06", curve_roll="unknown")


if __name__ == "__main__":
    unittest.main()

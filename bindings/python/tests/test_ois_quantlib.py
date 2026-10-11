"""Optional numerical parity checks against QuantLib's OIS helpers."""

import unittest

import quantsupport as qs

try:
    import QuantLib as ql
except ImportError:
    ql = None


@unittest.skipIf(ql is None, "QuantLib is an optional validation dependency")
class OisQuantLibTests(unittest.TestCase):
    def test_sofr_discount_factors_match_quantlib(self):
        reference = ql.Date(8, 10, 2026)
        previous_date = ql.Settings.instance().evaluationDate
        ql.Settings.instance().evaluationDate = reference
        try:
            calendar = ql.UnitedStates(ql.UnitedStates.SOFR)
            quotes = [("1M", 0.041), ("3M", 0.0405), ("1Y", 0.039),
                      ("2Y", 0.037), ("5Y", 0.035)]
            records = [{"identifier": "FixedRateDeposit_USD_SOFR_1D", "mid": 0.042}]
            helpers = [ql.DepositRateHelper(0.042, ql.Period(1, ql.Days), 0,
                                           calendar, ql.Following, False, ql.Actual360())]
            for tenor, rate in quotes:
                records.append({
                    "identifier": (f"OIS_USD_SOFR_{tenor}_Annual_Annual_2_2_Actual360"
                                   "_UnitedStates(Sofr)_UnitedStates(Sofr)"
                                   "_UnitedStates(Sofr)_UnitedStates(Sofr)"
                                   "_ModifiedFollowing_ModifiedFollowing_Following_Backward_false"),
                    "mid": rate,
                })
                helpers.append(ql.OISRateHelper(
                    2, ql.Period(tenor), rate, ql.Sofr(), paymentLag=2,
                    paymentConvention=ql.Following, paymentFrequency=ql.Annual,
                    paymentCalendar=calendar, endOfMonth=False,
                ))
            expected = ql.PiecewiseLogLinearDiscount(reference, helpers, ql.Actual365Fixed())
            expected.enableExtrapolation()
            store = qs.QuoteStore.from_dict({"reference_date": "2026-10-08", "quotes": records})
            config = qs.CurveConfiguration.from_dict({
                "market_index": "SOFR", "day_counter": "Actual365",
                "interpolator": "LogLinear", "quotes": [row["identifier"] for row in records],
            })
            with qs.PricingContext(quotes=store, curves=[config]) as context:
                actual = context.curve(qs.MarketIndex.SOFR)
                for helper in helpers:
                    date = helper.pillarDate()
                    value = actual.discount_factor(date.ISO())
                    with self.subTest(pillar=date.ISO()):
                        self.assertAlmostEqual(value, expected.discount(date), delta=2e-10)
                for tenor, rate in quotes:
                    date = reference + ql.Period(tenor)
                    with self.subTest(interpolated=date.ISO()):
                        self.assertAlmostEqual(actual.discount_factor(date.ISO()), expected.discount(date), delta=2e-10)
                for helper, (_, rate) in zip(helpers[1:], quotes):
                    self.assertAlmostEqual(helper.impliedQuote(), rate, delta=2e-10)
        finally:
            ql.Settings.instance().evaluationDate = previous_date


if __name__ == "__main__":
    unittest.main()
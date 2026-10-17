import datetime
import math
import unittest

import quantsupport as qs


class ZeroCurveTests(unittest.TestCase):
    def setUp(self):
        self.reference = qs.Date(2026, 1, 1)
        self.dates = [self.reference, self.reference +
                      "1Y", self.reference + "2Y"]
        self.rates = [0.01, 0.02, 0.03]

    def test_dated_and_tenor_curves_match(self):
        dated = qs.ZeroRateTermStructure(
            self.reference, self.dates, self.rates)
        tenor = qs.TenorBasedZeroRateTermStructure(
            self.reference, [qs.Period.parse("0D"), "1Y", "2Y"], self.rates
        )
        for curve in [dated, tenor]:
            self.assertEqual(curve.reference_date, self.reference)
            self.assertEqual(curve.dates, self.dates)
            self.assertEqual(curve.rates, self.rates)
            self.assertEqual(curve.discount_factor(self.reference), 1.0)
            self.assertAlmostEqual(curve.zero_rate("2027-01-01"), 0.02)
            self.assertAlmostEqual(
                curve.discount_factor(datetime.date(
                    2027, 1, 1)), math.exp(-0.02)
            )
            self.assertAlmostEqual(curve.forward_rate(
                self.dates[1], self.dates[2]), 0.04)
        self.assertEqual([str(tenor)
                         for tenor in tenor.tenors], ["0D", "1Y", "2Y"])

    def test_conventions_and_log_linear_interpolation(self):
        curve = qs.ZeroRateTermStructure(
            self.reference, [self.reference, self.dates[2]], [0.002, 0.008],
            day_counter=qs.DayCounter.Actual365,
            compounding=qs.Compounding.Simple,
            frequency=qs.Frequency.Annual,
            interpolator=qs.Interpolator.LogLinear,
        )
        self.assertAlmostEqual(curve.zero_rate(self.dates[1]), 0.004)
        self.assertAlmostEqual(
            curve.discount_factor(self.dates[1]), 1.0 / 1.004)
        self.assertEqual(curve.compounding, qs.Compounding.Simple)
        self.assertEqual(curve.interpolator, qs.Interpolator.LogLinear)

    def test_curve_round_trip_and_csa_accept_both_curve_classes(self):
        curves = [
            qs.ZeroRateTermStructure(self.reference, self.dates, self.rates),
            qs.TenorBasedZeroRateTermStructure(
                self.reference, ["0D", "1Y", "2Y"], self.rates),
        ]
        for curve in curves:
            restored = qs.ZeroRateTermStructure.from_dict(curve.to_dict())
            self.assertEqual(restored.rates, self.rates)
            specification = dict(
                collateral_index=qs.MarketIndex.SOFR,
                collateral_currency=qs.Currency.USD,
                credit_spread=0.01, recovery=0.4, funding_spread_curve=curve,
                variation_margin=True, counterparty_threshold=1000.0,
                own_threshold=1000.0, minimum_transfer_amount=500.0,
            )
            for terms in [qs.CsaTerms(**specification), qs.CsaTerms.from_dict(specification)]:
                self.assertIsInstance(
                    terms.funding_spread_curve, qs.ZeroRateTermStructure)
                self.assertEqual(terms.funding_spread_curve.rates, self.rates)
                self.assertTrue(terms.variation_margin)
                self.assertAlmostEqual(
                    terms.funding_spread_curve.zero_rate(self.dates[1]), 0.02)

    def test_invalid_data_and_disabled_extrapolation(self):
        for dates, rates in [([], []), (self.dates, [0.01]),
                             ([self.reference, self.reference], [0.01, 0.02]),
                             ([self.reference], [float("nan")])]:
            with self.assertRaises(qs.QuantSupportError):
                qs.ZeroRateTermStructure(self.reference, dates, rates)
        with self.assertRaises(qs.QuantSupportError):
            qs.TenorBasedZeroRateTermStructure(self.reference, ["-1Y"], [0.01])
        curve = qs.ZeroRateTermStructure(
            self.reference, self.dates, self.rates)
        with self.assertRaises(qs.QuantSupportError):
            curve.zero_rate(self.reference + "3Y")
        with self.assertRaises(qs.QuantSupportError):
            curve.discount_factor_from_time(-1.0)

    def test_single_pillar_extrapolation(self):
        curve = qs.TenorBasedZeroRateTermStructure(
            self.reference, ["0D"], [0.005], enable_extrapolation=True
        )
        self.assertEqual(curve.zero_rate(self.reference + "5Y"), 0.005)


if __name__ == "__main__":
    unittest.main()

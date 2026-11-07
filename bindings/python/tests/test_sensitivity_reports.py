"""Sensitivity reports order pillars by maturity without changing risk coordinates."""

import math
import unittest

import quantsupport as qs


REFERENCE = qs.Date(2026, 1, 15)
TENORS = ["6M", "1Y", "2Y", "10Y"]
REQUESTS = [qs.Request.Value, qs.Request.Sensitivities]


def discount_context(shifted_tenor=None, zero_bump=0.0):
    records = []
    for tenor in TENORS:
        time = qs.DayCounter.Actual365.year_fraction(REFERENCE, REFERENCE + tenor)
        zero = 0.03 + (zero_bump if tenor == shifted_tenor else 0.0)
        records.append({"identifier": f"DiscountFactor_EUR_ESTR_{tenor}",
                        "mid": math.exp(-zero * time)})
    quotes = qs.QuoteStore.from_dict({"reference_date": str(REFERENCE), "quotes": records})
    curve = qs.CurveConfiguration("ESTR", quotes.identifiers(), "Actual365", "LogLinear", True)
    return qs.PricingContext(quotes, [curve],
                             csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR"))


def deposit():
    instrument = qs.FixedRateDeposit(
        "DEPOSIT-18M", REFERENCE, REFERENCE + "18M", 1_000_000, 0.035, "EUR", "ESTR",
    )
    return qs.Trade(instrument, REFERENCE)


class SensitivityReportTests(unittest.TestCase):
    def test_tenor_order_preserves_raw_pairs_and_sums_duplicate_labels(self):
        pairs = [("OIS_EUR_ESTR_10Y", 10.0), ("OIS_EUR_ESTR_2Y", 2.0),
                 ("FixedRateDeposit_EUR_ESTR_6M", 0.5), ("OIS_EUR_ESTR_1Y", 1.0),
                 ("OIS_EUR_ESTR_2Y", 3.0)]
        result = qs.EvaluationResults(REFERENCE, "REPORT", sensitivities=pairs)
        report = result.sensitivities
        self.assertEqual(report.pillar.tolist(), [
            "FixedRateDeposit_EUR_ESTR_6M", "OIS_EUR_ESTR_1Y",
            "OIS_EUR_ESTR_2Y", "OIS_EUR_ESTR_10Y",
        ])
        self.assertEqual(report.value.tolist(), [0.5, 1.0, 5.0, 10.0])
        self.assertEqual(result.raw_sensitivities, pairs)

    def test_trade_and_portfolio_reports_sort_discount_factor_tenors(self):
        position = deposit()
        portfolio = qs.Portfolio("DEPOSITS", [position, position])
        expected = [f"DiscountFactor_EUR_ESTR_{tenor}" for tenor in TENORS]
        with discount_context() as context:
            for instrument in [position, portfolio]:
                result = context.evaluate(instrument, REQUESTS)
                self.assertEqual(result.sensitivities.pillar.tolist(), expected)
                self.assertEqual(result.sensitivities.set_index("pillar").value.to_dict(),
                                 dict(result.raw_sensitivities))

    def test_discount_curve_zero_rate_report_preserves_pv_and_matches_bumps(self):
        position = deposit()
        portfolio = qs.Portfolio("DEPOSITS", [position, position])
        # The reporting grid can have any order; target metadata retains that
        # order while the sensitivity table is presented chronologically.
        grid_tenors = ["10Y", "2Y", "6M", "1Y"]
        with discount_context() as context:
            original = context.evaluate(portfolio, REQUESTS)
            view = context.with_zero_rate_risk([qs.ZeroRateRisk(
                "ESTR", dates=[REFERENCE + tenor for tenor in grid_tenors],
                day_counter="Actual365", compounding="Continuous", frequency="Annual",
            )])
            with view:
                result = view.evaluate(portfolio, REQUESTS)
                target_labels = view.target_labels
                target_values = view.target_values
            self.assertEqual(result.price, original.price)
            self.assertEqual(context.evaluate(portfolio).price, original.price)
        labels = {
            tenor: f"ZeroRateRisk_ESTR_{REFERENCE + tenor}_Actual365_Continuous_Annual"
            for tenor in TENORS
        }
        self.assertEqual(target_labels, [labels[tenor] for tenor in grid_tenors])
        self.assertEqual(result.sensitivities.pillar.tolist(), [labels[tenor] for tenor in TENORS])
        self.assertTrue(all(abs(rate - 0.03) < 1e-12 for rate in target_values))
        risk = result.sensitivities.set_index("pillar").value.to_dict()
        for tenor in TENORS:
            prices = []
            for bump in [1e-6, -1e-6]:
                with discount_context(tenor, bump) as context:
                    prices.append(context.evaluate(portfolio).price)
            finite_difference = (prices[0] - prices[1]) / 2e-6
            self.assertAlmostEqual(risk[labels[tenor]], finite_difference, delta=0.01)


if __name__ == "__main__":
    unittest.main()

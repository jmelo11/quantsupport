"""Date-window coupon accrual uses native requests, conventions and projection state."""
from datetime import date
import math
import unittest

import quantsupport as qs

REFERENCE = "2026-03-01"


def market(*, forward_basis="Actual360", base="EUR"):
    inputs = [("ESTR", 0.025, "EUR"), ("EURIBOR3m", 0.03, "EUR"), ("SOFR", 0.035, "USD")]
    tenors = ["0D", "1Y", "5Y"]
    quotes = qs.QuoteStore.from_dict({"reference_date": REFERENCE, "quotes": [
        {"identifier": f"ZeroRate_{currency}_{index}_{tenor}", "mid": rate}
        for index, rate, currency in inputs for tenor in tenors
    ]})
    curves = [qs.CurveConfiguration(
        market_index=index, quotes=[f"ZeroRate_{currency}_{index}_{tenor}" for tenor in tenors],
        day_counter=forward_basis if index == "EURIBOR3m" else "Actual360",
        interpolator="Linear",
    ) for index, _, currency in inputs]
    fx = qs.FxStore()
    fx.add("EUR", "USD", 1.1)
    context = qs.PricingContext(
        quotes, curves, fx=fx,
        csa_policy=qs.SingleCurveCSADiscountPolicy(base, "ESTR" if base == "EUR" else "SOFR"),
        fixed_income_policy=qs.FixedIncomeDiscountPolicy({"EUR": "ESTR", "USD": "SOFR"}),
        pricer_configurations=[qs.PricerConfiguration.loan()],
    )
    return context


def deposit(side="LongReceive", override=None):
    instrument = qs.FixedRateDeposit(
        "SEASONED", "2025-12-01", "2026-06-01", 1000, 0.06, "EUR", "ESTR",
        day_counter="Actual365", compounding="Simple",
    )
    return qs.Trade(instrument, REFERENCE, side, notional=override)


class AccruedAmountTests(unittest.TestCase):
    def test_request_dates_parsing_hashing_and_invalid_ranges(self):
        request = qs.Request.AccruedAmount(date(2026, 3, 1), qs.Date.parse("2026-04-01"))
        self.assertEqual(request.start_date, qs.Date.parse(REFERENCE))
        self.assertEqual(request.end_date, qs.Date.parse("2026-04-01"))
        self.assertIsNone(request.index)
        self.assertIsNone(qs.Request.Value.start_date)
        self.assertEqual(qs.Request.parse(str(request)), request)
        self.assertEqual(qs.Request.parse("accrued_amount('2026-03-01', '2026-04-01')"), request)
        self.assertEqual(len({request, qs.Request.parse(str(request))}), 1)
        self.assertIn("AccruedAmount", repr(request))
        with self.assertRaises(qs.QuantSupportError):
            qs.Request.AccruedAmount("2026-04-01", "2026-03-01")
        with self.assertRaises(qs.QuantSupportError):
            qs.Request.parse("AccruedAmount(2026-02-30,2026-04-01)")

    def test_seasoned_deposit_does_not_require_discounting_its_historical_advance(self):
        request = qs.Request.AccruedAmount(REFERENCE, "2026-04-01")
        with market() as context:
            for side, direction in (("LongReceive", 1), ("PayShort", -1)):
                for notional in (None, 250, 0):
                    with self.subTest(side=side, notional=notional):
                        result = context.evaluate(deposit(side, notional), [request])
                        amount = 1000 if notional is None else notional
                        self.assertAlmostEqual(result.accrued_amount, direction * amount * 0.06 * 31 / 365)
                        self.assertIsNone(result.price)
                        self.assertIsNone(result.cashflows)

    def test_cashflows_and_accrual_do_not_compute_a_seasoned_deposit_pv(self):
        with market() as context:
            result = context.evaluate(deposit(), [qs.Request.Cashflows,
                qs.Request.AccruedAmount(REFERENCE, "2026-04-01")])
        self.assertIsNone(result.price)
        self.assertEqual(set(result.cashflows.type), {"Disbursement", "FixedRateCoupon", "Redemption"})
        self.assertAlmostEqual(result.accrued_amount, 1000 * 0.06 * 31 / 365)

    def test_clips_to_coupon_period_and_empty_windows_return_zero(self):
        with market() as context:
            for start, end, days in [(REFERENCE, REFERENCE, 0),
                                     ("2026-06-01", "2026-09-01", 0),
                                     ("2026-05-15", "2026-08-01", 17)]:
                result = context.evaluate(deposit(), [qs.Request.AccruedAmount(start, end)])
                self.assertAlmostEqual(result.accrued_amount, 1000 * 0.06 * days / 365)

    def test_coupon_level_method_clips_and_reconciles_thirty_360_partitions(self):
        coupon = qs.FixedRateCoupon(
            rate=qs.InterestRate(0.06, day_counter="Thirty360"), notional=1000,
            accrual_start_date="2026-01-31", accrual_end_date="2026-03-31", payment_date="2026-04-05",
        )
        dates = ["2026-01-31", "2026-02-01", "2026-03-01", "2026-03-31"]
        parts = [coupon.accrued_amount(a, b) for a, b in zip(dates, dates[1:])]
        self.assertAlmostEqual(sum(parts), 10)
        self.assertAlmostEqual(sum(parts), coupon.amount())
        self.assertEqual(coupon.accrued_amount("2025-01-01", "2026-01-31"), 0)
        with self.assertRaises(qs.QuantSupportError):
            coupon.accrued_amount("2026-03-01", "2026-02-01")

    def test_long_actual_actual_coupon_and_reverse_day_count_include_all_years(self):
        coupon = qs.FixedRateCoupon(
            rate=qs.InterestRate(0.06, day_counter="ActualActual"), notional=1000,
            accrual_start_date="2023-01-01", accrual_end_date="2026-01-01", payment_date="2026-01-01",
        )
        self.assertAlmostEqual(coupon.amount(), 180)
        self.assertAlmostEqual(coupon.accrued_amount("2024-01-01", "2025-01-01"), 60)
        self.assertAlmostEqual(qs.DayCounter.ActualActual.year_fraction("2026-01-01", "2023-01-01"), -3)

    def test_unresolved_coupon_requires_fixing_only_inside_its_accrual_period(self):
        coupon = qs.FloatingRateCoupon(
            index="EURIBOR3m", spread=0.01, notional=1000,
            accrual_start_date=REFERENCE, accrual_end_date="2026-06-01", payment_date="2026-06-01",
        )
        self.assertEqual(coupon.accrued_amount("2026-06-01", "2026-07-01"), 0)
        with self.assertRaises(qs.QuantSupportError):
            coupon.accrued_amount(REFERENCE, "2026-04-01")

    def test_floating_request_observes_index_day_count_not_curve_day_count(self):
        note = qs.FloatingRateNote("NOTE", REFERENCE, "2026-06-01", 1000, 0.01, "EUR", "EURIBOR3m",
                                   payment_frequency="Quarterly")
        with market(forward_basis="Actual365") as context:
            result = context.evaluate(qs.Trade(note, REFERENCE),
                                      [qs.Request.AccruedAmount(REFERENCE, "2026-04-01")])
        fixing = math.expm1(0.03 * 92 / 365) / (92 / 360)
        self.assertAlmostEqual(result.accrued_amount, 1000 * (fixing + 0.01) * 31 / 360, places=10)

    def test_loan_uses_prepayment_adjusted_notional_once_and_excludes_principal(self):
        instrument = qs.Loan(
            identifier="LOAN", start_date=REFERENCE, maturity_date="2027-03-01",
            notional=1000, currency="EUR", repayment="Bullet", payment_frequency="Monthly",
            coupons=[qs.FixedRateCoupon(rate=qs.InterestRate(0.06, day_counter="Actual365"))],
            prepayment=qs.PrepaymentModelConfiguration.constant_annual_cpr(0.12),
        )
        trade = qs.Trade(instrument, REFERENCE, "PayShort", notional=500)
        with market() as context:
            result = context.evaluate(trade, [qs.Request.AccruedAmount("2026-04-01", "2026-05-01"),
                                               qs.Request.Cashflows])
            with self.assertRaises(qs.QuantSupportError):
                context.evaluate(trade, [qs.Request.AccruedAmount("2026-02-01", "2026-04-01")])
        self.assertAlmostEqual(result.accrued_amount, -500 * 0.88 ** (1 / 12) * 0.06 * 30 / 365)
        march = result.coupons.query("type == 'FixedRateCoupon' and payment_date == '2026-05-01'")
        self.assertAlmostEqual(result.accrued_amount, march.amount.sum())

    def test_portfolio_aggregation_and_spot_currency_conversion(self):
        book = qs.Portfolio("BOOK", [deposit(), deposit("PayShort", 400)])
        request = qs.Request.AccruedAmount(REFERENCE, "2026-04-01")
        with market(base="USD") as context:
            result = context.evaluate(book, [request])
        self.assertAlmostEqual(result.accrued_amount, 600 * 0.06 * 31 / 365 * 1.1)
        self.assertIsNone(result.price)

    def test_one_accrual_window_per_evaluation_and_python_results_roundtrip(self):
        with market() as context:
            with self.assertRaises(qs.QuantSupportError):
                context.evaluate(deposit(), [qs.Request.AccruedAmount(REFERENCE, "2026-04-01"),
                                              qs.Request.AccruedAmount("2026-04-01", "2026-05-01")])
        result = qs.EvaluationResults(REFERENCE, "PYTHON", accrued_amount=-12.5)
        self.assertEqual(result.accrued_amount, -12.5)
        self.assertIsNone(qs.EvaluationResults(REFERENCE, "EMPTY").accrued_amount)


if __name__ == "__main__":
    unittest.main()

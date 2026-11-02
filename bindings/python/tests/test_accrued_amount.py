"""Python conversion contracts for accrued amounts; numerical cases live in Rust."""
from datetime import date
import unittest

import quantsupport as qs

REFERENCE = "2026-03-01"


def market():
    quotes = qs.QuoteStore.from_dict({"reference_date": REFERENCE, "quotes": [
        {"identifier": "ZeroRate_EUR_ESTR_1Y", "mid": 0.025},
    ]})
    return qs.PricingContext(
        quotes, [qs.CurveConfiguration(market_index="ESTR", quotes=["ZeroRate_EUR_ESTR_1Y"])],
        csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR"),
    )


def deposit():
    instrument = qs.FixedRateDeposit(
        "SEASONED", "2025-12-01", "2026-06-01", 1000, 0.06, "EUR", "ESTR",
        day_counter="Actual365", compounding="Simple",
    )
    return qs.Trade(instrument, REFERENCE)


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

    def test_native_accrual_results_and_cashflow_dataframe(self):
        request = qs.Request.AccruedAmount(date(2026, 3, 1), "2026-04-01")
        with market() as context:
            standalone = context.evaluate(deposit(), [request])
            combined = context.evaluate(deposit(), [qs.Request.Cashflows, request])
            for side, notional, scale in [(qs.Side.PayShort, 250, -0.25), (qs.Side.LongReceive, 0, 0)]:
                with self.subTest(side=side, notional=notional):
                    position = qs.Trade(deposit().instrument, REFERENCE, side, notional=notional)
                    scaled = context.evaluate(position, [request])
                    self.assertAlmostEqual(scaled.accrued_amount, standalone.accrued_amount * scale)
        self.assertIsInstance(standalone.accrued_amount, float)
        self.assertAlmostEqual(standalone.accrued_amount, 1000 * 0.06 * 31 / 365)
        self.assertIsNone(standalone.price)
        self.assertIsNone(standalone.cashflows)
        self.assertEqual(combined.accrued_amount, standalone.accrued_amount)
        self.assertIsNone(combined.price)
        self.assertEqual(set(combined.cashflows.type), {"Disbursement", "FixedRateCoupon", "Redemption"})

    def test_coupon_accrual_accepts_python_native_and_string_dates(self):
        dates = dict(notional=1000, accrual_start_date=REFERENCE,
                     accrual_end_date="2026-06-01", payment_date="2026-06-01")
        coupons = [
            qs.FixedRateCoupon(rate=qs.InterestRate(0.06), **dates),
            qs.FloatingRateCoupon(index="EURIBOR3m", fixing=0.04, **dates),
            qs.OptionEmbeddedCoupon(index="EURIBOR3m", payoff=qs.PayoffOps.Index,
                                    fixing=0.04, **dates),
        ]
        for coupon in coupons:
            with self.subTest(coupon=type(coupon).__name__):
                amount = coupon.accrued_amount(date(2026, 3, 1), qs.Date.parse("2026-04-01"))
                self.assertIsInstance(amount, float)
                self.assertEqual(amount, coupon.accrued_amount(start=REFERENCE, end="2026-04-01"))
                with self.assertRaises(qs.QuantSupportTypeError):
                    coupon.accrued_amount(object(), "2026-04-01")

    def test_native_request_validation_errors_are_exposed(self):
        with market() as context:
            with self.assertRaises(qs.QuantSupportError):
                context.evaluate(deposit(), [qs.Request.AccruedAmount(REFERENCE, "2026-04-01"),
                                              qs.Request.AccruedAmount("2026-04-01", "2026-05-01")])

    def test_accrual_requests_match_with_or_without_cashflows(self):
        windows = [(date(2026, 3, 1), qs.Date.parse("2026-04-01")),
                   ("2026-04-01", "2026-05-01"), ("2026-05-01", "2026-06-01"),
                   ("2026-06-01", "2026-07-01"), (REFERENCE, REFERENCE)]
        with market() as context:
            for rate in (0, -0.02, 0.06):
                for side in (qs.Side.LongReceive, qs.Side.PayShort):
                    instrument = qs.FixedRateDeposit(
                        "DEPOSIT", "2025-12-01", "2026-06-01", 1000, rate, "EUR", "ESTR",
                        day_counter="Thirty360", compounding="Simple")
                    trade = qs.Trade(instrument, REFERENCE, side, notional=250)
                    for start, end in windows:
                        with self.subTest(rate=rate, side=side, start=start, end=end):
                            request = qs.Request.AccruedAmount(start, end)
                            standalone = context.evaluate(trade, [request])
                            self.assertIsNone(standalone.cashflows)
                            for requests in ([request, qs.Request.Cashflows],
                                             [qs.Request.Cashflows, request]):
                                result = context.evaluate(trade, requests)
                                self.assertAlmostEqual(result.accrued_amount, standalone.accrued_amount)
                                self.assertIsNotNone(result.cashflows)
                                self.assertIsNone(result.price)

    def test_evaluation_results_expose_accrual_as_completed_data(self):
        result = qs.EvaluationResults(REFERENCE, "RESULT", accrued_amount=12.5)
        self.assertEqual(result.accrued_amount, 12.5)
        self.assertFalse(hasattr(result, "cashflow_accruals"))
        self.assertFalse(hasattr(result, "remaining_redemptions"))

    def test_python_pricer_roundtrips_requests_and_optional_accrued_amount(self):
        class AccrualPricer:
            def evaluate(self, trade, requests, context):
                self.received = requests
                return qs.EvaluationResults(context.evaluation_date, trade, accrued_amount=self.amount)

        context = qs.PricingContext(qs.QuoteStore.from_dict({"reference_date": REFERENCE, "quotes": []}), [])
        pricer = AccrualPricer()
        context.register_pricer(str, pricer)
        request = qs.Request.AccruedAmount(date(2026, 3, 1), "2026-04-01")
        with context:
            for amount in (-12.5, 0.0, None):
                with self.subTest(amount=amount):
                    pricer.amount = amount
                    # A single-member portfolio converts the Python result to Rust and back.
                    result = context.evaluate(qs.Portfolio("PYTHON", ["POSITION"]), [request])
                    self.assertEqual(pricer.received, [request])
                    self.assertEqual(pricer.received[0].start_date, qs.Date.parse(REFERENCE))
                    self.assertEqual(pricer.received[0].end_date, qs.Date.parse("2026-04-01"))
                    self.assertEqual(result.accrued_amount, amount)


if __name__ == "__main__":
    unittest.main()

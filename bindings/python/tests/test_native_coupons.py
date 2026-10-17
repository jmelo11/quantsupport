"""Native coupon amounts, payoff preservation, and market derivatives."""
import inspect
import unittest

import quantsupport as qs
from test_loans import build_loan_contract, build_pricing_context, evaluate_loan_trade


class NativeCouponTests(unittest.TestCase):
    def test_dated_native_coupon_amounts_and_spread_order(self):
        start = qs.Date(year=2026, month=2, day=24)
        end = start + qs.Period(length=1, units=qs.TimeUnit.Months)
        dates = dict(notional=100_000, accrual_start_date=start,
                     accrual_end_date=end, payment_date=end)
        fixed = qs.FixedRateCoupon(
            rate=qs.InterestRate(rate=0.06, day_counter=qs.DayCounter.Actual360), **dates)
        floating = qs.FloatingRateCoupon(
            index=qs.MarketIndex.EURIBOR12m, spread=0.01, fixing=0.04, **dates)
        capped = qs.OptionEmbeddedCoupon(
            index=qs.MarketIndex.EURIBOR12m, fixing=0.04,
            payoff=(qs.PayoffOps.Index + 0.01).max(0.02).min(0.045), **dates)
        margin_after_cap = qs.OptionEmbeddedCoupon(
            index=qs.MarketIndex.EURIBOR12m, fixing=0.04, spread=0.01,
            payoff=qs.PayoffOps.Index.min(0.045), **dates)
        for coupon, annual_rate in [(fixed, 0.06), (floating, 0.05),
                                    (capped, 0.045), (margin_after_cap, 0.05)]:
            with self.subTest(coupon=type(coupon).__name__):
                self.assertAlmostEqual(coupon.amount(), 100_000 * annual_rate * 28 / 360)
                restored = type(coupon).from_dict(coupon.to_dict())
                self.assertAlmostEqual(restored.amount(), coupon.amount())
                self.assertEqual(restored.payment_date, end)

    def test_option_coupon_defaults_to_native_index_day_count(self):
        start = qs.Date(year=2026, month=2, day=24)
        end = start + qs.Period(length=1, units=qs.TimeUnit.Months)
        coupon = qs.OptionEmbeddedCoupon(
            index=qs.MarketIndex.SONIA, payoff=qs.PayoffOps.Index,
            notional=100_000, accrual_start_date=start,
            accrual_end_date=end, payment_date=end, fixing=0.04)
        self.assertEqual(coupon.day_counter, qs.DayCounter.Actual365)
        self.assertAlmostEqual(coupon.amount(), 100_000 * 0.04 * 28 / 365)

    def test_custom_payoff_survives_loan_serialization_and_prices_gearing(self):
        payoff = (2 * qs.PayoffOps.Index + 0.005).max(0.02).min(0.09)
        restored_payoff = qs.PayoffOps.from_dict(payoff.to_dict())
        for fixing, expected in [(-0.01, 0.02), (0.03, 0.065), (0.10, 0.09)]:
            self.assertAlmostEqual(restored_payoff.eval(index_fixing=fixing), expected)
        coupon = qs.OptionEmbeddedCoupon(index=qs.MarketIndex.EURIBOR12m, payoff=payoff)
        loan = build_loan_contract(coupons=[coupon], repayment=qs.LoanRepaymentStructure.bullet())
        restored = qs.Loan.from_dict(loan.to_dict())
        self.assertEqual(restored.to_dict(), loan.to_dict())
        self.assertEqual(loan.to_dict()["coupons"][0]["OptionEmbeddedCoupon"]["payoff"], payoff.to_dict())
        with build_pricing_context(restored) as context:
            result = evaluate_loan_trade(restored, context, [qs.Request.Value, qs.Request.Cashflows])
        first = result.cashflows.loc[result.cashflows.type == "OptionEmbeddedCoupon"].iloc[0]
        self.assertAlmostEqual(first.amount, 100_000 * first.accrual_period * 0.085)

    def test_custom_payoff_sensitivity_matches_repricing(self):
        coupon = qs.OptionEmbeddedCoupon(
            index=qs.MarketIndex.EURIBOR12m,
            payoff=(2 * qs.PayoffOps.Index + 0.005).max(0.02).min(0.09))
        loan = build_loan_contract(coupons=[coupon], repayment=qs.LoanRepaymentStructure.bullet())
        with build_pricing_context(loan) as context:
            result = evaluate_loan_trade(loan, context, [qs.Request.Value, qs.Request.Sensitivities])
        euribor = result.sensitivities.loc[result.sensitivities.pillar.str.contains("EURIBOR12m"), "value"].sum()
        prices = []
        epsilon = 1e-6
        for shift in [-epsilon, epsilon]:
            with build_pricing_context(loan, forward=0.04 + shift) as context:
                prices.append(evaluate_loan_trade(loan, context, [qs.Request.Value]).price)
        self.assertAlmostEqual(euribor, (prices[1] - prices[0]) / (2 * epsilon), delta=0.01)

    def test_native_coupon_signatures_expose_parameter_names(self):
        for cls, names in [
            (qs.InterestRate, ["rate", "day_counter", "compounding", "frequency"]),
            (qs.FixedRateCoupon, ["rate", "notional", "payment_date"]),
            (qs.FloatingRateCoupon, ["index", "spread", "fixing"]),
            (qs.OptionEmbeddedCoupon, ["index", "payoff", "spread"]),
        ]:
            for name in names:
                self.assertIn(name, inspect.signature(cls).parameters)
                self.assertIn(name, cls.__doc__)

"""Contract reset, stub, reporting-window and origination prepayment regressions."""
import unittest

import pandas as pd
import quantsupport as qs

from test_loans import (
    REFERENCE, build_loan_contract, build_floating_rate_loan,
    build_pricing_context, evaluate_loan_trade,
)


class LoanScheduleTests(unittest.TestCase):
    def test_cashflow_request_defaults_and_parsing(self):
        default = qs.Request.Cashflows()
        complete = qs.Request.Cashflows(from_evaluation_date=False)
        self.assertEqual(default, qs.Request.Cashflows())
        self.assertTrue(default.from_evaluation_date)
        self.assertFalse(complete.from_evaluation_date)
        self.assertNotEqual(default, complete)
        self.assertEqual(complete, qs.Request.parse("Cashflows(false)"))
        self.assertEqual(complete, qs.Request.parse(str(complete)))
        self.assertEqual(repr(default), "Request.Cashflows(from_evaluation_date=True)")
        self.assertEqual(default, qs.Request.Cashflows(True))
        self.assertEqual(complete, qs.Request.Cashflows(False))
        self.assertEqual(default, qs.Request.parse("Cashflows"))
        self.assertFalse(callable(default))
        self.assertFalse(callable(qs.Request.Value))
        with self.assertRaises(TypeError):
            default(False)
        with self.assertRaises(qs.QuantSupportError):
            qs.Request.parse("Cashflows(maybe)")

    def test_advanced_context_retains_prepayment_from_origination(self):
        loan = build_loan_contract(
            prepayment=qs.PrepaymentModelConfiguration.constant_annual_cpr(0.12))
        full_request = qs.Request.Cashflows(from_evaluation_date=False)
        with build_pricing_context() as ctx:
            initial = evaluate_loan_trade(loan, ctx, [full_request]).cashflows
        later = "2027-02-24"
        with build_pricing_context(reference=later) as ctx:
            full = evaluate_loan_trade(loan, ctx, ["Value", full_request])
            future = evaluate_loan_trade(loan, ctx, [qs.Request.Cashflows(), "Value"])
            with self.assertRaises(qs.QuantSupportError):
                evaluate_loan_trade(loan, ctx, [full_request, qs.Request.Cashflows()])
        pd.testing.assert_frame_equal(initial, full.cashflows)
        self.assertEqual(full.price, future.price)
        self.assertIn("Disbursement", set(full.cashflows.type))
        self.assertTrue((future.cashflows.payment_date > later).all())
        principal = future.cashflows.loc[future.cashflows.type == "Redemption", "amount"].sum()
        self.assertAlmostEqual(principal, loan.contractual_principal(later) * 0.88, places=7)

    def test_french_stubs_and_rate_blocks_have_level_scheduled_installments(self):
        fixed = qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.06, day_counter="Actual360"))
        cases = [
            (dict(start_date="2026-02-10", first_coupon_date="2026-02-24"), None),
            (dict(maturity_date="2028-03-10", next_to_last_coupon_date="2028-02-24"), None),
            (dict(coupons=[fixed] * 12 + [qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.08, day_counter="Actual360"))]), 12),
            (dict(coupons=[qs.FloatingRateCoupon(index="EURIBOR12m", spread=0.01)], amortization_rate=0.05), 12),
        ]
        for overrides, split in cases:
            with self.subTest(overrides=overrides):
                loan = build_loan_contract(**overrides)
                with build_pricing_context(loan, forward=0.06, reference=overrides.get("start_date", REFERENCE)) as ctx:
                    flows = evaluate_loan_trade(loan, ctx, ["Cashflows"]).cashflows
                payments = flows[flows.leg_index == 0].groupby("payment_date").amount.sum()
                blocks = [payments] if split is None else [payments.iloc[:split], payments.iloc[split:]]
                for block in blocks:
                    self.assertLess(block.max() - block.min(), 1e-7)
                self.assertEqual(qs.Loan.from_dict(loan.to_dict()).to_dict(), loan.to_dict())

    def test_report_preserves_shared_reset_dates_and_nullable_principal_dates(self):
        loan = build_floating_rate_loan()
        with build_pricing_context(loan) as ctx:
            table = evaluate_loan_trade(loan, ctx, ["Cashflows"]).cashflows
        coupons = table[table.type == "FloatingRateCoupon"]
        requests = loan.coupon_fixing_requests()
        self.assertEqual(coupons.fixing_date.iloc[:12].tolist(), [requests.fixing_date.iloc[0]] * 12)
        self.assertEqual(coupons.fixing_date.iloc[12:].tolist(), [requests.fixing_date.iloc[1]] * 12)
        self.assertTrue(table.loc[table.type == "Redemption", "fixing_date"].isna().all())

    def test_portfolio_preserves_distinct_fixing_dates(self):
        loans = [build_floating_rate_loan(fixing_lag=lag) for lag in [1, 2]]
        ctx = build_pricing_context(loans[0])
        fixings = ctx.fixings
        for row in loans[1].coupon_fixing_requests().itertuples(index=False):
            if row.fixing_date < pd.Timestamp(REFERENCE):
                fixings.add(row.market_index, row.fixing_date, 0.04)
        ctx.fixings = fixings
        ctx.pricer_configurations = [qs.PricerConfiguration.loan()]
        with ctx:
            result = ctx.evaluate(qs.Portfolio("DATES", [qs.Trade(loan, REFERENCE) for loan in loans]), ["Cashflows"])
        coupons = result.cashflows[result.cashflows.type == "FloatingRateCoupon"]
        self.assertEqual(len(coupons), 48)
        self.assertTrue((coupons.groupby("payment_date").fixing_date.nunique() == 2).all())
        self.assertNotIn("fixing_index", coupons.columns)
        self.assertNotIn("rate_definition", coupons.columns)

    def test_missing_expired_reset_is_an_error_when_replaying_the_loan(self):
        loan = build_floating_rate_loan()
        requests = loan.coupon_fixing_requests()
        fixings = qs.FixingStore()
        row = requests.iloc[1]
        fixings.add(row.market_index, row.fixing_date, 0.04)
        ctx = build_pricing_context(reference="2027-03-24", include_fixings=False)
        ctx.fixings = fixings
        with ctx:
            for request in [qs.Request.Cashflows(), qs.Request.Cashflows(False)]:
                with self.assertRaises(qs.QuantSupportError):
                    evaluate_loan_trade(loan, ctx, [request])

    def test_bond_and_note_expose_stub_controls_and_round_trip(self):
        schedule = dict(date_generation_rule="Backward", end_of_month=False,
                        first_coupon_date="2026-03-24", next_to_last_coupon_date="2027-12-24")
        common = dict(identifier="STUB", start_date=REFERENCE, maturity_date="2028-02-24",
                      notional=100_000, currency="EUR", payment_frequency="Quarterly", **schedule)
        instruments = [
            qs.FixedRateBond(rate=0.05, discount_index="ESTR", **common),
            qs.FloatingRateNote(spread=0.01, forward_index="EURIBOR3m", **common),
        ]
        for instrument in instruments:
            self.assertEqual(type(instrument).from_dict(instrument.to_dict()).to_dict(), instrument.to_dict())
        maker = qs.MakeLoan(identifier="RELATIVE", start_tenor="0D", tenor="2Y", notional=100_000,
                            coupons=[qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.06))], currency="EUR",
                            first_coupon_tenor="1M", next_to_last_coupon_tenor="-1M")
        with build_pricing_context() as context:
            loan = maker.build_using(context)
        self.assertEqual(str(loan.first_coupon_date), "2026-03-24")
        self.assertEqual(str(loan.next_to_last_coupon_date), "2028-01-24")


if __name__ == "__main__":
    unittest.main()

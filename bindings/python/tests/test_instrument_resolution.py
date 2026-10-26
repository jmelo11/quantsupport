"""Pending product and trade terms resolve against each evaluation's market."""

import unittest

import quantsupport as qs
from pandas.testing import assert_frame_equal

from test_loans import build_pricing_context


class InstrumentResolutionTests(unittest.TestCase):
    def test_trade_and_portfolio_resolve_without_mutating_terms(self):
        pending = qs.Swap.from_tenors(identifier="SWAP", start_tenor="0D", tenor="2Y",
            notional=100_000, fixed_rate=0.03, currency="EUR", market_index="EURIBOR12m")
        trade = qs.Trade(pending)
        self.assertIsNone(trade.trade_date)
        self.assertEqual(str(trade.trade_tenor), "0D")
        before = pending.to_dict()
        for date in ["2026-02-24", "2027-02-24", "2026-02-24"]:
            with build_pricing_context(reference=date) as context:
                dated = trade.resolve_trade(context)
                self.assertEqual(str(dated.trade_date), date)
                self.assertIsNone(dated.trade_tenor)
                self.assertEqual(dated.instrument.to_dict()["start_date"], date)
                requests = ["Value", "Sensitivities", "Cashflows"]
                automatic = context.evaluate(trade, requests)
                explicit = context.evaluate(dated, requests)
                self.assertAlmostEqual(automatic.price, explicit.price, places=8)
                self.assertEqual(automatic.raw_sensitivities, explicit.raw_sensitivities)
                assert_frame_equal(automatic.cashflows, explicit.cashflows)
                portfolio = context.evaluate(qs.Portfolio("BOOK", [trade, trade]))
                self.assertAlmostEqual(portfolio.price, 2 * automatic.price, places=7)
        self.assertEqual(pending.to_dict(), before)
        self.assertIsNone(trade.trade_date)

    def test_initial_floating_loan_rate_is_resolved_and_can_be_frozen(self):
        pending = qs.Loan.from_tenors(identifier="LOAN", start_tenor="0D", tenor="2Y",
            notional=100_000, coupons=[qs.FloatingRateCoupon("EURIBOR12m", 0.01)], currency="EUR")
        trade = qs.Trade(pending)
        with build_pricing_context(forward=0.02) as low:
            frozen = trade.resolve_trade(low)
            rate = frozen.instrument.to_dict()["amortization_rate"]
            self.assertAlmostEqual(rate, 0.03, places=7)
            roundtrip = qs.Trade(qs.Loan.from_dict(frozen.instrument.to_dict()), frozen.trade_date)
            self.assertAlmostEqual(low.evaluate(frozen).price, low.evaluate(roundtrip).price, places=8)
            sized = qs.Trade(frozen.instrument, frozen.trade_date, notional=50_000)
            self.assertAlmostEqual(low.evaluate(trade).price, low.evaluate(frozen).price, places=8)
        with build_pricing_context(forward=0.08) as high:
            automatic = high.evaluate(trade, ["Value", "Cashflows"])
            fixed = high.evaluate(frozen, ["Value", "Cashflows"])
            self.assertAlmostEqual(high.evaluate(sized).price, fixed.price / 2, places=7)
        self.assertNotEqual(automatic.cashflows.amount.tolist(), fixed.cashflows.amount.tolist())

    def test_historical_policy_requires_fixings_and_today_projects(self):
        terms = dict(identifier="LOAN", start_date="2026-02-24", maturity_date="2028-02-24",
            notional=100_000, coupons=[qs.FloatingRateCoupon("EURIBOR12m", 0.01)], currency="EUR")
        pending = qs.Loan.from_tenors(**terms)
        fixings = qs.FixingStore()
        fixings.add("EURIBOR12m", "2026-02-24", 0.10)
        context = build_pricing_context()
        context.fixings = fixings
        with context:
            automatic = context.evaluate(qs.Trade(pending)).price
            context.fixings = qs.FixingStore()
            self.assertAlmostEqual(context.evaluate(qs.Trade(pending)).price, automatic, places=8)
        historical = build_pricing_context(reference="2026-03-10")
        with historical:
            with self.assertRaisesRegex(qs.QuantSupportError, "fixing"):
                historical.evaluate(qs.Trade(pending))
            historical.fixings = fixings
            self.assertIsNotNone(historical.evaluate(qs.Trade(pending)).price)
        self.assertTrue(historical.anchor_fixings)
        historical.anchor_fixings = False
        self.assertFalse(historical.anchor_fixings)

    def test_disabled_anchoring_projects_a_past_fixing_without_loading_history(self):
        pending = qs.Loan.from_tenors(identifier="LOAN", start_tenor="0D", tenor="2Y",
            notional=100_000, coupons=[qs.FloatingRateCoupon("EURIBOR12m", 0.01)],
            currency="EUR", calendar="TARGET", fixing_lag=2)
        terms = build_pricing_context()
        context = qs.PricingContext(terms.quotes, terms.curves,
            csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR"), anchor_fixings=False)
        with context:
            projected = context.evaluate(qs.Trade(pending)).price
            fake = qs.FixingStore()
            fake.add("EURIBOR12m", "2026-02-20", 0.50)
            context.fixings = fake
            self.assertAlmostEqual(context.evaluate(qs.Trade(pending)).price, projected, places=8)
            context.anchor_fixings = True
            self.assertNotAlmostEqual(context.evaluate(qs.Trade(pending)).price, projected, places=4)

    def test_all_bound_product_families_materialize(self):
        common = dict(identifier="CONTRACT", start_tenor="0D", tenor="2Y", currency="EUR")
        samples = [
            (qs.Swap, dict(common, notional=100, fixed_rate=0.03, market_index="EURIBOR6m")),
            (qs.BasisSwap, dict(common, notional=100, pay_market_index="EURIBOR3m", receive_market_index="EURIBOR6m")),
            (qs.CrossCurrencySwap, dict(identifier="XCCY", start_tenor="0D", tenor="2Y",
                domestic_notional=100, foreign_notional=110, domestic_currency="EUR", foreign_currency="USD",
                domestic_market_index="EURIBOR6m", foreign_market_index="TermSOFR3m")),
            (qs.FixFloatCrossCurrencySwap, dict(identifier="XCCY", start_tenor="0D", tenor="2Y",
                domestic_notional=100, foreign_notional=110, domestic_currency="EUR", foreign_currency="USD",
                fixed_rate=0.03, floating_index="TermSOFR3m")),
            (qs.FixedRateBond, dict(common, notional=100, rate=0.03, first_coupon_tenor="3M")),
            (qs.SteppedCouponBond, dict(common, notional=100, coupons=[qs.FixedRateCoupon(0.03)],
                notional_percentages=[1.0], first_coupon_tenor="3M", next_to_last_coupon_tenor="-3M",
                call_schedule_tenors=[("1Y", 100.0)])),
            (qs.FloatingRateNote, dict(common, notional=100, spread=0.01, forward_index="EURIBOR6m")),
            (qs.FixedRateDeposit, dict(common, notional=100, rate=0.03)),
            (qs.Loan, dict(common, notional=100, coupons=[qs.FixedRateCoupon(0.03)], prepayment_tenors=[("0D", 0.1)])),
            (qs.CreditDefaultSwap, dict(common, spread=0.01, credit_index={"Credit": "ACME"}, discount_index="ESTR")),
            (qs.CapFloor, dict(common, strike=0.03, notional=100, market_index="EURIBOR6m", cap_floor_type="Cap")),
            (qs.CapletFloorlet, dict(identifier="CAPLET", start_tenor="1Y", tenor="6M",
                fixing_tenor="-2D", payment_tenor="2D", strike=0.03, currency="EUR", market_index="EURIBOR6m", payoff_type="Caplet")),
            (qs.RateFutures, dict(identifier="FUTURE", start_tenor="1Y", tenor="3M", futures_price=96, market_index="EURIBOR3m")),
            (qs.EuropeanSwaption, dict(identifier="SWAPTION", expiry_tenor="1Y", tenor="2Y", strike=0.03,
                notional=100, currency="EUR", market_index="EURIBOR6m")),
            (qs.FxForward, dict(identifier="FX", delivery_tenor="1Y", base_currency="EUR", quote_currency="USD", forward_price=1.1)),
            (qs.FxOption, dict(identifier="FX", expiry_tenor="1Y", strike=1.1, option_type="Call", base_currency="EUR", quote_currency="USD")),
            (qs.EquityForward, dict(identifier="EQ", market_index={"Equity": "SPX"}, delivery_tenor="1Y", strike=100, currency="USD")),
            (qs.EquityOption, dict(identifier="EQ", market_index={"Equity": "SPX"}, expiry_tenor="1Y", strike=100, option_type="Call")),
            (qs.ScriptedProduct, dict(name="SCRIPT", currency="EUR", event_tenors=[("1Y", "value = 0; value pays 100;")])),
        ]
        with build_pricing_context() as context:
            for product, parameters in samples:
                with self.subTest(product=product.__name__):
                    pending = product.from_tenors(**parameters)
                    concrete = pending.resolve_instrument(context)
                    self.assertIsInstance(concrete, product)
                    self.assertNotIn("start_tenor", concrete.to_dict())

    def test_terms_are_copied_and_subclasses_keep_their_type(self):
        class DeskSwap(qs.Swap):
            pass

        pending = DeskSwap.from_tenors(identifier="SWAP", start_tenor="0D", tenor="1Y",
            notional=100, fixed_rate=0.03, currency="EUR", market_index="EURIBOR12m")
        coupons = [{"Fixed": {"rate": 0.03}}]
        loan = qs.Loan.from_tenors(identifier="LOAN", start_tenor="0D", tenor="1Y",
            notional=100, currency="EUR", coupons=coupons)
        coupons[0]["Fixed"]["rate"] = 0.50
        copied = loan.to_dict()
        copied["parameters"]["coupons"][0]["Fixed"]["rate"] = 0.75
        self.assertEqual(loan.to_dict()["parameters"]["coupons"][0]["Fixed"]["rate"], 0.03)
        with build_pricing_context() as context:
            self.assertIsInstance(pending.resolve_instrument(context), DeskSwap)
        with self.assertRaises(ValueError):
            qs.Swap.from_tenors(start_date="2026-02-24", start_tenor="0D")
        with self.assertRaises(ValueError):
            qs.Trade(pending, "2026-02-24", trade_tenor="0D")


if __name__ == "__main__":
    unittest.main()

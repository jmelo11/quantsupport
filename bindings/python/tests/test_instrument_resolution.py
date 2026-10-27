"""Products are built explicitly before entering trades, portfolios, or valuation."""
import unittest

import quantsupport as qs
from pandas.testing import assert_frame_equal
from test_loans import build_pricing_context


class InstrumentConstructionTests(unittest.TestCase):
    def test_maker_requires_context_and_is_reusable_across_dates(self):
        maker = qs.MakeSwap(identifier="SWAP", start_tenor="0D", tenor="2Y",
            notional=100_000, fixed_rate=0.03, currency="EUR", market_index="EURIBOR12m")
        before = maker.to_dict()
        self.assertFalse(maker.is_resolved())
        with self.assertRaises(qs.UnresolvedProductErr) as raised:
            maker.build()
        self.assertEqual(raised.exception.code, "unresolved_product")
        self.assertEqual(raised.exception.native_type, "UnresolvedProductErr")
        self.assertIn("start_tenor", str(raised.exception))
        self.assertIn("maker.build_using(context)", str(raised.exception))
        with self.assertRaises(qs.UnresolvedProductErr):
            qs.Trade(maker, "2026-02-24")
        for date in ["2026-02-24", "2027-02-24", "2026-02-24"]:
            with build_pricing_context(reference=date) as context:
                product = maker.build_using(context)
                self.assertEqual(product.to_dict()["start_date"], date)
                trade = qs.MakeTrade(instrument=product, trade_tenor="0D").build_using(context)
                self.assertEqual(str(trade.trade_date), date)
                result = context.evaluate(trade)
                portfolio = context.evaluate(qs.Portfolio("BOOK", [trade, trade]))
                self.assertAlmostEqual(portfolio.price, 2 * result.price, places=7)
        self.assertEqual(maker.to_dict(), before)
        self.assertFalse(maker.is_resolved())

    def test_maturity_tenor_requires_resolution_even_with_an_absolute_start(self):
        maker = qs.MakeFixedRateDeposit(identifier="DEPOSIT", start_date="2026-02-24",
            tenor="2Y", notional=100, rate=0.03, currency="EUR")
        self.assertFalse(maker.is_resolved())
        with self.assertRaisesRegex(qs.UnresolvedProductErr, "tenor.*build_using"):
            maker.build()
        with build_pricing_context() as context:
            product = maker.build_using(context)
        self.assertEqual(str(product.maturity_date), "2028-02-24")
        explicit = qs.MakeFixedRateDeposit(identifier="DEPOSIT", start_date="2026-02-24",
            maturity_date="2028-02-24", notional=100, rate=0.03, currency="EUR")
        self.assertTrue(explicit.is_resolved())
        self.assertEqual(explicit.build().to_dict(), product.to_dict())
        with self.assertRaises(qs.UnresolvedProductErr):
            qs.Trade(product)
        trade = qs.Trade(product, "2026-02-24")
        self.assertEqual(str(trade.trade_date), "2026-02-24")

    def test_explicit_caplet_defaults_dates_without_resolving_tenors(self):
        terms = dict(identifier="CAPLET", start_accrual_date="2027-02-24",
            end_accrual_date="2027-08-24", strike=0.03, currency="EUR",
            market_index="EURIBOR6m", payoff_type="Caplet")
        maker = qs.MakeCapletFloorlet(**terms)
        self.assertTrue(maker.is_resolved())
        product = maker.build().to_dict()
        self.assertEqual(product["fixing_date"], terms["start_accrual_date"])
        self.assertEqual(product["payment_date"], terms["end_accrual_date"])
        relative = qs.MakeCapletFloorlet(**terms, payment_tenor="2D")
        self.assertFalse(relative.is_resolved())
        with self.assertRaisesRegex(qs.UnresolvedProductErr, "payment_tenor.*build_using"):
            relative.build()
        with build_pricing_context() as context:
            self.assertEqual(maker.build_using(context).to_dict(), product)
            self.assertGreater(relative.build_using(context).payment_date,
                qs.Date(2027, 8, 24))

    def test_floating_loan_construction_freezes_principal_and_serialization(self):
        maker = qs.MakeLoan(identifier="LOAN", start_date="2026-02-24", maturity_date="2028-02-24",
            notional=100_000, coupons=[qs.FloatingRateCoupon("EURIBOR12m", 0.01)], currency="EUR")
        self.assertFalse(maker.is_resolved())
        with self.assertRaises(qs.UnresolvedProductErr):
            maker.build()
        with build_pricing_context(forward=0.02) as low:
            loan = maker.build_using(low)
            self.assertAlmostEqual(loan.to_dict()["amortization_rate"], 0.03, places=7)
            trade = qs.Trade(loan, "2026-02-24")
            reconstructed = qs.Trade(qs.Loan.from_dict(loan.to_dict()), "2026-02-24")
            self.assertAlmostEqual(low.evaluate(trade).price, low.evaluate(reconstructed).price, places=8)
        before = loan.to_dict()
        with build_pricing_context(forward=0.08) as high:
            fresh = maker.build_using(high)
            old_result = high.evaluate(trade, ["Value", "Cashflows"])
            fresh_result = high.evaluate(qs.Trade(fresh, "2026-02-24"), ["Value", "Cashflows"])
            self.assertNotEqual(old_result.cashflows.amount.tolist(), fresh_result.cashflows.amount.tolist())
            repeated = high.evaluate(trade, ["Value", "Cashflows"])
            assert_frame_equal(old_result.cashflows, repeated.cashflows)
        self.assertEqual(loan.to_dict(), before)

    def test_missing_historical_fixings_fail_during_construction(self):
        maker = qs.MakeLoan(identifier="LOAN", start_date="2026-02-24", maturity_date="2028-02-24",
            notional=100_000, coupons=[qs.FloatingRateCoupon("EURIBOR12m", 0.01)], currency="EUR")
        fixings = qs.FixingStore()
        fixings.add("EURIBOR12m", "2026-02-24", 0.10)
        with build_pricing_context() as today:
            today.fixings = fixings
            first = maker.build_using(today).amortization_rate
            today.fixings = qs.FixingStore()
            self.assertAlmostEqual(maker.build_using(today).amortization_rate, first, places=8)
        with build_pricing_context(reference="2026-03-10") as historical:
            with self.assertRaisesRegex(qs.QuantSupportError, "fixing"):
                maker.build_using(historical)
            historical.fixings = fixings
            self.assertAlmostEqual(maker.build_using(historical).amortization_rate, 0.11, places=8)
            historical.anchor_fixings = False
            self.assertFalse(historical.anchor_fixings)

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
                    maker = getattr(qs, "Make" + product.__name__)(**parameters)
                    self.assertFalse(maker.is_resolved())
                    with self.assertRaises(qs.UnresolvedProductErr):
                        maker.build()
                    concrete = maker.build_using(context)
                    self.assertIsInstance(concrete, product)
                    self.assertNotIn("start_tenor", concrete.to_dict())


    def test_parameter_copies_subclasses_and_input_errors(self):
        class DeskSwap(qs.Swap):
            pass
        terms = dict(identifier="SWAP", start_tenor="0D", tenor="1Y",
            notional=100, fixed_rate=0.03, currency="EUR", market_index="EURIBOR12m")
        maker = qs.MakeInstrument(DeskSwap, terms)
        coupons = [{"Fixed": {"rate": 0.03}}]
        loan = qs.MakeLoan(identifier="LOAN", start_tenor="0D", tenor="1Y",
            notional=100, currency="EUR", coupons=coupons)
        coupons[0]["Fixed"]["rate"] = 0.5
        copied = loan.to_dict()
        copied["coupons"][0]["Fixed"]["rate"] = 0.75
        self.assertEqual(loan.to_dict()["coupons"][0]["Fixed"]["rate"], 0.03)
        with build_pricing_context() as context:
            self.assertIsInstance(maker.build_using(context), DeskSwap)
        with self.assertRaises(ValueError):
            qs.MakeSwap(start_date="2026-02-24", start_tenor="0D")
        with self.assertRaises(qs.QuantSupportValueError):
            qs.MakeLoan(identifier="BAD", start_date="2026-02-24", maturity_date="2028-02-24",
                notional=-1, currency="EUR", coupons=[qs.FixedRateCoupon(0.03)]).build()

    def test_subclasses_keep_product_construction_rules(self):
        class DeskSwap(qs.Swap):
            pass

        class DeskLoan(qs.Loan):
            pass

        terms = dict(identifier="SWAP", start_tenor="0D", tenor="1Y",
            notional=100, fixed_rate=0.03, currency="EUR", market_index="EURIBOR12m")
        with build_pricing_context(reference="2026-02-28") as context:
            standard = qs.MakeSwap(**terms).build_using(context)
            custom = qs.MakeInstrument(DeskSwap, terms).build_using(context)
            self.assertIsInstance(custom, DeskSwap)
            self.assertEqual(custom.to_dict(), standard.to_dict())
        terms = dict(identifier="LOAN", start_date="2026-02-24", maturity_date="2028-02-24",
            notional=100, coupons=[qs.FloatingRateCoupon("EURIBOR12m", 0.01)], currency="EUR")
        maker = qs.MakeInstrument(DeskLoan, terms)
        with self.assertRaises(qs.UnresolvedProductErr):
            maker.build()
        with build_pricing_context() as context:
            custom = maker.build_using(context)
            self.assertIsInstance(custom, DeskLoan)
            self.assertEqual(custom.to_dict(), qs.MakeLoan(**terms).build_using(context).to_dict())
            with self.assertRaisesRegex(qs.QuantSupportValueError, "build_using"):
                qs.MakeLoan(**terms, context=context)

    def test_readiness_uses_explicit_inputs_without_accessing_a_context(self):
        terms = dict(identifier="LOAN", start_date="2026-02-24", maturity_date="2028-02-24",
            notional=100, coupons=[qs.FloatingRateCoupon("EURIBOR12m", 0.01)], currency="EUR")
        maker = qs.MakeLoan(**terms, amortization_rate=0.03)
        self.assertTrue(maker.is_resolved())
        self.assertIsInstance(maker.build(), qs.Loan)
        bullet = qs.MakeLoan(**terms, repayment="Bullet")
        self.assertTrue(bullet.is_resolved())
        self.assertIsInstance(bullet.build(), qs.Loan)
        anchored = qs.MakeInstrument(qs.FixedRateDeposit, dict(identifier="D", reference_date="2026-02-24",
            start_tenor="0D", tenor="1Y", notional=100, rate=0.03, currency="EUR"))
        self.assertFalse(anchored.is_resolved())
        with self.assertRaisesRegex(qs.UnresolvedProductErr, "build_using"):
            anchored.build()
        with build_pricing_context() as context:
            self.assertEqual(str(anchored.build_using(context).maturity_date), "2027-02-24")
        invalid = qs.MakeLoan(identifier="INVALID", start_date="2026-02-24",
            maturity_date="2028-02-24", notional=-1, coupons=[qs.FixedRateCoupon(0.03)], currency="EUR")
        self.assertTrue(invalid.is_resolved())
        with self.assertRaises(qs.QuantSupportValueError):
            invalid.build()


if __name__ == "__main__":
    unittest.main()

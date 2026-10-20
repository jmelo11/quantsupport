"""Behavioral tests for Python-native extension points.

Run after installing the extension with maturin develop:
    python -m unittest discover bindings/python/tests
"""

from dataclasses import dataclass
import unittest

import quantsupport as qs


class Coupon(qs.Instrument):
    def __init__(self, identifier: str, amount: float):
        super().__init__(identifier)
        self.amount = amount


class CouponTrade(qs.Trade):
    pass


class CouponPricer(qs.Pricer):
    def evaluate(self, trade, requests, context):
        return qs.EvaluationResults(
            context.evaluation_date,
            trade.instrument.identifier,
            price=(
                trade.instrument.amount * trade.side.sign()
                if qs.Request.Value in requests
                else None
            ),
            sensitivities={"CUSTOM_AMOUNT": 1.0},
            cashflows=[
                {
                    "payment_date": str(context.evaluation_date),
                    "type": "CustomCoupon",
                    "amount": trade.instrument.amount,
                    "currency": "USD",
                }
            ],
        )


@dataclass
class DuckTrade:
    identifier: str
    value: float


class DuckPricer:
    def evaluate(self, trade, requests, provider):
        return qs.EvaluationResults(
            provider.evaluation_date,
            trade.identifier,
            price=trade.value * provider.multiplier,
        )


class Provider:
    evaluation_date = "2025-01-02"
    multiplier = 3.0


class ExtensionTests(unittest.TestCase):
    def make_context(self):
        quotes = qs.QuoteStore.from_dict(
            {"reference_date": "2025-01-02", "quotes": []}
        )
        return qs.PricingContext(quotes=quotes, curves=[])

    def test_context_dispatches_to_python_pricer(self):
        context = self.make_context()
        context.register_pricer(CouponTrade, CouponPricer())
        instrument = Coupon("COUPON", 125.0)
        trade = CouponTrade(instrument, "2025-01-02", qs.Side.PayShort)

        with context:
            result = context.evaluate(
                trade,
                [qs.Request.Value, qs.Request.Sensitivities, qs.Request.Cashflows],
            )

        self.assertEqual(result.identifier, "COUPON")
        self.assertEqual(str(result.evaluation_date), "2025-01-02")
        self.assertEqual(result.price, -125.0)
        self.assertEqual(result.raw_sensitivities, [("CUSTOM_AMOUNT", 1.0)])

    def test_fair_rate_target_survives_native_request_conversion(self):
        class TargetPricer(qs.Pricer):
            def evaluate(self, trade, requests, context):
                self.received = requests
                return qs.EvaluationResults(context.evaluation_date, trade.instrument.identifier,
                                            fair_rate=float(requests[0].index))
        context = self.make_context()
        pricer = TargetPricer()
        context.register_pricer(CouponTrade, pricer)
        with context:
            result = context.evaluate(CouponTrade(Coupon("TARGET", 1), "2025-01-02"), [qs.Request.FairRate(1)])
        self.assertEqual(pricer.received, [qs.Request.FairRate(1)])
        self.assertEqual(result.fair_rate, 1.0)

    def test_registration_can_be_removed(self):
        context = self.make_context()
        context.register_pricer(CouponTrade, CouponPricer())
        self.assertIsInstance(
            context.pricer_for(CouponTrade(Coupon("X", 1.0), "2025-01-02")),
            CouponPricer,
        )
        self.assertTrue(context.unregister_pricer(CouponTrade))
        self.assertFalse(context.unregister_pricer(CouponTrade))

    def test_standalone_evaluator_is_structural(self):
        evaluator = qs.Evaluator()
        evaluator.register(DuckTrade, DuckPricer())
        result = evaluator.evaluate(DuckTrade("DUCK", 14.0), Provider())
        self.assertEqual(result.identifier, "DUCK")
        self.assertEqual(result.price, 42.0)

    def test_base_type_registration_handles_subclasses(self):
        class ChildTrade(DuckTrade):
            pass

        evaluator = qs.Evaluator()
        evaluator.register(DuckTrade, DuckPricer())
        result = evaluator.evaluate(ChildTrade("CHILD", 2.0), Provider())
        self.assertEqual(result.price, 6.0)

    def test_native_instruments_and_their_trades_are_subclassable(self):
        class ExtendedSwap(qs.Swap):
            pass

        class ExtendedSwapTrade(qs.Trade):
            pass

        class OverridePricer:
            def evaluate(self, trade, requests, context):
                return qs.EvaluationResults(
                    context.evaluation_date, trade.instrument.identifier, price=99.0
                )

        swap = ExtendedSwap(
            identifier="EXTENDED-SWAP",
            start_date="2025-01-02",
            maturity_date="2026-01-02",
            notional=1_000_000.0,
            fixed_rate=0.03,
            currency=qs.Currency.USD,
            market_index=qs.MarketIndex.SOFR,
        )
        swap.python_metadata = {"desk": "custom"}
        context = self.make_context()
        trade = ExtendedSwapTrade(swap, "2025-01-02")
        context.register_pricer(ExtendedSwapTrade, OverridePricer())
        with context:
            result = context.evaluate(trade)

        self.assertEqual(result.price, 99.0)
        self.assertIsInstance(swap, qs.Instrument)
        self.assertIs(trade.instrument, swap)
        self.assertEqual(swap.python_metadata["desk"], "custom")


if __name__ == "__main__":
    unittest.main()

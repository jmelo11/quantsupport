"""Native pricing keeps contract terms separate from position metadata."""

from pathlib import Path
import unittest

import quantsupport as qs


class TradeInstrumentTests(unittest.TestCase):
    def make_swap(self):
        return qs.Swap(
            identifier="POSITION-SWAP", start_date="2026-02-24",
            maturity_date="2031-02-24", notional=1_000_000,
            fixed_rate=0.04, currency="USD", market_index="SOFR",
        )

    def make_context(self):
        data = Path(__file__).resolve(
        ).parents[3] / "examples" / "hullwhite" / "data"
        return qs.PricingContext(
            quotes=qs.QuoteStore.from_json(str(data / "quotes.json")),
            curves=qs.CurveConfiguration.from_json(
                str(data / "curve_specs.json")),
            csa_policy=qs.SingleCurveCSADiscountPolicy("USD", "SOFR"),
        )

    def test_position_size_does_not_mutate_shared_contract(self):
        instrument = self.make_swap()
        full = qs.Trade(instrument, "2026-02-24")
        half_short = qs.Trade(instrument, "2026-02-24",
                              "PayShort", notional=500_000)
        zero = qs.Trade(instrument, "2026-02-24", notional=0.0)
        with self.make_context() as context:
            full_value = context.evaluate(full).price
            self.assertAlmostEqual(context.evaluate(
                half_short).price, -0.5 * full_value)
            self.assertEqual(context.evaluate(zero).price, 0.0)
            self.assertAlmostEqual(context.evaluate(full).price, full_value)
        self.assertEqual(instrument.notional, 1_000_000)
        self.assertIsInstance(instrument, qs.Instrument)
        self.assertIsInstance(half_short, qs.Trade)

    def test_position_keywords_do_not_belong_to_instrument_constructors(self):
        spec = dict(identifier="SWAP", start_date="2026-02-24", maturity_date="2031-02-24",
                    notional=100.0, fixed_rate=0.04, currency="USD", market_index="SOFR")
        for key, value in [("side", "PayShort"), ("trade_date", "2026-02-24")]:
            with self.subTest(key=key), self.assertRaises(TypeError):
                qs.Swap.from_dict(dict(spec, **{key: value}))
        futures = qs.RateFutures(
            "FUTURE", "2027-02-24", "2027-05-24", 96.0, "SOFR")
        self.assertFalse(hasattr(futures, "num_contracts"))
        self.assertEqual(qs.Trade(futures, "2026-02-24",
                         num_contracts=3).num_contracts, 3)

    def test_invalid_position_sizes_are_rejected(self):
        instrument = self.make_swap()
        for amount in [-1.0, float("nan"), float("inf")]:
            for field in ["notional", "domestic_notional", "foreign_notional", "num_contracts"]:
                with self.subTest(field=field, amount=amount), self.assertRaises(ValueError):
                    qs.Trade(instrument, "2026-02-24", **{field: amount})

    def test_netting_requires_positions_and_offsets_opposite_trades(self):
        instrument = self.make_swap()
        csa = qs.CsaTerms(collateral_index="SOFR", collateral_currency="USD",
                          credit_spread=0.01, recovery=0.4, funding_spread=0.005)
        with self.assertRaisesRegex(TypeError, "Trade"):
            qs.NettingSet("INVALID", [instrument], csa)
        positions = [qs.Trade(instrument, "2026-02-24", "LongReceive"),
                     qs.Trade(instrument, "2026-02-24", "PayShort")]
        netting_set = qs.NettingSet("OFFSET", positions, csa)
        self.assertEqual(len(netting_set), 2)
        config = qs.XvaConfig.from_dict({
            "model_configs": [{"market_index": "SOFR", "lambda": 0.05,
                               "parameter_source": {"Fixed": {"sigma": 0.01}}}],
            "n_paths": 4, "seed": 42, "frequency": "Annual",
        })
        with self.make_context() as context:
            for _ in range(2):
                result = context.run_xva(config, [netting_set])
                profile = result.exposures[0].to_dataframe()
                self.assertLess(profile["epe"].abs().max(), 1e-8)
                self.assertLess(profile["ene"].abs().max(), 1e-8)

    def test_native_trades_share_an_instrument_without_sharing_side(self):
        data = Path(__file__).resolve(
        ).parents[3] / "examples" / "hullwhite" / "data"
        quotes = qs.QuoteStore.from_json(str(data / "quotes.json"))
        curves = qs.CurveConfiguration.from_json(
            str(data / "curve_specs.json"))
        reference_date = quotes.reference_date
        instrument = qs.Swap(
            identifier="SHARED-SWAP",
            start_date=reference_date,
            maturity_date=reference_date + "5Y",
            notional=1_000_000,
            fixed_rate=0.04,
            currency=qs.Currency.USD,
            market_index=qs.MarketIndex.SOFR,
        )
        long = qs.Trade(instrument, reference_date, qs.Side.LongReceive)
        short = qs.Trade(instrument, reference_date, qs.Side.PayShort)
        self.assertIs(long.instrument, instrument)
        self.assertIs(short.instrument, instrument)
        with qs.PricingContext(
            quotes=quotes,
            curves=curves,
            csa_policy=qs.SingleCurveCSADiscountPolicy(
                qs.Currency.USD, qs.MarketIndex.SOFR),
        ) as context:
            long_value = context.evaluate(long).price
            short_value = context.evaluate(short).price
            self.assertAlmostEqual(long_value, -short_value)
            with self.assertRaisesRegex(TypeError, "Trade"):
                context.evaluate(instrument)
        self.assertFalse(hasattr(instrument, "trade_date"))
        self.assertFalse(hasattr(instrument, "side"))


if __name__ == "__main__":
    unittest.main()

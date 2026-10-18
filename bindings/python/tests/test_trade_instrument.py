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

    def test_new_products_price_and_scale_positions(self):
        data = Path(__file__).resolve().parents[3] / "examples" / "hullwhite" / "data"
        quotes = qs.QuoteStore.from_json(str(data / "quotes.json"))
        curves = qs.CurveConfiguration.from_json(str(data / "curve_specs.json"))
        date = quotes.reference_date
        fixings = qs.FixingStore()
        fixings.add("SOFR", date, 100.0)
        configuration = qs.PricerConfiguration.from_dict({
            "instrument_type": "EuropeanSwaption", "pricer": "HullWhiteClosedForm",
            "models": [{"market_index": "SOFR", "model": {"HullWhite": {
                "alpha": 0.1, "parameter_source": {"Fixed": {"sigma": 0.01}}
            }}}],
        })
        forward = qs.EquityForward.from_dict(dict(identifier="FWD", market_index="SOFR",
            delivery_date=str(date + "1Y"), strike=100.0, currency="USD"))
        swaption = qs.EuropeanSwaption.from_dict(dict(identifier="SWPT", expiry_date=str(date + "1Y"),
            maturity_date=str(date + "5Y"), strike=0.04, notional=1_000_000.0,
            currency="USD", market_index="SOFR", swaption_type="Payer"))
        self.assertEqual(swaption.swaption_type, qs.SwaptionType.Payer)
        with qs.PricingContext(quotes, curves, fixings=fixings, pricer_configurations=[configuration],
                              csa_policy=qs.SingleCurveCSADiscountPolicy("USD", "SOFR")) as context:
            for instrument in [forward, swaption]:
                long = qs.Trade(instrument, date)
                full = context.evaluate(long, ["Value", "Sensitivities"])
                self.assertGreater(full.price, 0.0)
                self.assertGreater(len(full.sensitivities), 0)
                amount = 0.5 if instrument is forward else 500_000.0
                short = qs.Trade(instrument, date, "PayShort", notional=amount)
                self.assertAlmostEqual(context.evaluate(short).price, -0.5 * full.price, places=7)
                self.assertEqual(context.evaluate(qs.Trade(instrument, date, notional=0.0)).price, 0.0)
            fair = context.evaluate(qs.Trade(forward, date), ["FairRate"]).fair_rate
            self.assertGreater(fair, 100.0)
            at_market = qs.EquityForward("PAR", "SOFR", date + "1Y", fair, "USD")
            self.assertAlmostEqual(context.evaluate(qs.Trade(at_market, date)).price, 0.0, places=9)
        with self.make_context() as context:
            with self.assertRaisesRegex(qs.QuantSupportError, "configuration"):
                context.evaluate(qs.Trade(swaption, date))
        for bad in [float("nan"), float("inf")]:
            with self.assertRaises(ValueError):
                qs.EquityForward("BAD", "SOFR", date + "1Y", bad, "USD")
        with self.assertRaises(ValueError):
            qs.EuropeanSwaption("BAD", date + "5Y", date + "1Y", 0.04, 1.0, "USD", "SOFR")

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

    def test_swap_cashflows_are_signed_and_scale_with_position(self):
        instrument = self.make_swap()
        long = qs.Trade(instrument, "2026-02-24")
        short = qs.Trade(instrument, "2026-02-24", "PayShort", notional=500_000)
        zero = qs.Trade(instrument, "2026-02-24", notional=0.0)
        with self.make_context() as context:
            result = context.evaluate(long, ["Cashflows", "Value"])
            table = result.cashflows
            curve = context.curve("SOFR")
            reported_pv = sum(row.amount * curve.discount_factor(row.payment_date)
                              for row in table.itertuples(index=False))
            self.assertAlmostEqual(reported_pv, result.price, places=7)
            for row in table.itertuples(index=False):
                receipt = (row.leg_index == 0) != (row.type == "Disbursement")
                self.assertEqual(row.amount > 0, receipt)
            short_table = context.evaluate(short, ["Cashflows"]).cashflows
            self.assertEqual(len(short_table), len(table))
            for amount, short_amount in zip(table.amount, short_table.amount):
                self.assertAlmostEqual(short_amount, -0.5 * amount, places=7)
            zero_table = context.evaluate(zero, ["Cashflows"]).cashflows
            self.assertTrue((zero_table.amount == 0.0).all())
        self.assertEqual(instrument.notional, 1_000_000)

    def test_swaptions_use_shared_surface_and_cube_calibration(self):
        date = qs.Date(2026, 2, 24)
        option = qs.EuropeanSwaption("CALIBRATED", date + "30M", date + "90M",
                                    0.04, 1_000_000.0, "USD", "SOFR")
        trade = qs.Trade(option, date)
        curve_ids = ["FixedRateDeposit_USD_SOFR_1D", "OIS_USD_SOFR_5Y", "OIS_USD_SOFR_10Y"]
        curves = [qs.CurveConfiguration.from_dict({"market_index": "SOFR", "quotes": curve_ids})]
        for source in ["Surface", "Cube"]:
            records = [{"identifier": identifier, "mid": 0.04, "bid": 0.04, "ask": 0.04}
                       for identifier in curve_ids]
            volatility_ids = []
            for expiry, volatility in [("1Y", 0.2), ("2Y", 0.3), ("3Y", 0.4)]:
                for strike in [0.02, 0.06]:
                    identifier = (f"Swaption_USD_SOFR_{expiry}_5Y_Absolute_{strike}_Black"
                                  if source == "Cube" else
                                  f"CapletFloorlet_USD_SOFR_3M_{expiry}_Absolute_{strike}_Straddle_Black")
                    records.append({"identifier": identifier, "mid": volatility,
                                    "bid": volatility * 0.8, "ask": volatility * 1.2})
                    volatility_ids.append(identifier)
            quotes = qs.QuoteStore.from_dict({"reference_date": str(date), "quotes": records})
            config_type = (qs.VolatilityCubeConfiguration if source == "Cube"
                           else qs.VolatilitySurfaceConfiguration)
            volatility = config_type.from_dict({"market_index": "SOFR", "quotes": volatility_ids})
            inputs = {"volatility_cubes" if source == "Cube" else "volatility_surfaces": [volatility]}
            model = {"HullWhite": {"alpha": 0.1, "parameter_source": {"Calibrated": {
                "source": {source: {"market_index": "SOFR"}},
                "calibration_basket": {"strike": "Atm"},
            }}}}
            configuration = qs.PricerConfiguration.from_dict({
                "instrument_type": "EuropeanSwaption", "pricer": "HullWhiteClosedForm",
                "models": [{"market_index": "SOFR", "model": model}],
            })
            simulation = qs.SimulationConfiguration.from_dict({
                "market_index": "SOFR", "model": model, "n_paths": 2,
                "seed": 42, "horizon": "3Y", "frequency": "Annual",
            })
            prices = []
            for level in [None, qs.Level.Bid, qs.Level.Ask]:
                with self.subTest(source=source, level=level), qs.PricingContext(
                    quotes, curves, pricer_configurations=[configuration], simulations=[simulation],
                    csa_policy=qs.SingleCurveCSADiscountPolicy("USD", "SOFR"),
                    quote_level=level, **inputs
                ) as context:
                    result = context.evaluate(trade, ["Value", "Sensitivities"])
                    prices.append(result.price)
                    self.assertGreater(result.price, 0.0)
                    report = context.simulation("SOFR").calibration_diagnostics()
                    self.assertEqual(len(report), 3)
                    self.assertGreater(report["calibrated_sigma"].max() - report["calibrated_sigma"].min(), 0.001)
                    risk = result.sensitivities
                    self.assertTrue(any(label in volatility_ids for label in risk["pillar"]))
                    self.assertGreater(risk.loc[risk["pillar"].isin(volatility_ids), "value"].abs().sum(), 1.0)
            self.assertLess(prices[1], prices[0])
            self.assertLess(prices[0], prices[2])

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

"""Native pricing keeps contract terms separate from position metadata."""

from pathlib import Path
import unittest

import quantsupport as qs


class TradeInstrumentTests(unittest.TestCase):
    def test_deposits_and_portfolios_use_standard_results(self):
        instrument = qs.FixedRateDeposit("DEPOSIT", "2026-02-24", "2027-02-24", 100_000, 0.04, "USD", "SOFR")
        long = qs.Trade(instrument, "2026-02-24")
        short = qs.Trade(instrument, "2026-02-24", "PayShort", notional=50_000)
        portfolio = qs.Portfolio("DEPOSIT-BOOK", [long, short, long])
        self.assertEqual(portfolio.identifier, "DEPOSIT-BOOK")
        self.assertIn("DEPOSIT-BOOK", repr(portfolio))
        self.assertEqual(len(portfolio), 3)
        with self.make_context() as context:
            single = context.evaluate(long, ["Value", "Sensitivities", "Cashflows"])
            result = context.evaluate(portfolio, ["Value", "Sensitivities", "Cashflows"])
            self.assertIsInstance(result, qs.EvaluationResults)
            self.assertEqual(result.identifier, "DEPOSIT-BOOK")
            self.assertAlmostEqual(result.price, 1.5 * single.price, places=7)
            expected = dict(single.raw_sensitivities)
            for factor, value in result.raw_sensitivities:
                self.assertAlmostEqual(value, 1.5 * expected[factor], places=6)
            summary = result.cashflow_summary
            self.assertEqual(list(summary.columns), ["payment_date", "currency", "interest", "capital", "other", "total"])
            self.assertTrue((summary.currency == "USD").all())
            self.assertAlmostEqual(summary.capital.sum(), 0.0)
            self.assertGreater(summary.interest.sum(), 0.0)
            self.assertEqual(set(result.cashflows.type), {"FixedRateCoupon", "Redemption", "Disbursement"})
            coupons = result.coupons[result.coupons.type == "FixedRateCoupon"]
            self.assertEqual(len(coupons), 2)  # Pay and receive directions remain separate.
            for rate in coupons.rate:
                self.assertAlmostEqual(rate, 0.04, places=14)
            self.assertAlmostEqual(coupons.notional.sum(), 150_000.0)
            omitted = context.evaluate(portfolio, ["FairRate(0)"])
            self.assertIsNone(omitted.fair_rate)
            self.assertIsNone(omitted.price)

    def test_portfolio_omits_incomplete_measures_and_keeps_cashflow_currencies_separate(self):
        class NumericPricer:
            def evaluate(self, trade, requests, context):
                if trade == "missing":
                    return qs.EvaluationResults(context.evaluation_date, trade)
                return qs.EvaluationResults(context.evaluation_date, trade, price=10.0, cashflows=[
                    dict(payment_date="2027-01-15", type="FixedRateCoupon", amount=3.0, currency="USD"),
                    dict(payment_date="2027-01-15", type="Redemption", amount=100.0, currency="EUR"),
                ])

        context = self.make_context()
        context.register_pricer(str, NumericPricer())
        with context:
            result = context.evaluate(qs.Portfolio("PORTFOLIO", ["first", "missing"]), ["Value", "Cashflows"])
            self.assertIsNone(result.price)
            self.assertIsNone(result.cashflows)
            result = context.evaluate(qs.Portfolio("PORTFOLIO", ["first", "second"]), ["Cashflows"])
        summary = result.cashflow_summary.set_index("currency")
        self.assertEqual(summary.loc["USD", "interest"], 6.0)
        self.assertEqual(summary.loc["USD", "capital"], 0.0)
        self.assertEqual(summary.loc["EUR", "capital"], 200.0)
        with self.assertRaises(qs.QuantSupportError):
            qs.Portfolio("PORTFOLIO", [])
        with self.assertRaisesRegex(qs.QuantSupportError, "identifier"):
            qs.Portfolio("  ", ["first"])

    def test_context_portfolio_defaults_requests_and_lifecycle(self):
        instrument = qs.FixedRateDeposit("DEPOSIT", "2026-03-02", "2027-03-02", 100_000, 0.04, "USD", "SOFR")
        trade = qs.Trade(instrument, "2026-02-24")
        portfolio = qs.Portfolio("PORTFOLIO", [trade, trade])
        self.assertFalse(hasattr(portfolio, "evaluate"))
        context = self.make_context()
        with self.assertRaisesRegex(qs.QuantSupportError, "not initialized"):
            context.evaluate(portfolio)
        with context:
            single = context.evaluate(trade)
            default = context.evaluate(portfolio)
            self.assertAlmostEqual(default.price, 2 * single.price, places=7)
            self.assertIsNone(default.sensitivities)
            self.assertIsNone(default.cashflows)
            empty = context.evaluate(portfolio, [])
            self.assertIsNone(empty.price)
            self.assertIsNone(empty.cashflows)
            risk_only = context.evaluate(portfolio, ["Sensitivities", "Sensitivities"])
            single_risk = context.evaluate(trade, ["Sensitivities"])
            self.assertIsNone(risk_only.price)
            expected = dict(single_risk.raw_sensitivities)
            for pillar, value in risk_only.raw_sensitivities:
                self.assertAlmostEqual(value, 2 * expected[pillar], places=6)
        with self.assertRaisesRegex(qs.QuantSupportError, "not initialized"):
            context.evaluate(portfolio)

    def test_portfolio_rejects_futures_quotes_as_monetary_values(self):
        futures = qs.Trade(qs.RateFutures(
            "FUTURE", "2027-02-24", "2027-05-24", 96.0, "SOFR",
        ), "2026-02-24")
        deposit = qs.Trade(qs.FixedRateDeposit(
            "DEPOSIT", "2026-02-24", "2027-02-24", 100_000, 0.04, "USD", "SOFR",
        ), "2026-02-24")
        with self.make_context() as context:
            self.assertIsNotNone(context.evaluate(futures).price)
            for requests in [["Value"], ["Sensitivities"]]:
                with self.assertRaisesRegex(qs.QuantSupportError, "rate futures return quotes"):
                    context.evaluate(qs.Portfolio("PORTFOLIO", [deposit, futures]), requests)

    def test_context_converts_members_to_base_currency_before_aggregation(self):
        date = "2026-02-24"
        curves = [
            qs.CurveConfiguration(
                market_index=index,
                quotes=[f"ZeroRate_{currency}_{index}_{tenor}" for tenor in ["0D", "5Y"]],
            )
            for currency, index in [("USD", "SOFR"), ("EUR", "ESTR")]
        ]
        fx = qs.FxStore()
        fx.add("USD", "EUR", 0.9)
        quotes = qs.QuoteStore.from_dict({"reference_date": date, "quotes": [
            {"identifier": identifier, "mid": 0.03}
            for curve in curves for identifier in curve.to_dict()["quotes"]
        ]})
        context = qs.PricingContext(
            quotes, curves, fx=fx,
            csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR"),
        )
        usd_context = qs.PricingContext(
            quotes, curves, fx=fx,
            csa_policy=qs.SingleCurveCSADiscountPolicy("USD", "SOFR"),
        )
        eur = qs.Trade(qs.FixedRateDeposit("EUR", "2026-03-02", "2027-03-02", 100_000, 0.04, "EUR", "ESTR"), date)
        usd = qs.Trade(qs.FixedRateDeposit("USD", "2026-03-02", "2027-03-02", 100_000, 0.04, "USD", "SOFR"), date)
        with context:
            single = context.evaluate(eur, ["Value", "Sensitivities"])
            result = context.evaluate(qs.Portfolio("PORTFOLIO", [eur, eur]), ["Value", "Sensitivities"])
            self.assertAlmostEqual(result.price, 2 * single.price, places=7)
            converted = context.evaluate(usd, ["Value", "Sensitivities", "Cashflows"])
            mixed = context.evaluate(qs.Portfolio("PORTFOLIO", [eur, usd]), ["Value", "Sensitivities", "Cashflows"])
            self.assertAlmostEqual(mixed.price, single.price + converted.price, places=7)
            payments = context.evaluate(qs.Portfolio("PORTFOLIO", [eur, usd]), ["Cashflows"])
            self.assertEqual(set(payments.cashflow_summary.currency), {"EUR", "USD"})
            risk_only = context.evaluate(usd, ["Sensitivities"])
            self.assertIsNone(risk_only.price)
            self.assertEqual(dict(risk_only.raw_sensitivities), dict(converted.raw_sensitivities))
        with usd_context:
            local = usd_context.evaluate(usd, ["Value", "Sensitivities"])
        self.assertAlmostEqual(converted.price, 0.9 * local.price, places=7)
        self.assertAlmostEqual(dict(converted.raw_sensitivities)["USD/EUR"], local.price, places=7)
        for pillar, value in local.raw_sensitivities:
            self.assertAlmostEqual(dict(converted.raw_sensitivities)[pillar], 0.9 * value, places=6)
        with usd_context:
            inverse = usd_context.evaluate(eur, ["Value", "Sensitivities"])
        self.assertAlmostEqual(inverse.price, single.price / 0.9, places=7)
        self.assertAlmostEqual(dict(inverse.raw_sensitivities)["USD/EUR"], -single.price / 0.9**2, places=6)
        triangulated = qs.FxStore()
        triangulated.add("USD", "GBP", 0.8)
        triangulated.add("GBP", "EUR", 1.125)
        context.fx = triangulated
        with context:
            via_gbp = context.evaluate(usd, ["Value", "Sensitivities"])
        self.assertAlmostEqual(via_gbp.price, converted.price, places=7)
        fx_risk = dict(via_gbp.raw_sensitivities)
        self.assertAlmostEqual(fx_risk["USD/GBP"], 1.125 * local.price, places=7)
        self.assertAlmostEqual(fx_risk["GBP/EUR"], 0.8 * local.price, places=7)
        bumped_values = []
        for shift in [1e-5, -1e-5]:
            shifted_fx = qs.FxStore()
            shifted_fx.add("USD", "GBP", 0.8 + shift)
            shifted_fx.add("GBP", "EUR", 1.125)
            context.fx = shifted_fx
            with context:
                bumped_values.append(context.evaluate(usd).price)
        self.assertAlmostEqual(
            (bumped_values[0] - bumped_values[1]) / 2e-5,
            fx_risk["USD/GBP"], places=5,
        )
        context.fx = qs.FxStore()
        with context:
            with self.assertRaisesRegex(qs.QuantSupportError, "exchange rate path"):
                context.evaluate(qs.Portfolio("PORTFOLIO", [eur, usd]))

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

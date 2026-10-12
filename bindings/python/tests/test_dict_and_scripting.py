"""Dictionary and native scripting contracts; also exercised by cargo test."""

import json
import math
from pathlib import Path
import tempfile
import unittest

import quantsupport as qs


class DictAndScriptTests(unittest.TestCase):
    def test_public_type_hints_are_packaged_and_cover_native_classes(self):
        import ast
        import inspect
        module_file = getattr(qs, "__file__", None)
        if module_file:
            stub = Path(module_file).with_name("__init__.pyi")
            self.assertTrue(stub.with_name("py.typed").exists())
        else:
            # The Rust contract runner creates the module in memory.
            stub = Path(__file__).resolve().parents[1] / "python" / "quantsupport" / "__init__.pyi"
        declarations = {node.name: node for node in ast.parse(stub.read_text()).body
                        if isinstance(node, ast.ClassDef)}
        exported = {name for name, value in vars(qs).items()
                    if inspect.isclass(value) and value.__module__ == "quantsupport"}
        self.assertEqual(exported, set(declarations))
        for product in ["EuropeanSwaption", "EquityForward"]:
            methods = {node.name: node for node in declarations[product].body
                       if isinstance(node, ast.FunctionDef)}
            self.assertIn("__new__", methods)
            self.assertEqual(methods["from_dict"].decorator_list[0].id, "classmethod")

    def test_calendars_and_calibration_diagnostics(self):
        london = qs.Calendar("UnitedKingdom(Settlement)")
        japan = qs.Calendar("Japan")
        joint = qs.Calendar("JointCalendar(JoinHolidays,UnitedKingdom(Settlement),Japan)")
        self.assertFalse(london.is_business_day("2023-05-08"))
        self.assertFalse(japan.is_business_day("2021-08-09"))
        self.assertEqual(str(joint.adjust("2019-04-26")), "2019-04-26")
        self.assertEqual(str(joint.adjust("2019-04-27")), "2019-05-07")
        self.assertEqual(joint.name(), "JointCalendar(JoinHolidays,UnitedKingdom(Settlement),Japan)")
        data = Path(__file__).resolve().parents[3] / "examples" / "hullwhite" / "data"
        quotes = qs.QuoteStore.from_json(str(data / "quotes.json"))
        curves = qs.CurveConfiguration.from_json(str(data / "curve_specs.json"))
        surfaces = qs.VolatilitySurfaceConfiguration.from_json(str(data / "vol_specs.json"))
        simulations = qs.SimulationConfiguration.from_json(str(data / "simulation.json"))
        with qs.PricingContext(quotes, curves, volatility_surfaces=surfaces,
                              simulations=simulations) as context:
            report = context.simulation("SOFR").calibration_diagnostics()
            self.assertGreater(len(report), 0)
            self.assertTrue((report["calibrated_sigma"] > 0).all())
            self.assertLess(report["price_error"].abs().max(), 1e-6)
            self.assertTrue((report["price_error"] == report["model_price"] - report["market_price"]).all())
        fixed = qs.SimulationConfiguration.from_dict({"market_index": "SOFR",
            "model": {"HullWhite": {"alpha": 0.1, "parameter_source": {"Fixed": {"sigma": 0.01}}}},
            "n_paths": 2, "seed": 1, "horizon": "1Y", "frequency": "Annual"})
        with qs.PricingContext(quotes, curves, simulations=[fixed]) as context:
            self.assertIsNone(context.simulation("SOFR").calibration_diagnostics())

    def test_sparse_volatility_grids_and_upper_cube_boundaries(self):
        surface_nodes = [
            ("6M", "0.02", 0.20), ("6M", "0.04", 0.30),
            ("1Y", "0.025", 0.22), ("1Y", "0.045", 0.34),
        ]
        records = [{
            "identifier": f"CapletFloorlet_USD_SOFR_3M_{expiry}_Absolute_{strike}_Straddle_Black",
            "mid": value,
        } for expiry, strike, value in surface_nodes]
        surface_ids = [record["identifier"] for record in records]
        cube_records = [{
            "identifier": f"Swaption_USD_SOFR_{expiry}_{tenor}_Absolute_{strike}_Black",
            "mid": 0.20 + 0.02 * years + 0.01 * tenor_years + 0.001 * strike,
        } for expiry, years in [("6M", 0.5), ("1Y", 1.0)]
            for tenor, tenor_years in [("1Y", 1.0), ("5Y", 5.0)]
            for strike in [0.02, 0.05]]
        quotes = qs.QuoteStore.from_dict({
            "reference_date": "2025-01-01", "quotes": records + cube_records,
        })
        for policy in ["Reject", "Flat"]:
            surface_config = qs.VolatilitySurfaceConfiguration.from_dict({
                "market_index": "SOFR", "quotes": surface_ids,
                "extrapolation_policy": policy,
            })
            cube_config = qs.VolatilityCubeConfiguration.from_dict({
                "market_index": "SOFR", "quotes": [record["identifier"] for record in cube_records],
                "extrapolation_policy": policy,
            })
            self.assertEqual(surface_config.to_dict()["extrapolation_policy"], policy)
            self.assertEqual(cube_config.to_dict()["extrapolation_policy"], policy)
            with qs.PricingContext(quotes, [], volatility_surfaces=[surface_config],
                                  volatility_cubes=[cube_config]) as context:
                surface = context.volatility_surface("SOFR")
                cube = context.volatility_cube("SOFR")
                self.assertAlmostEqual(surface.volatility("9M", 0.03), 0.25)
                self.assertAlmostEqual(surface.volatility("1Y", 0.045), 0.34)
                self.assertAlmostEqual(cube.volatility("1Y", "5Y", 0.05), 0.27005)
                if policy == "Reject":
                    with self.assertRaises(qs.QuantSupportError):
                        surface.volatility("2Y", 0.06)
                    with self.assertRaises(qs.QuantSupportError):
                        cube.volatility("2Y", "10Y", 0.06)
                else:
                    self.assertAlmostEqual(surface.volatility("2Y", 0.06), 0.34)
                    self.assertAlmostEqual(cube.volatility("2Y", "10Y", 0.06), 0.27005)

    def test_single_quote_volatility_and_default_extrapolation_policy(self):
        surface_id = "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.03_Straddle_Black"
        cube_id = "Swaption_USD_SOFR_1Y_5Y_Absolute_0.03_Black"
        quotes = qs.QuoteStore.from_dict({"reference_date": "2025-01-01", "quotes": [
            {"identifier": surface_id, "mid": 0.20},
            {"identifier": cube_id, "mid": 0.20},
        ]})
        for extra, expected_policy in [({}, "Reject"), ({"extrapolation_policy": "Flat"}, "Flat")]:
            surface_config = qs.VolatilitySurfaceConfiguration.from_dict({
                "market_index": "SOFR", "quotes": [surface_id], **extra,
            })
            cube_config = qs.VolatilityCubeConfiguration.from_dict({
                "market_index": "SOFR", "quotes": [cube_id], **extra,
            })
            self.assertEqual(surface_config.to_dict()["extrapolation_policy"], expected_policy)
            with qs.PricingContext(quotes, [], volatility_surfaces=[surface_config],
                                  volatility_cubes=[cube_config]) as context:
                surface = context.volatility_surface("SOFR")
                cube = context.volatility_cube("SOFR")
                self.assertAlmostEqual(surface.volatility("1Y", 0.03), 0.20)
                self.assertAlmostEqual(cube.volatility("1Y", "5Y", 0.03), 0.20)
                if expected_policy == "Flat":
                    self.assertAlmostEqual(surface.volatility("2Y", 0.04), 0.20)
                    self.assertAlmostEqual(cube.volatility("2Y", "10Y", 0.04), 0.20)
                else:
                    with self.assertRaises(qs.QuantSupportError):
                        surface.volatility("2Y", 0.04)
                    with self.assertRaises(qs.QuantSupportError):
                        cube.volatility("2Y", "10Y", 0.04)

    def test_python_enums_and_string_dicts_remain_equivalent(self):
        specification = self.stepped_spec()
        enums = dict(specification, currency=qs.Currency.USD,
                     payment_frequency=qs.Frequency.Annual,
                     coupon_day_counter=qs.DayCounter.Actual365,
                     start_date=qs.Date(2026, 2, 24))
        enums["coupons"] = [qs.Coupon.fixed(0.04), {
            "FloatingRateCoupon": {"index": qs.MarketIndex.SOFR, "spread": 0.005,
                                   "floor": 0.0, "cap": 0.06},
        }]
        self.assertEqual(qs.SteppedCouponBond.from_dict(enums).to_dict(),
                         qs.SteppedCouponBond.from_dict(specification).to_dict())
        self.assertEqual(qs.SteppedCouponBond(**enums).to_dict(),
                         qs.SteppedCouponBond.from_dict(specification).to_dict())
        self.assertEqual(qs.Coupon.from_dict({"FixedRateCoupon": {"rate": 0.04}}).to_dict(),
                         qs.Coupon.fixed(0.04).to_dict())
        curve = {"market_index": qs.MarketIndex.SOFR, "quotes": []}
        self.assertEqual(qs.CurveConfiguration.from_dict(
            curve).to_dict()["market_index"], "SOFR")
        surface = {"market_index": qs.MarketIndex.SOFR, "quotes": [],
                   "volatility_type": qs.VolatilityType.Black, "smile_type": qs.SmileType.Strike}
        self.assertEqual(qs.VolatilitySurfaceConfiguration.from_dict(
            surface).to_dict()["volatility_type"], "Black")
        self.assertIs(surface["volatility_type"], qs.VolatilityType.Black)
        with self.make_context() as context:
            bond = qs.FixedRateBond.from_dict({"identifier": "ENUM-CHECK", "start_date": "2026-02-24",
                                               "maturity_date": "2028-02-24", "notional": 100.0, "rate": 0.04,
                                               "currency": qs.Currency.USD})
            trade = self.bond_trade(bond)
            enum_value = context.evaluate(trade, [qs.Request.Value]).price
            string_value = context.evaluate(trade, ["Value"]).price
            self.assertEqual(enum_value, string_value)

    def stepped_spec(self):
        return {
            "identifier": "STEP",
            "start_date": "2026-02-24",
            "maturity_date": "2030-02-24",
            "notional": 100.0,
            "currency": "USD",
            "payment_frequency": "Annual",
            "coupon_day_counter": "Actual365",
            "coupons": [
                {"Fixed": {"rate": 0.04}},
                {"Floating": {"index": "SOFR", "spread": 0.005,
                              "floor": 0.0, "cap": 0.06}},
            ],
            "notional_percentages": [1.0, 0.8],
            "call_schedule": [
                {"date": "2028-02-24", "strike": 100.0},
                {"start_date": "2029-02-24", "end_date": "2029-08-24",
                 "strike": 99.0},
            ],
        }

    def bond_trade(self, instrument, anchor=None):
        if anchor is None:
            anchor = (qs.BondPricingAnchor.OAS(0.0) if isinstance(instrument, qs.SteppedCouponBond)
                      else qs.BondPricingAnchor.z_spread(0.0))
        return qs.Trade(instrument, "2026-02-24", pricing_anchor=anchor)

    def make_context(self, models=None, with_volatility=False, engine="TreeBased", quote_level=None):
        data = Path(__file__).resolve(
        ).parents[3] / "examples" / "hullwhite" / "data"
        records = json.loads((data / "quotes.json").read_text())
        if quote_level is not None:
            for quote in records["quotes"]:
                quote[str(quote_level).lower()] = quote.pop("mid")
        quotes = qs.QuoteStore.from_dict(records)
        curves = qs.CurveConfiguration.from_dict_list(
            json.loads((data / "curve_specs.json").read_text())
        )
        volatility = (
            qs.VolatilitySurfaceConfiguration.from_dict_list(
                json.loads((data / "vol_specs.json").read_text())
            ) if with_volatility else None
        )
        pricers = None if models is None else [qs.PricerConfiguration.from_dict({
            "instrument_type": "SteppedCouponBond",
            "pricer": {engine: {"time_grid": {}}},
            "models": models,
        })]
        return qs.PricingContext(
            quotes=quotes, curves=curves, volatility_surfaces=volatility,
            csa_policy=qs.SingleCurveCSADiscountPolicy.from_dict(
                {"collateral_currency": "USD", "collateral_index": "SOFR"}),
            pricer_configurations=pricers,
            quote_level=quote_level,
        )

    def test_quote_level_setup_accepts_enums_and_strings(self):
        quotes = qs.QuoteStore.from_dict({"reference_date": "2026-02-24", "quotes": [
            {"identifier": "FixedRateDeposit_USD_SOFR_1D",
                "mid": 0.04, "bid": 0.03, "ask": 0.05},
            {"identifier": "OIS_USD_SOFR_5Y", "mid": 0.04, "bid": 0.03, "ask": 0.05},
        ]})
        curves = [qs.CurveConfiguration.from_dict({"market_index": "SOFR", "quotes": [
            "FixedRateDeposit_USD_SOFR_1D", "OIS_USD_SOFR_5Y",
        ]})]
        discounts = []
        for level, expected in [("bId", qs.Level.Bid), (None, qs.Level.Mid), (qs.Level.Ask, qs.Level.Ask)]:
            with qs.PricingContext(quotes, curves, quote_level=level) as context:
                self.assertEqual(context.quote_level, expected)
                discounts.append(context.curve(
                    "SOFR").discount_factor("2030-02-24"))
        self.assertGreater(discounts[0], discounts[1])
        self.assertGreater(discounts[1], discounts[2])
        self.assertEqual(qs.Level.parse("ASK"), qs.Level.Ask)
        for invalid in ["last", 123]:
            with self.subTest(invalid=invalid), self.assertRaises(qs.QuantSupportError):
                qs.PricingContext(quotes, curves, quote_level=invalid)
        mid_only = qs.QuoteStore.from_dict({"reference_date": "2026-02-24", "quotes": [
            {"identifier": "FixedRateDeposit_USD_SOFR_1D", "mid": 0.04},
            {"identifier": "OIS_USD_SOFR_5Y", "mid": 0.04},
        ]})
        with self.assertRaisesRegex(qs.QuantSupportError, "No bid quote available"):
            qs.PricingContext(mid_only, curves, quote_level="Bid").initialize()

    def test_callable_bond_calibration_uses_selected_quote_level(self):
        model = {"market_index": "SOFR", "model": {"HullWhite": {
            "alpha": 0.05, "parameter_source": {"Calibrated": {
                "source": {"Surface": {"market_index": "SOFR"}},
                "calibration_basket": {"expiries": ["1Y", "2Y"], "strike": "Atm"},
            }},
        }}}
        for engine in ["TreeBased", "BackwardFourierEvolution"]:
            prices = []
            for level in [None, qs.Level.Bid, qs.Level.Ask]:
                with self.subTest(engine=engine, level=level):
                    with self.make_context(models=[model], with_volatility=True,
                                           engine=engine, quote_level=level) as context:
                        bond = qs.SteppedCouponBond.from_dict(
                            self.stepped_spec())
                        prices.append(context.evaluate(
                            self.bond_trade(bond)).price)
            self.assertAlmostEqual(prices[0], prices[1], places=10)
            self.assertAlmostEqual(prices[0], prices[2], places=10)

    def rate_model(self, sigma=0.0, family="Lgm"):
        parameter = "lambda" if family == "Lgm" else "alpha"
        return {"market_index": "SOFR", "model": {family: {
            parameter: 0.05, "parameter_source": {"Fixed": {"sigma": sigma}},
        }}}

    def script_spec(self, models=None):
        spec = {
            "name": "NOTE", "currency": "USD", "result_variable": "note",
            "events": [{"date": "2026-02-24",
                        "code": 'note = 0; note pays 100 on "2028-02-24";'}],
        }
        if models is not None:
            spec["models"] = models
        return spec

    def test_all_product_dict_constructors_accept_strings(self):
        dated = {"identifier": "TEST", "start_date": "2026-02-24",
                 "maturity_date": "2028-02-24", "notional": 100.0, "currency": "USD"}
        xccy = {"identifier": "TEST", "start_date": "2026-02-24",
                "maturity_date": "2028-02-24", "domestic_notional": 100.0,
                "foreign_notional": 90.0, "domestic_currency": "USD",
                "foreign_currency": "EUR"}
        cases = [
            (qs.Swap, dict(dated, fixed_rate=0.04, market_index="SOFR",
                           day_counter="Actual360", compounding="Simple",
                           rate_frequency="Annual", fixed_leg_frequency="Semiannual")),
            (qs.CrossCurrencySwap, dict(xccy, domestic_market_index="SOFR",
                                        foreign_market_index="ESTR")),
            (qs.BasisSwap, dict(dated, pay_market_index="SOFR",
             receive_market_index="TermSOFR3m")),
            (qs.FixFloatCrossCurrencySwap, dict(
                xccy, fixed_rate=0.04, floating_index="ESTR")),
            (qs.FixedRateBond, dict(dated, rate=0.04, payment_structure="Bullet")),
            (qs.SteppedCouponBond, self.stepped_spec()),
            (qs.FloatingRateNote, dict(dated, spread=0.005, forward_index="SOFR")),
            (qs.FixedRateDeposit, dict(dated, rate=0.04)),
            (qs.FxForward, {"identifier": "TEST", "delivery_date": "2028-02-24",
                            "base_currency": "EUR", "quote_currency": "USD",
                            "forward_price": 1.1}),
            (qs.FxOption, {"identifier": "TEST", "expiry_date": "2028-02-24",
                           "strike": 1.1, "option_type": "Call", "base_currency": "EUR",
                           "quote_currency": "USD"}),
            (qs.EquityOption, {"identifier": "TEST", "expiry_date": "2028-02-24",
                               "strike": {"Absolute": 100.0}, "option_type": "Put",
                               "market_index": {"Equity": "AAPL"}}),
            (qs.CreditDefaultSwap, dict({key: value for key, value in dated.items() if key != "notional"},
                                        spread=0.01, credit_index={"Credit": "CORPORATE-A"}, discount_index="SOFR")),
            (qs.CapFloor, dict(dated, strike=0.04,
             market_index="SOFR", cap_floor_type="Cap")),
            (qs.CapletFloorlet, {"identifier": "TEST", "fixing_date": "2027-02-24",
                                 "start_accrual_date": "2027-02-24",
                                 "end_accrual_date": "2027-05-24", "payment_date": "2027-05-24",
                                 "strike": 0.04, "currency": "USD",
                                 "market_index": "SOFR", "payoff_type": "Caplet"}),
            (qs.EquityForward, {"identifier": "TEST", "market_index": "SOFR", "delivery_date": "2028-02-24", "strike": 100.0, "currency": "USD"}),
            (qs.EuropeanSwaption, {"identifier": "TEST", "expiry_date": "2027-02-24", "maturity_date": "2032-02-24", "strike": 0.04, "notional": 100.0, "currency": "USD", "market_index": "SOFR"}),
            (qs.RateFutures, {"identifier": "TEST", "start_date": "2027-02-24",
                              "end_date": "2027-05-24", "futures_price": 96.0,
                              "market_index": "SOFR"}),
        ]
        for product_type, spec in cases:
            with self.subTest(product=product_type.__name__):
                product = product_type.from_dict(spec)
                self.assertIsInstance(product, product_type)
                self.assertIsInstance(product, qs.Instrument)
                self.assertFalse(hasattr(product, "side"))
                self.assertFalse(hasattr(product, "trade_date"))
                self.assertEqual(product.identifier, spec["identifier"])
                self.assertEqual(repr(product), repr(product_type(**spec)))
                with tempfile.TemporaryDirectory() as directory:
                    path = Path(directory) / "product.json"
                    path.write_text(json.dumps(spec))
                    self.assertEqual(
                        repr(product_type.from_json(str(path))), repr(product))

    def test_from_dict_retains_subclass_constructor(self):
        class DeskBond(qs.FixedRateBond):
            pass

        bond = DeskBond.from_dict({"identifier": "DESK", "start_date": "2026-02-24",
                                  "maturity_date": "2028-02-24", "notional": 100.0,
                                   "rate": 0.04, "currency": "USD"})
        self.assertIsInstance(bond, DeskBond)

    def test_stepped_nested_roundtrip_and_object_constructor(self):
        bond = qs.SteppedCouponBond.from_dict(self.stepped_spec())
        spec = bond.to_dict()
        self.assertNotIn("discount_index", spec)
        self.assertEqual(qs.SteppedCouponBond.from_dict(spec).to_dict(), spec)
        self.assertEqual(spec["call_schedule"][0]["start_date"], "2028-02-24")
        spec["coupons"] = [qs.Coupon.from_dict(
            coupon) for coupon in spec["coupons"]]
        spec["call_schedule"] = [qs.CallScheduleEntry.from_dict(
            call) for call in spec["call_schedule"]]
        self.assertEqual(qs.SteppedCouponBond(
            **spec).to_dict(), bond.to_dict())
        with self.assertRaises(TypeError):
            qs.SteppedCouponBond.from_dict(
                dict(self.stepped_spec(), discount_index="SOFR"))

    def test_call_window_is_inclusive_and_discrete_calls_remain(self):
        call = qs.CallScheduleEntry(
            start_date="2028-01-01", end_date="2028-06-30", strike=99.0)
        for date in ["2028-01-01", "2028-03-01", "2028-06-30"]:
            self.assertTrue(call.contains(date))
        self.assertFalse(call.contains("2027-12-31"))
        self.assertFalse(call.contains("2028-07-01"))
        self.assertEqual(str(call.start_date), str(call.date))
        self.assertEqual(str(call.end_date), "2028-06-30")
        self.assertEqual(qs.CallScheduleEntry.window(
            "2028-01-01", "2028-06-30", 99).to_dict(), call.to_dict())
        discrete = qs.CallScheduleEntry("2028-01-01", 100)
        self.assertEqual(str(discrete.date), str(discrete.end_date))
        self.assertTrue(discrete.contains("2028-01-01"))
        self.assertFalse(discrete.contains("2028-01-02"))

    def test_invalid_call_windows_and_coupon_bounds(self):
        invalid = [
            {"start_date": "2028-06-30", "end_date": "2028-01-01", "strike": 100.0},
            {"date": "2028-01-01", "strike": -1.0},
        ]
        for spec in invalid:
            with self.subTest(spec=spec), self.assertRaises(qs.QuantSupportError):
                qs.CallScheduleEntry.from_dict(spec)
        with self.assertRaises(qs.QuantSupportError):
            qs.CallScheduleEntry.window("2028-06-30", "2028-01-01", 100)
        with self.assertRaises(ValueError):
            qs.CallScheduleEntry(date="2028-01-01", start_date="2028-01-01")
        with self.assertRaises(qs.QuantSupportError):
            qs.Coupon.from_dict({"Floating": {"index": "SOFR", "spread": 0.0,
                                              "floor": 0.06, "cap": 0.02}})

    def test_bond_anchor_is_required_and_owned_by_trade(self):
        spec = {"identifier": "ANCHORED", "start_date": "2026-02-24", "maturity_date": "2028-02-24",
                "notional": 100.0, "rate": 0.04, "currency": "USD"}
        instrument = qs.FixedRateBond(**spec)
        with self.assertRaises(TypeError):
            qs.FixedRateBond(
                **spec, pricing_anchor=qs.BondPricingAnchor.dirty_price(98.0))
        long = self.bond_trade(
            instrument, qs.BondPricingAnchor.dirty_price(98.0))
        short = qs.Trade(instrument, "2026-02-24",
                         side="PayShort", pricing_anchor=long.pricing_anchor)
        self.assertEqual(long.pricing_anchor.to_dict(), {"DirtyPrice": 98.0})
        self.assertEqual(qs.FixedRateBond.from_dict(
            instrument.to_dict()).to_dict(), instrument.to_dict())
        with self.make_context() as ctx:
            with self.assertRaisesRegex(ValueError, "pricing_anchor"):
                ctx.evaluate(qs.Trade(instrument, "2026-02-24"))
            result = ctx.evaluate(long, ["Value", "ZSpread", "Sensitivities"])
            short_result = ctx.evaluate(short, ["Value", "ZSpread"])
            repriced = ctx.evaluate(long.with_pricing_anchor(
                qs.BondPricingAnchor.z_spread(result.z_spread)))
            self.assertAlmostEqual(
                result.price, -short_result.price, places=10)
            self.assertAlmostEqual(
                result.z_spread, short_result.z_spread, places=10)
            self.assertAlmostEqual(repriced.price, result.price, places=9)
            with self.assertRaises(TypeError):
                ctx.evaluate(
                    long, bond_anchor=qs.BondPricingAnchor.z_spread(0.0))
        for anchor in [{"DirtyPrice": -1.0}, {"ZSpread": float("nan")}]:
            with self.assertRaises(qs.QuantSupportError):
                qs.Trade(instrument, "2026-02-24", pricing_anchor=anchor)

    def test_stepped_schedule_controls_reach_native_schedule_builder(self):
        spec = dict(self.stepped_spec(), start_date="2026-02-28", maturity_date="2027-02-28",
                    coupons=[{"Fixed": {"rate": 0.04}}], notional_percentages=[1.0], call_schedule=[],
                    payment_frequency="Semiannual",
                    calendar="WeekendsOnly", business_day_convention="Following",
                    termination_date_convention="Preceding", date_generation_rule="Forward",
                    end_of_month=False, first_coupon_date="2026-05-31", next_to_last_coupon_date="2026-11-30")
        bond = qs.SteppedCouponBond(**spec)
        native_objects = dict(spec, calendar=qs.Calendar("WeekendsOnly"),
                              business_day_convention=qs.BusinessDayConvention.Following,
                              termination_date_convention=qs.BusinessDayConvention.Preceding,
                              date_generation_rule=qs.DateGenerationRule.Forward)
        self.assertEqual(qs.SteppedCouponBond(
            **native_objects).to_dict(), bond.to_dict())
        self.assertEqual(qs.SteppedCouponBond.from_dict(
            bond.to_dict()).to_dict(), bond.to_dict())
        with self.make_context(models=[self.rate_model(family="HullWhite")]) as ctx:
            table = ctx.evaluate(self.bond_trade(
                bond, qs.BondPricingAnchor.z_spread(0.0)), ["Cashflows"]).cashflows
        dates = table.loc[table["type"] ==
                          "FixedRateCoupon", "payment_date"].tolist()
        self.assertEqual(dates, ["2026-06-01", "2026-11-30", "2027-02-26"])

    def test_csa_policy_and_fixed_income_policy_are_explicit(self):
        policy = qs.SingleCurveCSADiscountPolicy(
            collateral_currency="USD", collateral_index="SOFR")
        self.assertEqual(qs.SingleCurveCSADiscountPolicy.from_dict(
            policy.to_dict()).to_dict(), policy.to_dict())
        with self.make_context() as ctx:
            self.assertEqual(ctx.csa_policy.to_dict(), policy.to_dict())
        with self.assertRaises(qs.QuantSupportError):
            qs.SingleCurveCSADiscountPolicy.from_dict(
                {"currency": "USD", "index": "SOFR"})
        spec = dict(identifier="SELECTED", start_date="2026-02-24", maturity_date="2028-02-24",
                    notional=100, rate=0.04, currency="USD", discount_index={"Other": "UNAVAILABLE"})
        inputs = self.make_context()
        with qs.PricingContext(quotes=inputs.quotes, curves=inputs.curve_configurations,
                               csa_policy=policy,
                               fixed_income_policy=qs.FixedIncomeDiscountPolicy({"USD": "SOFR"}, prefer_instrument_index=False)) as ctx:
            result = ctx.evaluate(self.bond_trade(qs.FixedRateBond(**spec)))
            self.assertTrue(math.isfinite(result.price))

    def test_market_config_lists_and_fx_are_in_memory(self):
        spec = {"market_index": "SOFR", "quotes": []}
        for input_value in [spec, [spec], {"curve_specs": [spec]}]:
            curves = qs.CurveConfiguration.from_dict_list(input_value)
            self.assertEqual(len(curves), 1)
            self.assertEqual(
                curves[0].to_dict(), qs.CurveConfiguration.from_dict(spec).to_dict())
        fx = qs.FxStore.from_dict(
            [{"base": "EUR", "quote": "USD", "rate": 1.1}])
        self.assertAlmostEqual(fx.rate("USD", "EUR"), 1 / 1.1)
        configs = [
            (qs.VolatilitySurfaceConfiguration, {"market_index": "SOFR", "volatility_type": "Black",
                                                 "smile_type": "Strike", "quotes": []}),
            (qs.VolatilityCubeConfiguration, {"market_index": "SOFR", "volatility_type": "Black",
                                              "smile_type": "Strike", "quotes": []}),
            (qs.PricerConfiguration, {"instrument_type": "SteppedCouponBond",
                                      "pricer": {"TreeBased": {"time_grid": {}}},
                                      "models": [self.rate_model()]}),
            (qs.SimulationConfiguration, dict(
                self.rate_model(), horizon="2Y", n_paths=2)),
        ]
        for config_type, config in configs:
            with self.subTest(config=config_type.__name__):
                self.assertEqual(config_type.from_dict_list([config])[0].to_dict(),
                                 config_type.from_dict(config).to_dict())

    def test_fixed_bond_uses_context_discount_policy(self):
        spec = {"identifier": "BOND", "start_date": "2026-02-24", "maturity_date": "2028-02-24",
                "notional": 100.0, "rate": 0.04, "currency": "USD"}
        with self.make_context() as context:
            implicit = context.evaluate(self.bond_trade(
                qs.FixedRateBond.from_dict(spec))).price
            explicit = context.evaluate(self.bond_trade(
                qs.FixedRateBond.from_dict(dict(spec, discount_index="SOFR")))).price
        self.assertAlmostEqual(implicit, explicit, places=10)

    def test_script_zero_volatility_matches_context_curve(self):
        spec = self.script_spec([self.rate_model()])
        product = qs.ScriptedProduct.from_dict(spec)
        self.assertEqual(qs.ScriptedProduct.from_dict(
            product.to_dict()).to_dict(), product.to_dict())
        with self.make_context() as context:
            expected = 100 * \
                context.curve("SOFR").discount_factor("2028-02-24")
            result = context.evaluate_script(product, n_paths=2, seed=7)
            self.assertAlmostEqual(result["note"], expected, places=8)
            self.assertEqual(result, context.evaluate_script(
                spec, n_paths=2, seed=7))
            explicit = dict(spec, discount_index="SOFR")
            self.assertEqual(result, context.evaluate_script(
                explicit, n_paths=2, seed=7))

    def test_script_inherits_context_hull_white_model(self):
        with self.make_context(models=[self.rate_model(family="HullWhite")]) as context:
            result = context.evaluate_script(self.script_spec(), n_paths=2)
            expected = 100 * \
                context.curve("SOFR").discount_factor("2028-02-24")
            self.assertAlmostEqual(result["note"], expected, places=8)

    def test_stepped_native_engines_and_script_share_discount_policy(self):
        bond_spec = self.stepped_spec()
        bond_spec.update(maturity_date="2028-02-24", coupons=[{"Fixed": {"rate": 0.04}}],
                         notional_percentages=[1.0], call_schedule=[])
        script = self.script_spec()
        script["events"][0]["code"] = (
            'note = 0; note pays 4 * cvg("2026-02-24", "2027-02-24", "Actual365") '
            'on "2027-02-24"; note pays 100 + 4 * '
            'cvg("2027-02-24", "2028-02-24", "Actual365") on "2028-02-24";'
        )
        for engine in ["TreeBased", "BackwardFourierEvolution"]:
            with self.subTest(engine=engine):
                with self.make_context(models=[self.rate_model(family="HullWhite")], engine=engine) as context:
                    native = context.evaluate(self.bond_trade(qs.SteppedCouponBond.from_dict(
                        bond_spec), qs.BondPricingAnchor.z_spread(0.0))).price
                    scripted = context.evaluate_script(
                        script, n_paths=2)["note"]
                    self.assertAlmostEqual(native, scripted, places=8)

    def test_script_seed_is_repeatable(self):
        spec = self.script_spec([self.rate_model(sigma=0.01)])
        spec["events"] = [{"date": "2027-02-24", "code": (
            'note = 0; note pays max(RateIndex("SOFR", "2027-02-24", "2028-02-24")'
            ' - 0.04, 0) * 10000 on "2028-02-24";'
        )}]
        with self.make_context() as context:
            first = context.evaluate_script(spec, n_paths=32, seed=91)
            second = context.evaluate_script(spec, n_paths=32, seed=91)
            other = context.evaluate_script(spec, n_paths=32, seed=92)
        self.assertEqual(first, second)
        self.assertNotEqual(first["note"], other["note"])
        self.assertTrue(math.isfinite(first["note"]))

    def test_script_calibrates_against_context_surface(self):
        model = {"market_index": "SOFR", "model": {"Lgm": {
            "lambda": 0.05, "parameter_source": {"Calibrated": {
                "source": {"Surface": {"market_index": "SOFR"}},
                "calibration_basket": {"expiries": ["1Y", "2Y"], "strike": "Atm"},
            }},
        }}}
        spec = self.script_spec([model])
        prices = []
        for level in [None, qs.Level.Bid, qs.Level.Ask]:
            with self.subTest(level=level):
                with self.make_context(with_volatility=True, quote_level=level) as context:
                    first = context.evaluate_script(spec, n_paths=4, seed=42)
                    self.assertTrue(math.isfinite(first["note"]))
                    self.assertEqual(first, context.evaluate_script(
                        spec, n_paths=4, seed=42))
                    prices.append(first["note"])
        self.assertEqual(prices[0], prices[1])
        self.assertEqual(prices[0], prices[2])

    def test_script_errors_are_explicit(self):
        spec = self.script_spec([self.rate_model()])
        context = self.make_context()
        with self.assertRaises(qs.QuantSupportError):
            context.evaluate_script(spec)
        with context:
            with self.assertRaises(ValueError):
                context.evaluate_script(spec, n_paths=0)
            with self.assertRaisesRegex(qs.QuantSupportError, "no rate model configured"):
                context.evaluate_script(self.script_spec())
            with self.assertRaisesRegex(qs.QuantSupportError, "Not found error: result variable 'unknown'"):
                context.evaluate_script(
                    dict(spec, result_variable="unknown"), n_paths=2)
            with self.assertRaises(qs.QuantSupportError):
                context.evaluate_script(
                    dict(spec, events=[{"date": "2025-01-01", "code": "note = 1;"}]))
        with self.assertRaises(ValueError):
            qs.ScriptedProduct.from_dict(dict(spec, events=[]))
        with self.assertRaisesRegex(qs.QuantSupportError, "parsing script event dated 2027-01-01: Parsing error:"):
            qs.ScriptedProduct.from_dict(
                dict(spec, events=[{"date": "2027-01-01", "code": "???"}]))
        for code, message in [
                ('note = "unfinished;', "Unterminated string literal"),
                ("note = 1.2.3;", "Invalid numeric literal '1.2.3'")]:
            with self.subTest(code=code):
                with self.assertRaisesRegex(qs.QuantSupportError, message):
                    qs.ScriptedProduct.from_dict(
                        dict(spec, events=[{"date": "2027-01-01", "code": code}]))


if __name__ == "__main__":
    unittest.main()

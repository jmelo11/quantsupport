"""Native credit configuration loading, bootstrapping, and Python pricing."""

import json
import math
from pathlib import Path
import tempfile
import unittest

import quantsupport as qs


class CreditCurveConfigurationTests(unittest.TestCase):
    reference_date = "2026-02-24"
    cds_quotes = [f"Cds_CLIENT-A_USD_{tenor}" for tenor in ["1Y", "3Y", "5Y"]]

    def specification(self):
        return {
            "market_index": {"Credit": "CLIENT-A"},
            "currency": "USD",
            "discount_index": "SOFR",
            "recovery": 0.4,
            "quotes": list(self.cds_quotes),
        }

    def market(self):
        rate_quotes = ["FixedRateDeposit_USD_SOFR_1D", "OIS_USD_SOFR_5Y"]
        quotes = qs.QuoteStore.from_dict({
            "reference_date": self.reference_date,
            "quotes": [
                {"identifier": identifier, "mid": spread,
                    "bid": spread, "ask": spread}
                for identifier, spread in zip(
                    rate_quotes +
                    self.cds_quotes, [0.04, 0.04, 0.01, 0.012, 0.015]
                )
            ],
        })
        curves = [qs.CurveConfiguration.from_dict({
            "market_index": "SOFR", "quotes": rate_quotes,
        })]
        return quotes, curves

    def make_context(self, scenarios=None, specification=None):
        quotes, curves = self.market()
        configuration = qs.CreditCurveConfiguration.from_dict(
            self.specification() if specification is None else specification
        )
        return qs.PricingContext(
            quotes, curves, scenarios=scenarios,
            csa_policy=qs.SingleCurveCSADiscountPolicy("USD", "SOFR"),
            credit_curves=[configuration],
        )

    def test_native_defaults_and_explicit_fields_round_trip(self):
        expected = dict(self.specification(), day_counter="Actual360",
                        premium_frequency="Quarterly", interpolator="LogLinear",
                        enable_extrapolation=True)
        configuration = qs.CreditCurveConfiguration.from_dict(
            self.specification())
        self.assertEqual(configuration.to_dict(), expected)
        self.assertIn("CreditCurveConfiguration", repr(configuration))
        explicit = dict(expected, day_counter="Actual365", premium_frequency="Semiannual",
                        enable_extrapolation=False)
        self.assertEqual(qs.CreditCurveConfiguration.from_dict(
            explicit).to_dict(), explicit)

    def test_list_loaders_accept_native_and_compatible_shapes(self):
        specification = self.specification()
        expected = qs.CreditCurveConfiguration.from_dict(
            specification).to_dict()
        shapes = [specification, [specification], {
            "credit_curve_specs": [specification]}]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "credit_curves.json"
            for shape in shapes:
                with self.subTest(shape=shape):
                    path.write_text(json.dumps(shape), encoding="utf-8")
                    for configurations in [
                        qs.CreditCurveConfiguration.from_dict_list(shape),
                        qs.CreditCurveConfiguration.from_json(str(path)),
                    ]:
                        self.assertEqual(len(configurations), 1)
                        self.assertEqual(configurations[0].to_dict(), expected)

    def test_invalid_configuration_uses_library_error(self):
        specification = self.specification()
        del specification["currency"]
        with self.assertRaisesRegex(qs.QuantSupportError, "currency"):
            qs.CreditCurveConfiguration.from_dict(specification)

    def test_optional_credit_curves_preserve_existing_positional_arguments(self):
        quotes, curves = self.market()
        contexts = [qs.PricingContext(quotes, curves),
                    qs.PricingContext(quotes, curves, credit_curves=None),
                    qs.PricingContext(quotes, curves, credit_curves=[])]
        for context in contexts:
            self.assertEqual(context.credit_curve_configurations, [])
            with context:
                self.assertGreater(context.curve(
                    "SOFR").discount_factor("2027-02-24"), 0.0)
        with qs.PricingContext(
            quotes, curves, None, None, None, None, None, None,
            qs.SingleCurveCSADiscountPolicy(
                "USD", "SOFR"), None, None, qs.Level.Bid,
        ) as context:
            self.assertEqual(context.quote_level, qs.Level.Bid)
            self.assertEqual(context.credit_curve_configurations, [])

    def test_bootstrapped_credit_curve_prices_cds_with_quote_sensitivity(self):
        instrument = qs.CreditDefaultSwap(
            "CLIENT-A-CDS", self.reference_date, "2031-02-24", 0.01,
            "USD", {"Credit": "CLIENT-A"}, "SOFR",
        )
        trade = qs.Trade(instrument, self.reference_date, notional=1_000_000)
        with self.make_context() as context:
            expected = qs.CreditCurveConfiguration.from_dict(
                self.specification()).to_dict()
            self.assertEqual(
                context.credit_curve_configurations[0].to_dict(), expected)
            result = context.evaluate(
                trade, [qs.Request.Value, qs.Request.Sensitivities])
            self.assertTrue(math.isfinite(result.price))
            self.assertGreater(abs(result.price), 1.0)
            risks = result.sensitivities.set_index("pillar")["value"]
            self.assertTrue(
                all(identifier in risks.index for identifier in self.cds_quotes))
            sensitivity = float(risks[self.cds_quotes[-1]])
            self.assertGreater(abs(sensitivity), 1.0)
        bump = 1e-5
        prices = []
        for shock in [bump, -bump]:
            with self.make_context([qs.Scenario(self.cds_quotes[-1], shock)]) as context:
                prices.append(context.evaluate(trade).price)
        finite_difference = (prices[0] - prices[1]) / (2 * bump)
        self.assertAlmostEqual(sensitivity, finite_difference,
                               delta=max(0.1, abs(finite_difference) * 0.01))

    def test_missing_credit_quote_fails_during_bootstrap(self):
        missing = "Cds_CLIENT-A_USD_10Y"
        specification = dict(self.specification(), quotes=[missing])
        with self.assertRaisesRegex(qs.QuantSupportError, missing):
            with self.make_context(specification=specification):
                pass

    def test_xva_credit_curve_reports_cds_quote_sensitivity(self):
        swap = qs.Swap(
            "CLIENT-A-SWAP", self.reference_date, "2031-02-24", 1_000_000,
            0.02, "USD", "SOFR",
        )
        trade = qs.Trade(swap, self.reference_date, qs.Side.PayShort)
        csa = qs.CsaTerms("SOFR", "USD", 0.01, 0.4,
                          credit_index={"Credit": "CLIENT-A"})
        netting_set = qs.NettingSet("CLIENT-A", [trade], csa)
        config = qs.XvaConfig.from_dict({
            "model_configs": [{"market_index": "SOFR", "lambda": 0.05,
                               "parameter_source": {"Fixed": {"sigma": 0.01}}}],
            "n_paths": 8, "seed": 42, "frequency": "Annual",
        })

        def run(scenarios=None):
            with self.make_context(scenarios) as context:
                result = context.run_xva(config, [netting_set])
                values = result.xva_values
                cva = float(values.loc[(values["netting_set"] == "CLIENT-A")
                                       & (values["measure"] == "CVA"), "value"].iloc[0])
                risks = result.sensitivities.set_index("parameter")["value"]
                return cva, risks

        cva, risks = run()
        self.assertTrue(math.isfinite(cva))
        self.assertGreater(abs(cva), 1.0)
        labels = [
            f"CLIENT-A.CVA.{identifier}" for identifier in self.cds_quotes]
        self.assertTrue(all(label in risks.index for label in labels))
        self.assertNotIn("CLIENT-A.CVA.credit_spread", risks.index)
        sensitivity = float(risks[labels[-1]])
        self.assertTrue(math.isfinite(sensitivity))
        self.assertGreater(abs(sensitivity), 1.0)
        bump = 1e-5
        up, _ = run([qs.Scenario(self.cds_quotes[-1], bump)])
        down, _ = run([qs.Scenario(self.cds_quotes[-1], -bump)])
        self.assertTrue(math.isfinite(up) and math.isfinite(down))
        self.assertGreater(abs(up - down), 0.01)


if __name__ == "__main__":
    unittest.main()

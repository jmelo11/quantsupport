import json
import tempfile
import unittest
from pathlib import Path

import quantsupport as qs


class CsaVariationMarginTests(unittest.TestCase):
    def specification(self):
        return {
            "collateral_index": "SOFR",
            "collateral_currency": "USD",
            "credit_spread": 0.01,
            "recovery": 0.4,
        }

    def test_legacy_defaults_leave_variation_margin_disabled(self):
        for terms in [qs.CsaTerms(**self.specification()),
                      qs.CsaTerms.from_dict(self.specification())]:
            self.assertFalse(terms.variation_margin)
            self.assertEqual(terms.counterparty_threshold, 0.0)
            self.assertEqual(terms.own_threshold, 0.0)
            self.assertEqual(terms.minimum_transfer_amount, 0.0)

    def test_constructor_and_loaders_expose_margin_terms(self):
        specification = dict(self.specification(), variation_margin=True,
                             counterparty_threshold=100_000.0,
                             own_threshold=200_000.0,
                             minimum_transfer_amount=5_000.0)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "csa.json"
            path.write_text(json.dumps(specification))
            loaded = qs.CsaTerms.from_dict(json.loads(Path(path).read_text(encoding="utf-8")))
        for terms in [qs.CsaTerms(**specification),
                      qs.CsaTerms.from_dict(specification), loaded]:
            self.assertTrue(terms.variation_margin)
            self.assertEqual(terms.counterparty_threshold, 100_000.0)
            self.assertEqual(terms.own_threshold, 200_000.0)
            self.assertEqual(terms.minimum_transfer_amount, 5_000.0)

    def test_invalid_margin_amounts_are_rejected(self):
        for name in ["counterparty_threshold", "own_threshold",
                     "minimum_transfer_amount"]:
            for invalid in [-1.0, float("nan"), float("inf"), -float("inf")]:
                for enabled in [False, True]:
                    specification = dict(self.specification(),
                                         variation_margin=enabled,
                                         **{name: invalid})
                    with self.subTest(name=name, invalid=invalid, enabled=enabled):
                        with self.assertRaises(qs.QuantSupportError):
                            qs.CsaTerms(**specification)
                        with self.assertRaises(qs.QuantSupportError):
                            qs.CsaTerms.from_dict(specification)


if __name__ == "__main__":
    unittest.main()

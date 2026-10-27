"""Consistent calling, conversion, and display of public Python objects."""

import inspect
import json
from pathlib import Path
import tempfile
import unittest

import quantsupport as qs


class PythonApiTests(unittest.TestCase):
    def loan(self, coupons):
        return qs.Loan("LOAN", "2026-02-24", "2028-02-24", 100_000, coupons, "EUR")

    def test_public_parameters_accept_positional_and_keyword_arguments(self):
        checked = 0
        for name in qs.__all__:
            cls = getattr(qs, name)
            if not isinstance(cls, type) or cls.__module__ != "quantsupport":
                continue
            self.assertFalse(hasattr(cls, "from_json"), name)
            methods = [(name, cls)]
            for method_name, method in vars(cls).items():
                if method_name.startswith("_"):
                    continue
                if isinstance(method, (classmethod, staticmethod)):
                    method = getattr(cls, method_name)
                if callable(method):
                    methods.append((f"{name}.{method_name}", method))
            for label, method in methods:
                try:
                    signature = inspect.signature(method)
                except (ValueError, TypeError):
                    continue
                for parameter in signature.parameters.values():
                    if parameter.name in ("self", "cls"):
                        continue
                    with self.subTest(method=label, parameter=parameter.name):
                        self.assertNotIn(parameter.kind, (parameter.KEYWORD_ONLY, parameter.POSITIONAL_ONLY))
                        checked += 1
        self.assertGreater(checked, 300)

    def test_positional_keyword_and_mixed_construction_agree(self):
        rate = qs.InterestRate(0.04, "Actual365", "Compounded", "Semiannual")
        self.assertEqual(dict(rate), dict(qs.InterestRate(
            rate=0.04, day_counter="Actual365", compounding="Compounded", frequency="Semiannual")))
        coupon = qs.FixedRateCoupon(rate)
        args = ("LOAN", "2026-02-24", "2028-02-24", 100_000, [coupon], "EUR",
                qs.LoanRepaymentStructure.bullet(), "Quarterly", "Annual")
        positional = qs.Loan(*args)
        keyword = qs.Loan(identifier="LOAN", start_date="2026-02-24", maturity_date="2028-02-24",
                          notional=100_000, coupons=[coupon], currency="EUR",
                          repayment=qs.LoanRepaymentStructure.bullet(), payment_frequency="Quarterly",
                          fixing_frequency="Annual")
        self.assertEqual(dict(positional), dict(keyword))
        self.assertEqual(dict(qs.FloatingRateCoupon("EURIBOR12m", 0.01)),
                         dict(qs.FloatingRateCoupon(index="EURIBOR12m", spread=0.01)))
        payoff = qs.PayoffOps.Index.max(0.0)
        self.assertEqual(dict(qs.OptionEmbeddedCoupon("EURIBOR12m", payoff, spread=0.01)),
                         dict(qs.OptionEmbeddedCoupon(index="EURIBOR12m", payoff=payoff, spread=0.01)))
        self.assertEqual(dict(qs.CurveConfiguration("SOFR", [], "Actual365", "Linear", False)),
                         dict(qs.CurveConfiguration(market_index="SOFR", quotes=[], day_counter="Actual365",
                                                    interpolator="Linear", enable_extrapolation=False)))
        self.assertEqual(qs.Calendar.TARGET.business_days_between("2026-02-24", "2026-03-24"),
                         qs.Calendar.TARGET.business_days_between(from_="2026-02-24", to="2026-03-24"))
        self.assertEqual(rate.discount_factor("2026-02-24", "2027-02-24"),
                         rate.discount_factor(start="2026-02-24", end="2027-02-24"))

    def test_coupon_lists_accept_objects_dicts_tuples_and_mixed_entries(self):
        fixed = qs.FixedRateCoupon(0.04)
        floating = qs.FloatingRateCoupon("EURIBOR12m", 0.01)
        for coupons in ([fixed], [fixed.to_dict()], (fixed,), [fixed, fixed.to_dict()],
                        [fixed, floating], [fixed.to_dict(), floating.to_dict()]):
            with self.subTest(coupons=coupons):
                loan = self.loan(coupons)
                self.assertEqual(qs.Loan.from_dict(dict(loan)).to_dict(), dict(loan))

    def test_coupon_errors_locate_the_bad_entry_and_terms(self):
        fixed = qs.FixedRateCoupon(0.04)
        for coupons, message in [
            ([[fixed]], r"Loan.coupons\[0\].*expected dict, got list"),
            ([fixed, [fixed]], r"Loan.coupons\[1\].*got list"),
            ([{"FixedRateCoupon": [0.04]}], r"Loan.coupons\[0\].FixedRateCoupon.*got list"),
            ([0.04], r"Loan.coupons\[0\].*got float"),
            (fixed, r"Loan.coupons:.*expected a flat sequence"),
        ]:
            with self.subTest(coupons=coupons), self.assertRaisesRegex(TypeError, message):
                self.loan(coupons)
        with self.assertRaisesRegex(qs.QuantSupportError, r"Loan.coupons\[1\].*finite"):
            self.loan([fixed, {"Fixed": {"rate": float("nan")}}])

    def test_dict_and_json_errors_name_the_expected_input(self):
        with self.assertRaisesRegex(TypeError, r"Loan.from_dict:.*expected dict, got list"):
            qs.Loan.from_dict([])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "loan.json"
            path.write_text(json.dumps([]))
            with self.assertRaisesRegex(TypeError, r"Loan.from_dict:.*expected dict, got list"):
                qs.Loan.from_dict(json.loads(Path(path).read_text(encoding="utf-8")))

    def test_class_reprs_and_dictionary_conversion_are_consistent(self):
        objects = [qs.InterestRate(0.04), qs.FixedRateCoupon(0.04), qs.FloatingRateCoupon("SOFR"),
                   qs.LoanRepaymentStructure.bullet(), qs.LoanRepaymentStructure.balloon(0.25),
                   qs.PrepaymentModelConfiguration.no_prepayment(), qs.PayoffOps.Index,
                   qs.PayoffOps.Const(0.04), qs.CurveConfiguration("SOFR"),
                   qs.BondPricingAnchor.clean_price(99), qs.FixedIncomeDiscountPolicy({"EUR": "ESTR"}),
                   qs.MarketSnapshotStore(), self.loan([qs.FixedRateCoupon(0.04)])]
        for obj in objects:
            with self.subTest(cls=type(obj).__name__):
                self.assertIsInstance(obj.to_dict(), dict)
                self.assertEqual(dict(obj), obj.to_dict())
                self.assertTrue(repr(obj).startswith(type(obj).__name__ + "("), repr(obj))
                self.assertEqual(str(obj), repr(obj))
                self.assertNotIn('object at 0x', repr(obj))
                with self.assertRaises(KeyError):
                    obj["missing_field"]
        self.assertEqual(qs.LoanRepaymentStructure.bullet().to_dict(), {"Bullet": None})
        self.assertEqual(qs.PrepaymentModelConfiguration.no_prepayment().to_dict(), {"None": None})
        self.assertEqual(qs.PayoffOps.Index.to_dict(), {"Index": None})
        self.assertIn("enable_extrapolation=True", repr(qs.CurveConfiguration("SOFR")))
        self.assertIn("rate=", repr(qs.FixedRateCoupon(0.04)))
        loan = self.loan([qs.FixedRateCoupon(0.04)])
        copied = dict(loan)
        copied["coupons"].clear()
        self.assertEqual(len(loan.to_dict()["coupons"]), 1)

        class DeskLoan(qs.Loan):
            pass

        self.assertTrue(repr(DeskLoan.from_dict(dict(loan))).startswith("DeskLoan("))

    def test_public_classes_have_class_representations(self):
        for name in qs.__all__:
            cls = getattr(qs, name)
            if isinstance(cls, type) and cls.__module__ == "quantsupport":
                with self.subTest(cls=name):
                    self.assertIsNot(cls.__repr__, object.__repr__)

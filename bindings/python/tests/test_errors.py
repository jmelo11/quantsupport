"""The Python error contract preserves native categories across API domains."""

import inspect
import json
from pathlib import Path
import tempfile
import unittest

import quantsupport as qs
from quantsupport import quantsupport as native


class StructuredErrorTests(unittest.TestCase):
    def capture(self, function):
        with self.assertRaises(qs.QuantSupportError) as caught:
            function()
        error = caught.exception
        payload = error.to_dict()
        self.assertEqual(set(payload), {
            "code", "native_type", "message", "operation", "path", "expected", "actual",
            "contexts", "details", "cause",
        })
        self.assertEqual(payload["code"], error.code)
        self.assertEqual(payload["message"], error.message)
        self.assertTrue(error.code)
        self.assertTrue(error.native_type)
        self.assertIsInstance(error.contexts, tuple)
        self.assertIsInstance(payload["contexts"], list)
        json.dumps(payload)
        self.assertIn(f"[{error.code}]", str(error))
        return error

    def test_error_types_are_identical_in_public_and_native_interfaces(self):
        for name in ["QuantSupportError", "QuantSupportTypeError", "QuantSupportValueError",
                     "QuantSupportStateError", "QuantSupportIOError", "QuantSupportNotImplementedError"]:
            self.assertIs(getattr(qs, name), getattr(native, name))
            self.assertTrue(issubclass(getattr(qs, name), qs.QuantSupportError))

    def test_reinstalling_error_boundaries_preserves_classmethods(self):
        original = qs.Loan("L", "2026-02-24", "2028-02-24", 1000,
                           [qs.FixedRateCoupon(.04)], "EUR")
        for _ in range(2):
            native._install_error_boundaries(vars(native))
            restored = qs.Loan.from_dict(original.to_dict())
            self.assertEqual(restored.to_dict(), original.to_dict())
            self.assertEqual(list(inspect.signature(qs.Loan.from_dict).parameters), ["obj"])

    def test_coupon_failure_has_structured_path_and_preserves_cast_cause(self):
        error = self.capture(lambda: qs.Loan("L", "2026-02-24", "2028-02-24", 100_000,
                                           [qs.FixedRateCoupon(0.04), {"FixedRateCoupon": []}], "EUR"))
        self.assertIsInstance(error, TypeError)
        self.assertEqual(error.code, "invalid_type")
        self.assertEqual(error.native_type, "InvalidTypeErr")
        self.assertEqual(error.operation, "Loan")
        self.assertEqual(error.path, "coupons[1].FixedRateCoupon")
        self.assertEqual(error.expected, "dict")
        self.assertEqual(error.actual, "list")
        cause = error
        while cause.__cause__ is not None:
            cause = cause.__cause__
        self.assertIsInstance(cause, TypeError)
        self.assertNotIsInstance(cause, qs.QuantSupportError)

    def test_deserialization_path_identifies_the_nested_market_input(self):
        error = self.capture(lambda: qs.QuoteStore.from_dict({
            "reference_date": "2026-02-24", "quotes": [
                {"identifier": "ZeroRate_EUR_ESTR_1Y", "mid": 0.04},
                {"identifier": "ZeroRate_EUR_ESTR_2Y", "mid": "wrong"},
            ],
        }))
        self.assertEqual(error.code, "deserialization")
        self.assertEqual(error.native_type, "DeserializationErr")
        self.assertEqual(error.operation, "QuoteStore.from_dict")
        self.assertEqual(error.path, "quotes[1].mid")
        self.assertEqual(error.actual, "str")
        self.assertIsInstance(error, ValueError)
        self.assertIsNotNone(error.__cause__)

    def test_native_date_category_and_cause_are_preserved(self):
        error = self.capture(lambda: qs.Date.parse("not-a-date"))
        self.assertEqual(error.code, "date_parsing")
        self.assertEqual(error.native_type, "DateParsingErr")
        self.assertEqual(error.operation, "Date.parse")
        self.assertIsInstance(error, ValueError)
        self.assertIn("Date parsing error:", error.message)
        self.assertIsNotNone(error.__cause__)

    def test_input_validation_is_consistent_across_constructors(self):
        for function, operation in [
            (lambda: qs.InterestRate(float("nan")), "InterestRate"),
            (lambda: qs.Shock("", 0.01, qs.ScenarioType.Absolute), "Shock"),
            (lambda: qs.Trade(qs.Instrument("I"), "2026-02-24", notional=-1), "Trade"),
            (lambda: qs.Loan("L", "2026-02-24", "2028-02-24", -1, [qs.FixedRateCoupon(.04)], "EUR"), "Loan"),
        ]:
            with self.subTest(operation=operation):
                error = self.capture(function)
                self.assertIsInstance(error, ValueError)
                self.assertEqual(error.code, "invalid_value")
                self.assertEqual(error.native_type, "InvalidValueErr")
                self.assertEqual(error.operation, operation)

    def test_type_conversion_uses_native_type_error_category(self):
        error = self.capture(lambda: qs.CurveConfiguration([]))
        self.assertIsInstance(error, TypeError)
        self.assertEqual(error.code, "invalid_type")
        self.assertEqual(error.native_type, "InvalidTypeErr")
        self.assertEqual(error.operation, "CurveConfiguration")
        self.assertEqual(error.path, "market_index")
        self.assertEqual(error.actual, "list")

    def test_constructor_argument_binding_uses_the_shared_contract(self):
        for function, operation in [
            (lambda: qs.InterestRate("wrong"), "InterestRate"),
            (lambda: qs.Loan(), "Loan"),
            (lambda: qs.Loan("L", "2026-02-24", "2028-02-24", "wrong", [qs.FixedRateCoupon(.04)], "EUR"), "Loan"),
            (lambda: qs.Date(year=2026, month=1, day=1, unknown=True), "Date"),
        ]:
            with self.subTest(operation=operation):
                error = self.capture(function)
                self.assertIsInstance(error, TypeError)
                self.assertEqual(error.code, "invalid_type")
                self.assertEqual(error.operation, operation)

    def test_missing_quotes_and_invalid_context_have_distinct_native_categories(self):
        quotes = qs.QuoteStore.from_dict({"reference_date": "2026-02-24", "quotes": []})
        error = self.capture(lambda: quotes.value("MISSING"))
        self.assertEqual((error.code, error.native_type), ("not_found", "NotFoundErr"))
        self.assertEqual(error.operation, "QuoteStore.value")
        context = qs.PricingContext(quotes, [])
        error = self.capture(lambda: context.curve("SOFR"))
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual((error.code, error.native_type), ("evaluation", "EvaluationErr"))
        self.assertEqual(error.operation, "PricingContext.curve")

    def test_file_and_json_failures_keep_location_and_original_cause(self):
        with tempfile.TemporaryDirectory() as directory:
            missing = str(Path(directory) / "missing.json")
            error = self.capture(lambda: qs.QuoteStore.from_json(missing))
            self.assertIsInstance(error, OSError)
            self.assertEqual((error.code, error.native_type), ("io", "IoErr"))
            self.assertEqual(error.details["file"], missing)
            self.assertIsNotNone(error.__cause__)
            path = Path(directory) / "quotes.json"
            path.write_text('{"reference_date": "2026-02-24", "quotes": [}')
            error = self.capture(lambda: qs.QuoteStore.from_json(str(path)))
            self.assertEqual(error.code, "deserialization")
            self.assertEqual(error.details["file"], str(path))
            self.assertGreater(error.details["line"], 0)
            self.assertGreater(error.details["column"], 0)
            path.write_text('{"reference_date": "2026-02-24", "quotes": []} {}')
            self.capture(lambda: qs.QuoteStore.from_json(str(path)))

    def test_python_callback_cause_is_preserved_without_reclassifying_its_message(self):
        class Provider:
            evaluation_date = "2026-02-24"

        original = ValueError("Not found error: callback value is invalid")

        class Pricer:
            def evaluate(self, trade, requests, context):
                raise original

        evaluator = qs.Evaluator()
        evaluator.register(qs.Trade, Pricer())
        trade = qs.Trade(qs.Instrument("I"), "2026-02-24")
        error = self.capture(lambda: evaluator.evaluate(trade, Provider()))
        self.assertEqual((error.code, error.native_type), ("invalid_value", "InvalidValueErr"))
        self.assertIs(error.__cause__, original)

    def test_python_protocol_and_constructor_signatures_remain_valid(self):
        self.assertEqual(list(inspect.signature(qs.Loan).parameters)[:2], ["identifier", "start_date"])
        self.assertEqual(list(inspect.signature(qs.Loan.from_dict).parameters), ["obj"])
        class DeskLoan(qs.Loan):
            pass
        original = qs.Loan("L", "2026-02-24", "2028-02-24", 1000, [qs.FixedRateCoupon(.04)], "EUR")
        self.assertIsInstance(DeskLoan.from_dict(original.to_dict()), DeskLoan)
        with self.assertRaises(KeyError):
            original["missing"]
        self.assertFalse(hasattr(original, "missing"))
        class CustomInstrument(qs.Instrument):
            def __init__(self, identifier, amount):
                super().__init__(identifier)
                self.amount = amount
        self.assertEqual(list(inspect.signature(CustomInstrument).parameters), ["identifier", "amount"])
        self.assertEqual(CustomInstrument("I", 10).amount, 10)
        self.assertEqual(qs.Date.__new__(qs.Date, 2026, 1, 1), qs.Date(2026, 1, 1))
        error = self.capture(lambda: qs.Date.__new__(qs.InterestRate, 2026, 1, 1))
        self.assertEqual(error.operation, "Date")

    def test_cause_serialization_handles_cycles(self):
        error = qs.QuantSupportError("example")
        error.__cause__ = error
        payload = error.to_dict()
        self.assertEqual(payload["cause"]["message"], "cyclic exception cause")
        json.dumps(payload)

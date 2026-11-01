"""The Python error contract preserves native categories across API domains."""

import inspect
import json
from pathlib import Path
import re
import tempfile
import traceback
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
        self.assertEqual(error.contexts, ("FixedRateCoupon coupon terms",))
        self.assertNotIsInstance(error.__cause__, qs.QuantSupportError)
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

    def test_native_contexts_do_not_repeat_as_chained_exceptions(self):
        error = self.capture(lambda: qs.ScriptedProduct.from_dict({
            "name": "NOTE", "currency": "USD", "result_variable": "note",
            "events": [{"date": "2027-01-01", "code": "???"}],
        }))
        self.assertEqual(error.code, "parsing")
        self.assertEqual(error.contexts[0], "parsing script event dated 2027-01-01")
        self.assertIsNone(error.__cause__)
        self.assertIn("parsing script event dated 2027-01-01", str(error))
        self.assertEqual(error.to_dict()["contexts"], list(error.contexts))

    def test_standard_tracebacks_keep_callers_without_generated_boundary_frames(self):
        def user_code(function):
            function()

        for function in [lambda: qs.Date.parse("not-a-date"), lambda: qs.Date.parse(42),
                         lambda: qs.Loan(),
                         lambda: qs.QuoteStore.from_dict({"quotes": "wrong"})]:
            with self.subTest(function=function):
                try:
                    user_code(function)
                except qs.QuantSupportError as error:
                    rendered = "".join(traceback.format_exception(type(error), error, error.__traceback__))
                    self.assertIn("in user_code", rendered)
                    self.assertNotIn('File "<string>"', rendered)
                    self.assertEqual(rendered.count("quantsupport.QuantSupport"), 1)
                    self.assertLessEqual(rendered.count("direct cause"), 1)
                else:
                    self.fail("Expected a structured error")

    def test_notebook_tracebacks_show_one_failure_and_the_user_call_site(self):
        def user_code():
            qs.Date.parse("not-a-date")

        try:
            user_code()
        except qs.QuantSupportError as error:
            lines = error._render_traceback_()
            self.assertEqual(lines[0], "╭─ Traceback (most recent call last):")
            self.assertTrue(lines[-1].startswith("╰"))
            self.assertTrue(all(line.startswith(("│", "├")) for line in lines[1:-1]))
            rendered = "\n".join(lines)
            self.assertIn("Operation Date.parse", rendered)
            self.assertIn("in user_code", rendered)
            self.assertIn('qs.Date.parse("not-a-date")', rendered)
            self.assertLess(rendered.index('qs.Date.parse("not-a-date")'),
                            rendered.index("├─ QuantSupportValueError [date_parsing]"))
            self.assertNotIn('File "<string>"', rendered)
            self.assertNotIn("direct cause", rendered)
            self.assertEqual(rendered.count("QuantSupportValueError"), 1)
            self.assertIsInstance(error.__cause__, ValueError)
            self.assertEqual(error.to_dict()["cause"]["type"], "ValueError")
        else:
            self.fail("Expected a structured error")

    def test_notebook_panel_keeps_multiline_diagnostics_and_structured_fields(self):
        message = "The supplied coupon is invalid.\n" + "A detailed explanation. " * 12
        error = qs.QuantSupportTypeError(message, code="invalid_type", operation="Loan",
                                          path="coupons[1]", expected="dict", actual="list",
                                          contexts=("Reading coupon terms",), native_type="InvalidTypeErr")
        error.__cause__ = TypeError("original conversion failure")
        if hasattr(error, "add_note"):
            error.add_note("Check the source coupon record.")
        original_payload, original_text = error.to_dict(), str(error)
        rendered = "\n".join(error._render_traceback_())
        self.assertIn("Operation Loan", rendered)
        self.assertIn("Input     coupons[1]", rendered)
        self.assertIn("Expected  dict", rendered)
        self.assertIn("Received  list", rendered)
        self.assertIn("Context   Reading coupon terms", rendered)
        # Wrapping may add line breaks, but every word must remain visible.
        content = " ".join(line[3:].strip() for line in rendered.splitlines() if line.startswith("│  "))
        self.assertIn(" ".join(message.split()), content)
        if hasattr(error, "add_note"):
            self.assertIn("Check the source coupon record.", rendered)
        self.assertEqual(error.to_dict(), original_payload)
        self.assertEqual(str(error), original_text)

    def test_notebook_tracebacks_follow_the_active_color_theme(self):
        try:
            from IPython import get_ipython
            from IPython.core.interactiveshell import InteractiveShell
            from IPython.utils.capture import capture_output
            from ipykernel.compiler import XCachingCompiler
        except ImportError:
            self.skipTest("IPython and ipykernel are required for notebook rendering")

        existing_shell = get_ipython()
        shell = existing_shell if existing_shell is not None else InteractiveShell.instance()
        original_colors = shell.colors
        original_compiler = shell.compile
        try:
            shell.compile = XCachingCompiler()
            with capture_output():
                definition = shell.run_cell('''import quantsupport as qs
def make_deposit():
    contract = qs.FixedRateDeposit(
        identifier="TEST",
        start_date=20260101,
        maturity_date="2026-04-01",
        notional=1000,
        rate=0.03,
        currency="EUR",
    )
    return contract
''')
            self.assertIsNone(definition.error_in_exec)
            for colors in ("linux", "lightbg", "nocolor"):
                with self.subTest(colors=colors):
                    shell.run_line_magic("colors", colors)
                    with capture_output() as output:
                        result = shell.run_cell("make_deposit()")
                    rendered = output.stdout + output.stderr
                    plain = re.sub(r"\x1b\[[0-9;]*m", "", rendered)
                    self.assertIsInstance(result.error_in_exec, qs.QuantSupportTypeError)
                    self.assertTrue(plain.startswith("╭─ Traceback (most recent call last):"))
                    self.assertIn("├─ QuantSupportTypeError [invalid_type]", plain)
                    self.assertTrue(all(line.startswith(("│", "├"))
                                        for line in plain.splitlines()[1:-1]))
                    self.assertIn("Operation FixedRateDeposit", plain)
                    self.assertIn("Expected  Date,", plain)
                    self.assertIn("Received  int", plain)
                    self.assertIn("Invalid input type.", plain)
                    self.assertNotIn("Invalid type error: expected", plain)
                    self.assertIn("in make_deposit()", plain)
                    self.assertIn("contract = qs.FixedRateDeposit(", plain)
                    self.assertIn("start_date=20260101,", plain)
                    self.assertIn('currency="EUR",', plain)
                    self.assertLess(plain.index('currency="EUR",'),
                                    plain.index("├─ QuantSupportTypeError [invalid_type]"))
                    self.assertRegex(plain, r"-+> *\d+")
                    self.assertNotIn("direct cause", plain)
                    self.assertNotIn('File "<string>"', plain)
                    self.assertNotIn("interactiveshell.py", plain)
                    self.assertEqual(plain.count("QuantSupportTypeError"), 1)
                    if colors == "nocolor":
                        self.assertNotIn("\x1b[", rendered)
                    else:
                        self.assertIn("\x1b[", rendered.splitlines()[0])
                        self.assertIn("\x1b[", rendered.split("Traceback", 1)[1])
        finally:
            shell.compile = original_compiler
            shell.run_line_magic("colors", original_colors)
            if existing_shell is None:
                InteractiveShell.clear_instance()

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

    def test_snapshot_write_failures_keep_the_native_io_category(self):
        with tempfile.TemporaryDirectory() as directory:
            missing = str(Path(directory) / "missing" / "snapshots.json")
            error = self.capture(lambda: qs.MarketSnapshotStore().to_json(missing))
            self.assertIsInstance(error, OSError)
            self.assertEqual((error.code, error.native_type), ("io", "IoErr"))
            self.assertEqual(error.operation, "MarketSnapshotStore.to_json")

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
        self.assertIn("in evaluate", "".join(traceback.format_tb(original.__traceback__)))

    def test_structured_callback_errors_keep_their_identity_and_original_cause(self):
        class Provider:
            evaluation_date = "2026-02-24"

        original = qs.QuantSupportValueError("bad input", code="invalid_value",
                                             operation="CustomPricer.evaluate",
                                             path="rate", native_type="InvalidValueErr")
        original.__cause__ = ValueError("negative rate")

        class Pricer:
            def evaluate(self, trade, requests, context):
                raise original

        evaluator = qs.Evaluator()
        evaluator.register(qs.Trade, Pricer())
        trade = qs.Trade(qs.Instrument("I"), "2026-02-24")
        error = self.capture(lambda: evaluator.evaluate(trade, Provider()))
        self.assertIs(error, original)
        self.assertEqual(error.operation, "Evaluator.evaluate")
        self.assertEqual(error.path, "rate")
        self.assertEqual(error.contexts, ("CustomPricer.evaluate",))
        self.assertEqual(error.to_dict()["cause"]["message"], "negative rate")

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

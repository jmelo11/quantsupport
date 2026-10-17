"""Regression checks for declarations and complete parameter documentation."""

import ast
from html.parser import HTMLParser
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest

import docstring_parser


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "docs/_ext"))
from excel_api import function_page, result_type, worksheet_type
from python_api import constructor_signature, declarations, method_signature, own_constructor, public_signature
from api_navigation import PYTHON_SECTIONS, excel_section


def documented_parameters(*docs):
    return {parameter.arg_name.lstrip("*"): parameter.description
            for doc in docs if doc
            for parameter in docstring_parser.parse(doc).params}


def parameters(method):
    args = method.args
    names = [argument.arg for argument in (*args.posonlyargs, *args.args, *args.kwonlyargs)]
    if args.vararg:
        names.append(args.vararg.arg)
    if args.kwarg:
        names.append(args.kwarg.arg)
    return set(names) - {"self", "cls"}


class Text(HTMLParser):
    def __init__(self, source):
        super().__init__()
        self.parts = []
        self.ids = set()
        self.feed(source)

    def handle_data(self, data):
        self.parts.append(data)

    def handle_starttag(self, tag, attrs):
        self.ids.update(value for key, value in attrs if key == "id")

    @property
    def text(self):
        return " ".join(" ".join(self.parts).split())


class PythonReferenceTests(unittest.TestCase):
    def test_every_public_parameter_has_a_description(self):
        checked = 0
        for name, (cls, _) in declarations().items():
            if "._" in name:
                continue
            constructor = own_constructor(cls)
            for method in (node for node in cls.body if isinstance(node, ast.FunctionDef)):
                if method.name.startswith("_") and method is not constructor:
                    continue
                if method is constructor:
                    docs = documented_parameters(ast.get_docstring(cls), ast.get_docstring(method))
                else:
                    docs = documented_parameters(ast.get_docstring(method))
                for parameter in parameters(method):
                    with self.subTest(symbol=name, method=method.name, parameter=parameter):
                        self.assertTrue(docs.get(parameter), "Missing parameter description")
                        checked += 1
        self.assertGreaterEqual(checked, 574)

    def test_products_use_own_constructor_not_inherited_initializer(self):
        signature = constructor_signature(SimpleNamespace(id="quantsupport.instruments.Swap"))
        self.assertIn("fixed_rate: float", signature)
        self.assertIn("floating_leg_frequency", signature)
        self.assertNotIn("_args", signature)
        self.assertNotIn("cls", signature)

    def test_private_type_aliases_expand_in_constructors_and_methods(self):
        signature = constructor_signature(SimpleNamespace(id="quantsupport.pricing.Trade"))
        self.assertIn("datetime.date", signature)
        self.assertNotIn("_DateLike", signature)
        method = public_signature("date: _DateLike, index: _IndexLike, *, tenor: _PeriodLike = ...")
        self.assertIn("MarketIndex | str | Mapping[str, Any]", method)
        self.assertIn("Period | str", method)
        self.assertIn("*, tenor", method)

    def test_api_adapter_does_not_mutate_stub_declarations(self):
        cls, _ = declarations()["quantsupport.instruments.Swap"]
        before = ast.dump(cls)
        constructor_signature(SimpleNamespace(id="quantsupport.instruments.Swap"))
        self.assertEqual(ast.dump(cls), before)

    def test_each_public_class_has_one_focused_topic(self):
        assigned = [f"quantsupport.{domain}.{name}"
                    for domain, sections in PYTHON_SECTIONS.items()
                    for names in sections.values() for name in names.split()]
        assigned.append("quantsupport.QuantSupportError")
        self.assertCountEqual(assigned, [name for name in declarations() if "._" not in name])
        self.assertIn("Calendars", PYTHON_SECTIONS["time"])
        self.assertIn("Loans and Prepayment", PYTHON_SECTIONS["instruments"])
        self.assertIn("Volatility", PYTHON_SECTIONS["market"])

    def test_positional_only_method_defaults_are_not_shifted(self):
        signature = method_signature(SimpleNamespace(id="quantsupport.time.Calendar.business_days_between"))
        self.assertIn("from_: Date | str | datetime.date, /, to:", signature)
        self.assertNotIn("datetime.date=True", signature)
        self.assertIn("include_first: bool=True", signature)


class ExcelReferenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        output = subprocess.run(["cargo", "run", "--quiet", "-p", "quantsupport-xll", "--example", "function_catalog"],
                                cwd=ROOT, text=True, capture_output=True, check=True)
        cls.records = json.loads(output.stdout)

    def test_catalog_has_complete_symbol_and_parameter_details(self):
        self.assertEqual(len(self.records), 103)
        for record in self.records:
            with self.subTest(function=record["name"]):
                for field in ("name", "category", "help", "return_type", "returns", "errors", "declaration", "source"):
                    self.assertTrue(record[field])
                self.assertNotIn("See the function description", json.dumps(record))
                for argument in record["arguments"]:
                    self.assertTrue(argument["help"])
                    self.assertTrue(worksheet_type(argument["type"]))

    def test_every_function_renders_structured_fields(self):
        for record in self.records:
            page = function_page(record)
            with self.subTest(function=record["name"]):
                self.assertIn(f".. excel:function:: {record['name']}", page)
                self.assertIn(":returns:", page)
                self.assertIn(":errors:", page)
                for argument in record["arguments"]:
                    self.assertIn(f":param {argument['name']}:", page)

    def test_ranges_are_not_automatically_optional(self):
        self.assertEqual(worksheet_type("* const XLOPER12"), "cell or range")
        self.assertEqual(worksheet_type("Option < f64 >"), "number or blank")
        for record in self.records:
            self.assertNotIn("XLOPER12", result_type(record))

    def test_simulation_vectors_are_spilled_ranges(self):
        path = next(record for record in self.records if record["name"] == "QS.SIMULATION.PATH")
        self.assertEqual(result_type(path), "spilled range")

    def test_each_function_has_domain_and_subsection(self):
        for record in self.records:
            domain, subsection = excel_section(record["name"])
            self.assertTrue(domain)
            self.assertTrue(subsection)
        self.assertEqual(excel_section("QS.CALENDAR.HOLIDAYS"), ("Time", "Calendars"))
        self.assertEqual(excel_section("QS.TRADE.FIXED.BOND"), ("Instruments", "Fixed Income"))


class RenderedReferenceTests(unittest.TestCase):
    def test_strict_build_renders_real_signatures_and_fields(self):
        with tempfile.TemporaryDirectory() as temporary:
            result = subprocess.run([sys.executable, "-m", "sphinx", "-q", "-E", "-a", "-W", "--keep-going",
                                     "-b", "html", "docs", temporary], cwd=ROOT, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            output = Path(temporary)
            swap = Text((output / "autoapi/quantsupport/instruments/Swap.html").read_text())
            self.assertIn("fixed_rate", swap.text)
            self.assertIn("default None uses Semiannual", swap.text)
            self.assertRegex(swap.text, r"Parameters\s*:")
            self.assertNotIn(":classmethod:", swap.text)
            self.assertNotIn("Subclass positional construction arguments", swap.text)
            self.assertNotIn("_DateLike", swap.text)
            self.assertIn("quantsupport.instruments.Swap.from_dict", swap.ids)
            for section in ("Methods", "Properties"):
                self.assertIn(section, swap.text)
            calendars = Text((output / "python/sections/time/calendars.html").read_text())
            for variant in ("NullCalendar", "WeekendsOnly", "TARGET", "UnitedKingdom", "Japan",
                            "Chile", "UnitedStates", "Brazil", "JointCalendar"):
                self.assertIn(variant, calendars.text)
            for path in ("autoapi/quantsupport/time/Schedule.html", "autoapi/quantsupport/time/IMM.html",
                         "excel/functions/section-time-calendars.html", "coverage.html"):
                self.assertTrue((output / path).is_file(), path)
            trade = Text((output / "autoapi/quantsupport/pricing/Trade.html").read_text())
            self.assertIn("datetime.date", trade.text)
            self.assertNotIn("_DateLike", trade.text)
            price = Text((output / "excel/functions/qs-price.html").read_text())
            for field in ("Parameters", "Returns", "Errors"):
                self.assertRegex(price.text, field + r"\s*:")
            for error in ("#N/A", "#VALUE!"):
                self.assertIn(error, price.text)
            self.assertEqual(len(list((output / "excel/functions").glob("qs-*.html"))), 103)
            for index in ("genindex.html", "py-modindex.html", "excel-functionindex.html", "search.html", "objects.inv"):
                self.assertTrue((output / index).is_file(), index)


if __name__ == "__main__":
    unittest.main()
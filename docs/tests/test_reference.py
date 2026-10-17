"""Regression checks for declarations and complete parameter documentation."""

import ast
from html.parser import HTMLParser
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import textwrap
from types import SimpleNamespace
import unittest
from urllib.parse import unquote, urlsplit

import docstring_parser


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "docs/_ext"))
from excel_api import function_page, result_type, worksheet_type
from python_api import (
    class_declaration, construction_note, constructor_signature, declaration_source,
    declarations, method_signature, own_constructor, public_annotation, public_signature,
)
from api_navigation import PYTHON_SECTIONS, excel_section
from notebook_studies import STUDIES, publication_links


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


def guide_snippets():
    snippets = {}
    for path in (ROOT / "docs/python/guide").glob("*.rst"):
        lines = path.read_text().splitlines()
        for index, line in enumerate(lines):
            if line != ".. code-block:: python" or index + 1 >= len(lines):
                continue
            option = lines[index + 1].strip()
            if not option.startswith(":name: guide-python-"):
                continue
            name = option.removeprefix(":name: ")
            body = []
            for content in lines[index + 2:]:
                if content and not content.startswith("   "):
                    break
                body.append(content)
            if name in snippets:
                raise AssertionError(f"Duplicate executable guide block: {name}")
            snippets[name] = textwrap.dedent("\n".join(body))
    return snippets


def formulas(text):
    for match in re.finditer(r"=(QS\.[A-Z0-9.]+)\(", text):
        arguments, current, quoted, depth = [], "", False, 0
        for char in text[match.end():]:
            if char == '"':
                quoted = not quoted
            if not quoted:
                if char in "({":
                    depth += 1
                elif char in ")}":
                    if char == ")" and depth == 0:
                        if current or arguments:
                            arguments.append(current.strip())
                        break
                    depth -= 1
                elif char == "," and depth == 0:
                    arguments.append(current.strip())
                    current = ""
                    continue
            current += char
        else:
            raise AssertionError(f"Unclosed formula: {match[0]}")
        yield match[1], arguments


class Text(HTMLParser):
    def __init__(self, source):
        super().__init__()
        self.parts = []
        self.ids = set()
        self.hrefs = []
        self.images = []
        self.feed(source)

    def handle_data(self, data):
        self.parts.append(data)

    def handle_starttag(self, tag, attrs):
        self.ids.update(value for key, value in attrs if key == "id")
        if tag == "a":
            self.hrefs.extend(value for key, value in attrs if key == "href")
        if tag == "img":
            self.images.extend(value for key, value in attrs if key == "src")

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

    def test_factory_views_return_aliases_and_exception_source(self):
        factory = SimpleNamespace(id="quantsupport.market.CurveConstruction",
                                  short_name="CurveConstruction")
        self.assertEqual(class_declaration(factory), "")
        self.assertIn("factory", construction_note(factory))
        view = SimpleNamespace(id="quantsupport.market.DiscountCurve", short_name="DiscountCurve")
        self.assertIn("active", construction_note(view))
        self.assertEqual(public_annotation("PythonDate | PythonDateTime | _pricing.Trade"),
                         "datetime.date | datetime.datetime | quantsupport.pricing.Trade")
        source = declaration_source(SimpleNamespace(id="quantsupport.QuantSupportError"))
        self.assertIn("/quantsupport/_exceptions.pyi#L", source)

    def test_caplet_relative_strike_describes_its_consumer_not_the_generic_helper(self):
        caplet, _ = declarations()["quantsupport.instruments.CapletFloorlet"]
        strike = documented_parameters(ast.get_docstring(caplet))["strike"]
        self.assertIn("forward * (1 + pct)", strike)
        self.assertIn("additive forward + offset", strike)
        self.assertIn("absolute numeric strike", strike)
        self.assertNotIn("Relative adds an annual decimal spread", strike)

    def test_day_count_and_missing_fixing_restrictions_are_explicit(self):
        day_counter, _ = declarations()["quantsupport.time.DayCounter"]
        doc = " ".join(ast.get_docstring(day_counter).split())
        self.assertIn("endpoint years differ by at least two", doc)
        self.assertIn("US/NASD/BondBasis", doc)
        fixing_store, _ = declarations()["quantsupport.market.FixingStore"]
        self.assertIn("reversed interpolation coordinates", ast.get_docstring(fixing_store))
        self.assertNotIn("fill_missing_fixings", {member.name for member in fixing_store.body
                                                if isinstance(member, ast.FunctionDef)})

    def test_xva_aggregator_pillars_are_not_misrepresented_as_original_quote_risk(self):
        terms, _ = declarations()["quantsupport.risk.CsaTerms"]
        credit = documented_parameters(ast.get_docstring(terms))["credit_index"]
        self.assertIn("not original CDS-spread risk", credit)
        self.assertNotIn("CDS-quote sensitivities", credit)
        result, _ = declarations()["quantsupport.risk.XvaResult"]
        sensitivity = next(member for member in result.body
                           if isinstance(member, ast.FunctionDef) and member.name == "sensitivities")
        doc = ast.get_docstring(sensitivity)
        self.assertIn("independent survival-probability", doc)
        self.assertIn("independent derived funding-spread", doc)
        self.assertIn("not original CDS/funding-quote sensitivities", doc)
        self.assertIn("snapshots held fixed", doc)
        self.assertIn("sigma omits curve/strike", doc)
        guide = (ROOT / "docs/python/guide/risk.rst").read_text()
        self.assertIn("System/base discount factors", guide)
        self.assertIn("Calibrated sigma also omits curve/strike", guide)

    def test_shock_full_identifiers_are_not_documented_as_exact_only(self):
        shock, _ = declarations()["quantsupport.risk.Shock"]
        doc = ast.get_docstring(shock)
        target = documented_parameters(doc)["target"]
        self.assertIn("does not make the match exact-only", target)
        self.assertIn("without deduplication", doc)
        self.assertNotIn("Nonempty exact pillar identifier", target)
        scenario, _ = declarations()["quantsupport.risk.Scenario"]
        self.assertIn("can select more than one quote", ast.get_docstring(scenario))
        guide = (ROOT / "docs/python/guide/risk.rst").read_text()
        self.assertIn("not an exact-only selector", guide)
        self.assertIn("set membership", guide)
        self.assertIn("Inspect every transformed quote", guide)

    def test_evaluate_script_signature_matches_runtime_inputs_and_numeric_results(self):
        context, _ = declarations()["quantsupport.pricing.PricingContext"]
        method = next(member for member in context.body
                      if isinstance(member, ast.FunctionDef) and member.name == "evaluate_script")
        script = next(argument for argument in method.args.args if argument.arg == "script")
        self.assertEqual(ast.unparse(script.annotation), "ScriptedProduct | Mapping[str, Any]")
        self.assertEqual(ast.unparse(method.returns), "dict[str, float]")
        signature = method_signature(SimpleNamespace(id="quantsupport.pricing.PricingContext.evaluate_script"))
        self.assertIn("script: ScriptedProduct | Mapping[str, Any]", signature)
        module = ast.parse((ROOT / "bindings/python/python/quantsupport/pricing/__init__.pyi").read_text())
        self.assertTrue(any(isinstance(node, ast.ImportFrom) and node.level == 2
                            and node.module == "instruments"
                            and any(name.name == "ScriptedProduct" for name in node.names)
                            for node in module.body))
        self.assertNotIn("retained stub annotation", ast.get_docstring(method))


class ExcelReferenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        output = subprocess.run(["cargo", "run", "--quiet", "-p", "quantsupport-xll", "--example", "function_catalog"],
                                cwd=ROOT, text=True, capture_output=True, check=True)
        cls.records = json.loads(output.stdout)

    def test_catalog_has_complete_symbol_and_parameter_details(self):
        self.assertEqual(len(self.records), 109)
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

    def test_interpolation_methods_have_defaults_and_math_navigation(self):
        records = {record["name"]: record for record in self.records
                   if record["name"].startswith("QS.INTERPOLATE")}
        self.assertCountEqual(records, [
            "QS.INTERPOLATE", "QS.INTERPOLATE.LINEAR", "QS.INTERPOLATE.LOG.LINEAR",
            "QS.INTERPOLATE.CUBIC.SPLINE", "QS.INTERPOLATE.BILINEAR", "QS.INTERPOLATE.TRILINEAR",
        ])
        for name, record in records.items():
            with self.subTest(function=name):
                self.assertEqual(excel_section(name), ("Math", "Interpolation"))
                self.assertEqual(record["category"], "QuantSupport - Math")
                self.assertEqual(result_type(record), "number")
                self.assertIn("#VALUE!", record["errors"])
                arguments = {argument["name"]: argument for argument in record["arguments"]}
                policy = arguments.get("extrapolation", arguments.get("enable_extrapolation"))
                self.assertIn("defaults to", policy["help"])
                self.assertNotIn("\\", record["source"])
        method = next(argument for argument in records["QS.INTERPOLATE"]["arguments"]
                      if argument["name"] == "method")
        self.assertIn("defaults to Linear", method["help"])

    def test_guides_use_registered_formulas_and_correct_context_positions(self):
        records = {name: record for record in self.records
                   for name in (record["name"], *record["aliases"])}
        paths = [*(ROOT / "docs/excel/guide").glob("*.rst"),
                 ROOT / "bindings/excel/README.md"]
        contexts = []
        count = 0
        for path in paths:
            for name, arguments in formulas(path.read_text()):
                with self.subTest(path=path.name, formula=name):
                    self.assertIn(name, records)
                    self.assertLessEqual(len(arguments), len(records[name]["arguments"]))
                    count += 1
                    if name == "QS.CONTEXT.CREATE":
                        self.assertEqual(arguments[12:15], ['"USD"', '"SOFR"', '"Mid"'])
                        contexts.append(arguments)
        self.assertGreater(count, 70)
        self.assertGreaterEqual(len(contexts), 6)
        self.assertTrue(any(arguments[8] == "AA2" for arguments in contexts))
        self.assertTrue(any(arguments[11] == "BB2" for arguments in contexts))
        self.assertEqual([a["name"] for a in records["QS.CONTEXT.CREATE"]["arguments"]][12:15],
                         ["base_currency", "base_index", "quote_level"])

    def test_catalog_financial_units_and_range_threading_are_precise(self):
        records = {record["name"]: record for record in self.records}
        futures = records["QS.TRADE.RATE.FUTURE"]
        size = next(argument for argument in futures["arguments"]
                    if argument["name"] == "contract_size")
        self.assertIn("per one futures price point", size["help"])
        self.assertIn("before leg-side multiplication", records["QS.CASHFLOWS"]["returns"])
        self.assertIn("tenor text", records["QS.SIMULATION.CALIBRATION"]["returns"])
        self.assertIn("omits one intervening year", records["QS.YEAR.FRACTION"]["returns"])
        self.assertIn("no unit conversion, interpolation", records["QS.FIXINGS.GET"]["returns"])
        calendar = function_page(records["QS.CALENDAR.HOLIDAYS"])
        self.assertIn("Thread-safe worksheet function", calendar)
        self.assertNotIn("Thread-safe scalar", calendar)
        self.assertIn("Category: **QuantSupport - Time**", calendar)


class GuideAndStudyTests(unittest.TestCase):
    def test_guides_are_local_independent_reading_paths(self):
        for binding, chapters in (
            ("python", ("first-valuation", "markets", "valuation", "risk")),
            ("excel", ("workbook", "markets-and-products", "risk-and-history")),
        ):
            index = (ROOT / f"docs/{binding}/guide/index.rst").read_text()
            hub = (ROOT / f"docs/{binding}/index.rst").read_text()
            self.assertIn("guide/index", hub)
            for chapter in chapters:
                self.assertIn(chapter, index)
                self.assertGreater(len((ROOT / f"docs/{binding}/guide/{chapter}.rst").read_text()), 5000)
        self.assertEqual(len(guide_snippets()), 10)
        for code in guide_snippets().values():
            ast.parse(code)

    def test_every_canonical_notebook_has_a_financial_topic_and_local_page(self):
        entries = [entry for group in STUDIES.values() for entry in group]
        self.assertEqual(len(STUDIES), 8)
        self.assertEqual(len(entries), 14)
        self.assertEqual(len({slug for slug, _ in entries}), 14)
        source_paths = {path for _, path in entries}
        actual = {path.relative_to(ROOT).as_posix()
                  for directory in ("bindings/python/examples", "examples")
                  for path in (ROOT / directory).rglob("*.ipynb")
                  if ".ipynb_checkpoints" not in path.parts}
        self.assertEqual(source_paths, actual)
        index = (ROOT / "docs/python/studies/index.rst").read_text()
        for slug, source in entries:
            self.assertIn(f"notebooks/{slug}", index)
            self.assertTrue((ROOT / source).is_file())
            notebook = json.loads((ROOT / source).read_text())
            provenance = notebook["metadata"].get("execution_environment")
            cells = [cell for cell in notebook["cells"] if cell["cell_type"] == "code"
                     and "".join(cell["source"]).strip()]
            with self.subTest(study=slug):
                self.assertIsInstance(provenance, dict, "Study lacks verified execution provenance")
                self.assertTrue(provenance.get("python"))
                self.assertTrue(provenance.get("quantsupport"))
                self.assertIsInstance(provenance.get("matches_runner"), bool)
                self.assertTrue(cells)
                self.assertTrue(any(cell.get("outputs") for cell in cells), "Study has no saved results")
                self.assertTrue(all(cell.get("execution_count") is not None for cell in cells))
                self.assertFalse(any(result.get("output_type") == "error"
                                     for cell in cells for result in cell.get("outputs", [])))
        self.assertIn('nb_execution_mode = "off"', (ROOT / "docs/conf.py").read_text())

    def test_study_links_are_local_for_notebooks_and_explicit_for_companions(self):
        source = ROOT / "bindings/python/examples/valuation/swaps/tour.ipynb"
        linked = publication_links(
            "[par](par_swap.ipynb#some-section) [inputs](../../bootstrap/curves.ipynb) "
            "[source](tour.ipynb) [external](https://example.com) [section](#local)",
            source, ROOT,
        )
        self.assertIn("(par-swap.ipynb#some-section)", linked)
        self.assertIn("(curves.ipynb)", linked)
        self.assertIn("(first-swap.ipynb)", linked)
        self.assertIn("(https://example.com)", linked)
        self.assertIn("(#local)", linked)
        companion = publication_links("[inputs](market_data.py)", source, ROOT)
        self.assertIn("/blob/main/bindings/python/examples/valuation/swaps/market_data.py", companion)


class GuideExecutionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if importlib.util.find_spec("quantsupport") is None:
            raise unittest.SkipTest("Native binding unavailable; build this checkout to execute guide examples")

    def test_complete_guide_examples_against_the_native_binding(self):
        snippets = guide_snippets()
        standalone = {"guide-python-swap", "guide-python-bootstrap", "guide-python-history"}
        for name, code in snippets.items():
            with self.subTest(example=name):
                foundation = "" if name in standalone else snippets["guide-python-swap"]
                result = subprocess.run([sys.executable, "-c", foundation + "\n" + code],
                                        cwd=ROOT, text=True, capture_output=True, timeout=90)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_binding_readme_first_valuation(self):
        source = (ROOT / "bindings/python/README.md").read_text()
        code = re.search(r"```python\n(.*?)```", source, re.DOTALL).group(1)
        result = subprocess.run([sys.executable, "-c", code], cwd=ROOT,
                                text=True, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_script_product_and_mapping_return_numeric_dictionaries(self):
        snippets = guide_snippets()
        code = snippets["guide-python-swap"] + "\n" + snippets["guide-python-script"] + """
product = qs.ScriptedProduct.from_dict(script)
with context:
    product_values = context.evaluate_script(product, n_paths=8, seed=42)
assert isinstance(product_values, dict)
assert product_values.keys() == scripted_values.keys()
assert all(isinstance(key, str) and isinstance(value, float)
           for key, value in product_values.items())
assert all(abs(product_values[key] - scripted_values[key]) < 1e-8
           for key in product_values)
"""
        result = subprocess.run([sys.executable, "-c", code], cwd=ROOT,
                                text=True, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


class RenderedReferenceTests(unittest.TestCase):
    def test_strict_build_renders_real_signatures_and_fields(self):
        build_root = ROOT / "docs/_build"
        build_root.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="reference-", dir=build_root) as temporary:
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
                         "excel/functions/section-time-calendars.html",
                         "excel/functions/section-math-interpolation.html", "coverage.html"):
                self.assertTrue((output / path).is_file(), path)
            trade = Text((output / "autoapi/quantsupport/pricing/Trade.html").read_text())
            self.assertIn("datetime.date", trade.text)
            self.assertNotIn("_DateLike", trade.text)
            price = Text((output / "excel/functions/qs-price.html").read_text())
            for field in ("Parameters", "Returns", "Errors"):
                self.assertRegex(price.text, field + r"\s*:")
            for error in ("#N/A", "#VALUE!"):
                self.assertIn(error, price.text)
            interpolation = Text((output / "excel/functions/qs-interpolate.html").read_text())
            self.assertIn("defaults to Linear", interpolation.text)
            self.assertIn("strictly increasing", interpolation.text)
            self.assertIn("#VALUE!", interpolation.text)
            self.assertEqual(len(list((output / "excel/functions").glob("qs-*.html"))), 109)
            for index in ("genindex.html", "py-modindex.html", "excel-functionindex.html", "search.html", "objects.inv"):
                self.assertTrue((output / index).is_file(), index)
            self.check_guides_and_studies(output)
            context = Text((output / "autoapi/quantsupport/pricing/PricingContext.html").read_text())
            self.assertIn("quantsupport.pricing.PricingContext.__enter__", context.ids)
            context_source = (output / "autoapi/quantsupport/pricing/PricingContext.html").read_text()
            script_signature = re.search(
                r'<dt[^>]*id="quantsupport.pricing.PricingContext.evaluate_script"[^>]*>(.*?)</dt>',
                context_source, re.DOTALL,
            )
            self.assertIsNotNone(script_signature)
            rendered_signature = Text(script_signature[1]).text
            self.assertIn("ScriptedProduct", rendered_signature)
            self.assertIn("Mapping", rendered_signature)
            self.assertRegex(rendered_signature, r"→\s*dict\s*\[\s*str\s*,\s*float\s*\]")
            date = Text((output / "autoapi/quantsupport/time/Date.html").read_text())
            self.assertIn("quantsupport.time.Date.__add__", date.ids)
            factory = Text((output / "autoapi/quantsupport/market/CurveConstruction.html").read_text())
            self.assertIn("no direct constructor", factory.text)
            self.assertNotIn("PythonDate", date.text)
            function_index = Text((output / "excel-functionindex.html").read_text())
            for alias in ("QS.CREATE.FX.STORE", "QS.GET.FX.QUOTE", "QS.ADD.FX.QUOTE", "QS.STORE.RESET"):
                self.assertIn(alias, function_index.text)

    def check_guides_and_studies(self, output):
        paths = [output / "index.html", output / "coverage.html",
                 output / "python/index.html", output / "excel/index.html",
                 output / "python/studies/index.html",
                 *(output / "python/guide").glob("*.html"),
                 *(output / "excel/guide").glob("*.html"),
                 *(output / "python/studies/notebooks").glob("*.html")]
        parsed = {}
        for path in paths:
            page = parsed.setdefault(path, Text(path.read_text()))
            for href in (*page.hrefs, *page.images):
                link = urlsplit(href)
                if link.scheme or link.netloc:
                    continue
                target = (path.parent / unquote(link.path)).resolve() if link.path else path.resolve()
                with self.subTest(page=path.relative_to(output), link=href):
                    self.assertTrue(target.is_relative_to(output.resolve()), "Link leaves standalone site")
                    self.assertTrue(target.is_file(), "Missing local target")
                    if link.fragment and target.suffix == ".html":
                        destination = parsed.setdefault(target, Text(target.read_text()))
                        self.assertIn(unquote(link.fragment), destination.ids)
        studies = list((output / "python/studies/notebooks").glob("*.html"))
        self.assertEqual(len(studies), 14)
        canonical = {slug: source for group in STUDIES.values() for slug, source in group}
        for path in studies:
            page = Text(path.read_text())
            self.assertIn("documentation build does not execute", page.text)
            downloads = [href for href in page.hrefs if "_downloads/" in href and href.endswith(".ipynb")]
            self.assertTrue(downloads)
            copied_path = ROOT / f"docs/python/studies/notebooks/{path.stem}.ipynb"
            published_path = (path.parent / unquote(urlsplit(downloads[0]).path)).resolve()
            self.assertEqual(published_path.read_bytes(), copied_path.read_bytes())
            copied = json.loads(copied_path.read_text())
            original = json.loads((ROOT / canonical[path.stem]).read_text())
            self.assertEqual(copied["metadata"]["execution_environment"],
                             original["metadata"]["execution_environment"])
        curves = (output / "python/studies/notebooks/curves.html").read_text()
        self.assertIn("<table", curves)
        self.assertRegex(curves, r'class="math(?: |")')
        self.assertTrue(any(Text(path.read_text()).images for path in studies))
        loans = (output / "python/studies/notebooks/loans.html").read_text()
        self.assertRegex(loans, r'<table[^>]*id="T_[^"]+"')
        self.assertIn('<style type="text/css">', loans)


if __name__ == "__main__":
    unittest.main()
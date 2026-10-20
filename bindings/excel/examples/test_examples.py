"""Portable checks of workbook navigation and native formula dependencies."""

import json
from pathlib import Path
import re
import subprocess
from tempfile import TemporaryDirectory
import unittest
from xml.etree import ElementTree as ET
from zipfile import ZipFile

from create_examples import EXAMPLES, LOAN_CASES, validation_names

ROOT = Path(__file__).resolve().parents[3]
NS = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
QUOTED = r'"(?:[^"\n]|"")*"'
SHEET_RANGE = r"'((?:[^']|'')+)'!(\$?[A-Z]+\$?\d+(?::\$?[A-Z]+\$?\d+)?)"
LOCAL_RANGE = r"(?<![\w.])\$?[A-Z]{1,3}\$?\d+(?::\$?[A-Z]{1,3}\$?\d+)?\b"


def arguments(formula):
    """Split arguments without breaking quoted strings or nested calls."""
    values, current, quoted, depth = [], "", False, 0
    for character in formula.split("(", 1)[1]:
        if character == '"':
            quoted = not quoted
        if not quoted:
            if character == "(":
                depth += 1
            elif character == ")":
                if depth == 0:
                    return values + ([current] if current or values else [])
                depth -= 1
            elif character == "," and depth == 0:
                values.append(current)
                current = ""
                continue
        current += character
    raise AssertionError(f"Unclosed formula: {formula}")


def coordinates(address):
    column, row = re.fullmatch(r"([A-Z]+)(\d+)", address.replace("$", "")).groups()
    number = 0
    for character in column:
        number = number * 26 + ord(character) - ord("A") + 1
    return number, int(row)


class Workbook:
    def __init__(self, path):
        with ZipFile(path) as archive:
            assert archive.testzip() is None
            assert not any(part.startswith("xl/externalLinks/") for part in archive.namelist())
            self.xml = ET.fromstring(archive.read("xl/workbook.xml"))
            self.names = {name.attrib["name"]: name.text for name in self.xml.findall("s:definedNames/s:definedName", NS)}
            strings = ET.fromstring(archive.read("xl/sharedStrings.xml"))
            self.strings = ["".join(node.text or "" for node in item.findall(".//s:t", NS)) for item in strings]
            self.sheets = {}
            self.cells = {}
            for index, sheet in enumerate(self.xml.findall("s:sheets/s:sheet", NS), 1):
                name = sheet.attrib["name"]
                tree = ET.fromstring(archive.read(f"xl/worksheets/sheet{index}.xml"))
                self.sheets[name] = tree
                self.cells[name] = {cell.attrib["r"]: cell for cell in tree.findall(".//s:sheetData/s:row/s:c", NS)}

    def value(self, sheet, address):
        cell = self.cells[sheet][address]
        value = cell.find("s:v", NS)
        if value is None or value.text is None:
            return None
        return self.strings[int(value.text)] if cell.attrib.get("t") == "s" else value.text

    def literal(self, expression):
        if expression.startswith('"'):
            return expression.strip('"')
        match = re.fullmatch(SHEET_RANGE, self.names[expression])
        return self.value(match[1].replace("''", "'"), match[2].replace("$", ""))

    def formulas(self):
        for sheet, cells in self.cells.items():
            for address, cell in cells.items():
                expression = cell.find("s:f", NS)
                if expression is not None:
                    yield sheet, address, expression.text, expression

    def range_cells(self, sheet, reference):
        first, _, last = reference.partition(":")
        low, high = coordinates(first), coordinates(last or first)
        return [(sheet, cell) for cell in self.cells[sheet]
                if low[0] <= coordinates(cell)[0] <= high[0] and low[1] <= coordinates(cell)[1] <= high[1]]

    def dependencies(self, expression, sheet=None):
        """Find cell and defined-name edges, rejecting unresolved worksheet names."""
        dependencies = []
        expression = re.sub(QUOTED, "", expression)

        def external(match):
            target = match[1].replace("''", "'")
            dependencies.extend(self.range_cells(target, match[2]))
            return ""

        expression = re.sub(SHEET_RANGE, external, expression)

        def local(match):
            dependencies.extend(self.range_cells(sheet, match[0]))
            return ""

        expression = re.sub(LOCAL_RANGE, local, expression)
        # Remove function identifiers; their remaining operands must be names or constants.
        expression = re.sub(r"[A-Za-z_][\w.]*\s*(?=\()", "", expression)
        for name in re.findall(r"[A-Za-z_][\w.]*", expression):
            if name.upper() in ("TRUE", "FALSE"):
                continue
            if name not in self.names:
                raise AssertionError(f"Unknown name {name!r} in {expression!r}")
            dependencies.append(name)
        return dependencies


class WorkbookTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(["cargo", "run", "--quiet", "-p", "quantsupport-xll", "--example", "function_catalog"], cwd=ROOT, text=True, capture_output=True, check=True)
        cls.catalog = {name: record for record in json.loads(result.stdout)
                       for name in (record["name"], *record["aliases"])}
        cls.temporary = TemporaryDirectory()
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.books = {}
        for name, filename, generate in EXAMPLES:
            path = Path(cls.temporary.name) / filename
            generate(path)
            cls.books[name] = Workbook(path)

    def test_registered_functions_context_positions_and_native_pricing_workflow(self):
        for name, book in self.books.items():
            functions, count = set(), 0
            for sheet, address, expression, node in book.formulas():
                if not expression.startswith("_xll.QS."):
                    continue
                formula = expression.removeprefix("_xll.")
                function = formula.split("(", 1)[0]
                with self.subTest(example=name, sheet=sheet, cell=address, function=function):
                    functions.add(function)
                    self.assertIn(function, self.catalog)
                    self.assertFalse(function.startswith("QS.TRADE.") and function != "QS.TRADE.CREATE")
                    self.assertNotIn(function, ("QS.PRICE", "QS.BOND.PRICE", "QS.FAIR.RATE"))
                    self.assertFalse(function.endswith(".FROM.JSON"))
                    args = arguments(formula)
                    parameters = self.catalog[function]["arguments"]
                    self.assertLessEqual(len(args), len(parameters))
                    for index, parameter in enumerate(parameters):
                        if parameter["type"].startswith(("Option", "* const")):
                            continue
                        self.assertLess(index, len(args), parameter["name"])
                        self.assertTrue(args[index], parameter["name"])
                    self.assertEqual(node.attrib["t"], "array", "Formula2 must preserve XLL spills")
                    self.assertNotIn("@", formula)
                    if function == "QS.CONTEXT.CREATE":
                        self.assertGreaterEqual(len(args), 14)
                        self.assertIn(book.literal(args[12]), ("USD", "EUR"))
                        self.assertIn(book.literal(args[13]), ("SOFR", "ESTR"))
                    # Blank cached results cannot be mistaken for zero PV before Excel calculates.
                    self.assertIsNone(book.value(sheet, address))
                    count += 1
            self.assertGreater(count, 10)
            if name != "interpolation":
                self.assertTrue(any(function.startswith("QS.INSTRUMENT.") for function in functions))
                self.assertTrue({"QS.TRADE.CREATE", "QS.PRICER.CREATE", "QS.EVALUATE", "QS.RESULT.VALUE"} <= functions)
                self.assertTrue(any(function.startswith("QS.DISCOUNT.POLICY.") for function in functions))
            for value in book.strings:
                if value.strip().startswith(("{", "[")):
                    try:
                        decoded = json.loads(value)
                    except ValueError:
                        continue
                    self.assertNotIsInstance(decoded, (dict, list), "Worksheet inputs must not require JSON")

    def test_start_page_links_every_numbered_tab_and_opens_first(self):
        for name, book in self.books.items():
            with self.subTest(example=name):
                self.assertEqual(next(iter(book.sheets)), "Start Here")
                self.assertEqual(book.xml.find("s:bookViews/s:workbookView", NS).attrib.get("activeTab", "0"), "0")
                linked = {link.attrib["location"] for link in book.sheets["Start Here"].findall("s:hyperlinks/s:hyperlink", NS)}
                for index, (sheet, tree) in enumerate(list(book.sheets.items())[1:], 1):
                    self.assertTrue(sheet.startswith(f"{index} "))
                    self.assertIn(f"'{sheet}'!A1", linked)
                    self.assertIsNotNone(tree.find("s:sheetViews/s:sheetView/s:pane", NS))
                for sheet, tree in book.sheets.items():
                    for link in tree.findall("s:hyperlinks/s:hyperlink", NS):
                        target = re.fullmatch(SHEET_RANGE, link.attrib["location"])
                        self.assertIsNotNone(target, link.attrib)
                        self.assertIn(target[2].replace("$", ""), book.cells[target[1]])

    def test_all_formula_dependencies_resolve_without_cycles(self):
        for name, book in self.books.items():
            graph = {key: book.dependencies(expression) for key, expression in book.names.items()
                     if not key.startswith("_xlnm.")}
            for sheet, address, expression, _ in book.formulas():
                self.assertNotIn("#REF!", expression)
                graph[sheet, address] = book.dependencies(expression, sheet)
            done, pending = set(), set()

            def visit(node):
                if node in done:
                    return
                self.assertNotIn(node, pending, f"{name}: circular dependency at {node}")
                pending.add(node)
                for dependency in graph.get(node, []):
                    visit(dependency)
                pending.remove(node)
                done.add(node)

            for node in graph:
                visit(node)

    def test_results_do_not_mix_native_construction_with_financial_measures(self):
        for name, book in self.books.items():
            for sheet, address, expression, _ in book.formulas():
                if not sheet.endswith(" Results") or not expression.startswith("_xll.QS."):
                    continue
                with self.subTest(example=name, sheet=sheet, cell=address):
                    self.assertTrue(expression.startswith(("_xll.QS.RESULT.", "_xll.QS.CURVE.DISCOUNT(",
                                                          "_xll.QS.CURVE.FORWARD(", "_xll.QS.CONTEXT.SNAPSHOT.TIMESTAMP(",
                                                          "_xll.QS.CONTEXT.EVALUATION.DATE(")))
            filename = next(filename for example, filename, _ in EXAMPLES if example == name)
            handles, numbers = validation_names(Path(filename).stem)
            self.assertTrue(set(handles + numbers) <= set(book.names), "Live checks must follow the generated workbook")

    def test_detail_sheets_share_the_same_result_and_have_room_to_spill(self):
        for name in ("swap", "loans", "bonds"):
            book = self.books[name]
            details = [sheet for sheet in book.sheets if sheet.endswith((" Cashflows", " Risk"))]
            dependencies = []
            for sheet in details:
                formulas = [(address, expression) for tab, address, expression, _ in book.formulas()
                            if tab == sheet and expression.startswith("_xll.QS.")]
                self.assertEqual(len(formulas), 1)
                address, expression = formulas[0]
                self.assertEqual(address, "A7")
                dependencies.append(arguments(expression)[0])
                # No other content can collide with a longer cashflow or sensitivity table.
                self.assertFalse([cell for cell in book.cells[sheet] if coordinates(cell)[1] >= 7 and cell != "A7"])
            self.assertEqual(dependencies[0], dependencies[1])
            if name != "swap":
                self.assertTrue(dependencies[0].endswith("_SelectedResult"))
                tree = next(tree for sheet, tree in book.sheets.items() if sheet.endswith(" Results"))
                sources = [rule.find("s:formula1", NS).text for rule in tree.findall("s:dataValidations/s:dataValidation", NS)]
                self.assertTrue(any(source.endswith("_CaseNames") for source in sources))

    def test_all_loan_cases_keep_separate_instruments_trades_and_fair_targets(self):
        book = self.books["loans"]
        evaluate = [arguments(expression) for _, _, expression, _ in book.formulas()
                    if expression.startswith("_xll.QS.EVALUATE(")]
        for index, (label, _, _, _, _, segments, _) in enumerate(LOAN_CASES, 1):
            prefix = f"Loan_C{index:02}_"
            self.assertTrue({prefix + key for key in ("Instrument", "Trade", "Result", "PV")} <= set(book.names))
            fair = [args for args in evaluate if args[0].startswith(f'"Loan.C{index:02}.Fair')]
            count = 0 if label == "Bounded floating" else len(segments) or 1
            self.assertEqual([args[4] for args in fair], [f"Loan_Request{target}" for target in range(count)])


if __name__ == "__main__":
    unittest.main()

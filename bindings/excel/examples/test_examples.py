"""Portable checks of the generated workbooks against the native function catalog."""

import json
from pathlib import Path
import re
import subprocess
from tempfile import TemporaryDirectory
import unittest
from xml.etree import ElementTree as ET
from zipfile import ZipFile

from create_examples import EXAMPLES

ROOT = Path(__file__).resolve().parents[3]
NS = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}


def arguments(formula):
    """Split arguments without breaking quoted JSON, sheet names, or nested calls."""
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


class WorkbookTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(["cargo", "run", "--quiet", "-p", "quantsupport-xll", "--example", "function_catalog"], cwd=ROOT, text=True, capture_output=True, check=True)
        cls.catalog = {name: record for record in json.loads(result.stdout)
                       for name in (record["name"], *record["aliases"])}

    def test_every_workbook_uses_registered_functions_and_valid_context_positions(self):
        with TemporaryDirectory() as temporary:
            for name, filename, generate in EXAMPLES:
                path = Path(temporary) / filename
                generate(path)
                with self.subTest(example=name), ZipFile(path) as archive:
                    self.assertIsNone(archive.testzip())
                    self.assertFalse(any(part.startswith("xl/externalLinks/") for part in archive.namelist()))
                    formula_count = 0
                    functions = set()
                    for part in archive.namelist():
                        if not re.fullmatch(r"xl/worksheets/sheet\d+.xml", part):
                            continue
                        sheet = ET.fromstring(archive.read(part))
                        for cell in sheet.findall(".//s:c", NS):
                            expression = cell.find("s:f", NS)
                            if expression is None or not expression.text.startswith("_xll.QS."):
                                continue
                            formula = expression.text.removeprefix("_xll.")
                            function = formula.split("(", 1)[0]
                            with self.subTest(sheet=part, cell=cell.attrib["r"], function=function):
                                functions.add(function)
                                self.assertIn(function, self.catalog)
                                self.assertFalse(function.startswith("QS.TRADE.") and function != "QS.TRADE.CREATE", "Example contracts and positions must be separate")
                                self.assertNotIn(function, ("QS.PRICE", "QS.BOND.PRICE", "QS.FAIR.RATE"), "Examples must use explicit native pricers and result readers")
                                self.assertFalse(function.endswith(".FROM.JSON"), "Examples must use worksheet inputs rather than JSON imports")
                                args = arguments(formula)
                                self.assertLessEqual(len(args), len(self.catalog[function]["arguments"]))
                                # Formula2 storage preserves XLL spills, avoiding implicit @ intersection.
                                self.assertEqual(expression.attrib["t"], "array")
                                self.assertNotIn("@", formula)
                                if function == "QS.CONTEXT.CREATE":
                                    self.assertGreaterEqual(len(args), 14)
                                    self.assertIn(args[12], ('"USD"', '"EUR"'))
                                    self.assertIn(args[13], ('"SOFR"', '"ESTR"'))
                                formula_count += 1
                    self.assertGreater(formula_count, 10)
                    if name != "interpolation":
                        self.assertTrue(any(function.startswith("QS.INSTRUMENT.") for function in functions))
                        self.assertTrue({"QS.TRADE.CREATE", "QS.PRICER.CREATE", "QS.EVALUATE", "QS.RESULT.VALUE"} <= functions)
                        self.assertTrue(any(function.startswith("QS.DISCOUNT.POLICY.") for function in functions))
                    shared = ET.fromstring(archive.read("xl/sharedStrings.xml"))
                    for item in shared.findall("s:si", NS):
                        value = "".join(node.text or "" for node in item.findall(".//s:t", NS)).strip()
                        if value.startswith(("{", "[")):
                            try:
                                decoded = json.loads(value)
                            except ValueError:
                                continue
                            self.assertNotIsInstance(decoded, (dict, list), "Examples must not store JSON input cells")


if __name__ == "__main__":
    unittest.main()

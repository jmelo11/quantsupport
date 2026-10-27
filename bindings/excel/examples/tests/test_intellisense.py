"""Validate the native XLL's metadata against Excel-DNA's XML protocol."""

import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
import unittest
from xml.etree import ElementTree as ET


ROOT = Path(__file__).resolve().parents[4]
COMMAND = ["cargo", "run", "--quiet", "-p", "quantsupport-xll", "--example", "function_catalog"]
NS = {"i": "http://schemas.excel-dna.net/intellisense/1.0"}


class IntelliSenseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(COMMAND, cwd=ROOT, text=True, capture_output=True, check=True)
        cls.records = json.loads(result.stdout)
        with TemporaryDirectory() as temporary:
            # Exercise the file export used by the Windows build script,
            # including a path with spaces and non-ASCII characters.
            path = Path(temporary) / "QuantSupport parámetros.intellisense.xml"
            subprocess.run(COMMAND + ["--", "--intellisense", str(path)], cwd=ROOT,
                           text=True, capture_output=True, check=True)
            cls.xml = ET.fromstring(path.read_bytes())

    def test_every_registered_name_and_alias_has_parameter_descriptions(self):
        self.assertEqual(self.xml.tag, "{" + NS["i"] + "}IntelliSense")
        nodes = self.xml.findall("i:FunctionInfo/i:Function", NS)
        functions = {node.attrib["Name"]: node for node in nodes}
        expected = {name for record in self.records
                    for name in (record["name"], *record["aliases"])}
        self.assertEqual(len(functions), len(nodes), "Duplicate IntelliSense functions")
        self.assertEqual(set(functions), expected)
        for record in self.records:
            for name in (record["name"], *record["aliases"]):
                with self.subTest(function=name):
                    function = functions[name]
                    self.assertEqual(function.attrib["Description"], record["help"])
                    self.assertTrue(function.attrib["HelpTopic"].endswith(
                        "/" + record["name"].lower().replace(".", "-") + ".html"))
                    arguments = function.findall("i:Argument", NS)
                    self.assertEqual([argument.attrib["Name"] for argument in arguments],
                                     [argument["name"] for argument in record["arguments"]])
                    self.assertEqual([argument.attrib["Description"] for argument in arguments],
                                     [argument["help"] for argument in record["arguments"]])

    def test_inline_hints_preserve_the_full_context_signature_and_fair_rate_target(self):
        functions = {node.attrib["Name"]: node for node in
                     self.xml.findall("i:FunctionInfo/i:Function", NS)}
        context = functions["QS.CONTEXT.CREATE"].findall("i:Argument", NS)
        self.assertEqual([argument.attrib["Name"] for argument in context],
                         ["name", "reference_date", "base_currency", "base_index",
                          "quote_level", "anchor_fixings"])
        fair_rate = functions["QS.REQUEST.FAIR.RATE"].findall("i:Argument", NS)
        self.assertEqual([argument.attrib["Name"] for argument in fair_rate], ["index"])
        self.assertIn("Zero-based", fair_rate[0].attrib["Description"])


if __name__ == "__main__":
    unittest.main()

"""Contracts for references generated into the single mdBook site."""

import ast
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
import json
import subprocess


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("binding_docs", ROOT / "book" / "bindings.py")
bindings = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bindings)


class BindingReferenceTests(unittest.TestCase):
    def test_docstrings_preserve_fences_and_convert_inline_roles(self):
        text = "Use ``value`` and :class:`Trade`.\n\n```python\nprint(value)\n```"
        self.assertEqual(bindings.markdown(text),
                         "Use `value` and `Trade`.\n\n```python\nprint(value)\n```")

    def test_signatures_preserve_keyword_only_and_positional_only_arguments(self):
        method = ast.parse("def __new__(cls, value: _DateLike, /, *, enabled: bool = ...) -> Self: ...").body[0]
        self.assertEqual(bindings.signature(method, "Example"),
                         "Example(value: Date | str | datetime.date, /, *, enabled: bool=...)")
        self.assertEqual(method.args.posonlyargs[0].arg, "cls")

    def test_python_reference_distinguishes_properties_and_methods(self):
        pricing = bindings.python_reference(ROOT, "pricing")
        self.assertIn("## PricingContext", pricing)
        self.assertIn("**Read/write property:** `curves:", pricing)
        self.assertIn("**Read-only property:** `reference_date:", pricing)
        self.assertIn("evaluate(trade: Trade, requests:", pricing)
        self.assertNotIn("_DateLike", pricing)
        self.assertNotIn(".. code-block::", pricing)
        self.assertIn("```python\nquotes = qs.QuoteStore.from_json", pricing)
        self.assertIn("Market observations and reference date.", pricing)

    def test_matching_new_and_init_signatures_are_not_duplicated(self):
        instruments = bindings.python_reference(ROOT, "instruments")
        section = instruments.split("## Instrument {#instrument}", 1)[1].split("\n## ", 1)[0]
        self.assertEqual(section.count("### Constructor"), 1)

    def test_each_exported_class_has_a_reference_entry(self):
        for domain in bindings.DOMAINS:
            with self.subTest(domain=domain):
                reference = bindings.python_reference(ROOT, domain)
                self.assertGreater(reference.count("\n## "), 5)
                self.assertIn("Typed source", reference)

    def test_preprocessor_adds_searchable_chapters_without_mutating_sources(self):
        record = {"name": "QS.TEST", "category": "QuantSupport - Time", "aliases": ["QS.ALIAS"],
                  "help": "Tests <entity> dates.", "arguments": [{"name": "date", "help": "Date | ISO string"}],
                  "threadsafe": True, "volatile": False, "source": "bindings/excel/src/functions/time.rs"}
        book = {"items": [bindings.chapter("Python API", "python/api.md", "# Python API", {"sub_items": [], "name": "Book"}),
                             bindings.chapter("Excel Reference", "excel/reference.md", "# Excel Reference", {"sub_items": [], "name": "Book"})]}
        completed = subprocess.CompletedProcess([], 0, json.dumps([record]), "")
        with patch.object(bindings.subprocess, "run", return_value=completed):
            result = bindings.preprocess({"root": str(ROOT)}, book)
        python = result["items"][0]["Chapter"]
        excel = result["items"][1]["Chapter"]
        self.assertEqual(len(python["sub_items"]), 5)
        self.assertEqual(excel["sub_items"][0]["Chapter"]["path"], "excel/reference/time.md")
        content = excel["sub_items"][0]["Chapter"]["content"]
        self.assertIn("=QS.TEST(date)", content)
        self.assertIn("QS.ALIAS", content)
        self.assertIn("Date \\| ISO string", content)
        self.assertIn("Tests &lt;entity&gt; dates.", content)
        self.assertIsNone(excel["sub_items"][0]["Chapter"]["source_path"])


if __name__ == "__main__":
    unittest.main()
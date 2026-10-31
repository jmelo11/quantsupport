"""Check the distributed large workbook, including every dated valuation row."""
from pathlib import Path
from tempfile import TemporaryDirectory
import sys
import unittest
from xml.etree import ElementTree as ET
from zipfile import ZipFile

NS = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tools/excel/examples"))
from create_origination_example import generate


class OriginationWorkbookTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        directory = TemporaryDirectory()
        cls.addClassCleanup(directory.cleanup)
        cls.directory = Path(directory.name)
        for layout in ("rows", "batch"):
            generate(cls.directory / f"{layout}.xlsx", layout=layout)

    def test_fast_workbook_pairs_ten_thousand_contexts_and_trades_in_one_spill(self):
        path = self.directory / "batch.xlsx"
        with ZipFile(path) as archive:
            sheet = ET.fromstring(archive.read("xl/worksheets/sheet2.xml"))
            cells = {c.get("r"): c for c in sheet.findall("s:sheetData/s:row/s:c", NS)}
            self.assertEqual(cells["G2"].find("s:f", NS).text, "QS.EVALUATE.RANGE(C2:C10001,F2:F10001)")
            self.assertEqual(sum(c.find("s:f", NS) is not None for address, c in cells.items() if address.startswith("G")), 1)
            self.assertEqual(sum(c.find("s:f", NS) is not None for address, c in cells.items() if address.startswith("C")), 10000)
            self.assertIn('E10001,A10001,', cells["F10001"].find("s:f", NS).text)

    def test_ten_thousand_distinct_dated_contexts_and_live_valuations(self):
        path = self.directory / "rows.xlsx"
        with ZipFile(path) as archive:
            self.assertIsNone(archive.testzip())
            book = ET.fromstring(archive.read("xl/workbook.xml"))
            self.assertEqual([s.get("name") for s in book.findall("s:sheets/s:sheet", NS)], ["Start Here", "Origination"])
            strings = ["".join(item.itertext()) for item in ET.fromstring(archive.read("xl/sharedStrings.xml"))]
            sheet = ET.fromstring(archive.read("xl/worksheets/sheet2.xml"))
            rows = sheet.findall("s:sheetData/s:row", NS)
            self.assertEqual(len(rows), 10001)
            dates = set()
            for row in rows[1:]:
                n = row.get("r")
                cells = {c.get("r"): c for c in row}
                dates.add(strings[int(cells[f"A{n}"].find("s:v", NS).text)])
                self.assertIn(f',A{n},', cells[f"C{n}"].find("s:f", NS).text)
                self.assertIn(f'(C{n},"SOFR",', cells[f"D{n}"].find("s:f", NS).text)
                self.assertIn(f',E{n},A{n},', cells[f"F{n}"].find("s:f", NS).text)
                self.assertEqual(cells[f"G{n}"].find("s:f", NS).text, f"QS.EVALUATE(C{n},F{n})")
            self.assertEqual(len(dates), 10000)
            self.assertFalse(any(n.startswith("xl/externalLinks/") for n in archive.namelist()))


if __name__ == "__main__":
    unittest.main()

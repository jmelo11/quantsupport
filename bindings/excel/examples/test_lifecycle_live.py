"""Optional Excel UI tests: Windows + pywin32 + QS_XLL_PATH are required."""

import os
from pathlib import Path
import sys
import tempfile
import time
import unittest


@unittest.skipUnless(sys.platform == "win32" and os.environ.get("QS_XLL_PATH"),
                     "requires Windows Excel and QS_XLL_PATH")
class ExcelLifecycleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from win32com.client import DispatchEx

        cls.excel = DispatchEx("Excel.Application")
        cls.excel.DisplayAlerts = False
        cls.excel.EnableEvents = True
        cls.anchor = cls.excel.Workbooks.Add()
        cls.excel.Calculation = -4105  # xlCalculationAutomatic
        path = str(Path(os.environ["QS_XLL_PATH"]).resolve())
        if not cls.excel.RegisterXLL(path):
            cls.excel.Quit()
            raise AssertionError(f"Excel could not load {path}")
        time.sleep(1.2)

    @classmethod
    def tearDownClass(cls):
        try:
            cls.excel.Quit()
        finally:
            cls.anchor = None
            cls.excel = None

    def count(self):
        cell = self.anchor.Worksheets(1).Range("Z1")
        cell.Formula = "=QS.OBJECT.COUNT(TRUE)"
        cell.Calculate()
        self.assertIsInstance(cell.Value2, (float, int), cell.Value2)
        return int(cell.Value2)

    def quotes(self, book):
        cell = book.Worksheets(1).Range("A1")
        cell.Formula = '=QS.QUOTES.CREATE("shared","2026-01-15")'
        handle = cell.Value2
        self.assertIsInstance(handle, str)
        self.assertTrue(handle.startswith("QSObject:QuoteStore:"), handle)
        check = book.Worksheets(1).Range("B1")
        check.Formula = "=QS.OBJECT.TYPE(A1)"
        self.assertEqual(check.Value2, "QuoteStore", "SheetChange must preserve the freshly calculated object")
        return handle

    def test_workbooks_isolate_names_and_close_independently(self):
        first = self.excel.Workbooks.Add()
        second = self.excel.Workbooks.Add()
        try:
            one = self.quotes(first)
            two = self.quotes(second)
            self.assertNotEqual(one, two)
            self.assertEqual(self.count(), 2)
            first.Close(False)
            first = None
            time.sleep(1.3)
            self.assertEqual(self.count(), 1)
            second.Close(False)
            second = None
            time.sleep(1.3)
            self.assertEqual(self.count(), 0)
        finally:
            if first is not None:
                first.Close(False)
            if second is not None:
                second.Close(False)

    def test_closing_the_last_workbook_collects_without_quitting_excel(self):
        book = self.excel.Workbooks.Add()
        self.quotes(book)
        self.assertEqual(self.count(), 1)
        book.Close(False)
        self.anchor.Close(False)
        self.__class__.anchor = None
        time.sleep(1.3)
        self.__class__.anchor = self.excel.Workbooks.Add()
        self.assertEqual(self.count(), 0)

    def test_reopening_saved_formulas_rebuilds_handles_and_respects_manual_mode(self):
        previous_mode = self.excel.Calculation
        with tempfile.TemporaryDirectory(prefix="qs-excel-lifetime-") as directory:
            path = str(Path(directory) / "Market.xlsx")
            book = self.excel.Workbooks.Add()
            try:
                original = self.quotes(book)
                book.SaveAs(path, 51)
                book.Close(False)
                book = None
                time.sleep(1.3)
                self.assertEqual(self.count(), 0)
                book = self.excel.Workbooks.Open(path)
                self.assertNotEqual(book.Worksheets(1).Range("A1").Value2, original)
                self.assertEqual(book.Worksheets(1).Range("B1").Value2, "QuoteStore")
                book.Close(False)
                book = None
                time.sleep(1.3)
                self.excel.Calculation = -4135
                book = self.excel.Workbooks.Open(path)
                self.assertEqual(self.count(), 0, "Manual open must defer native construction")
                self.excel.Calculate()
                self.assertEqual(self.count(), 1)
                self.assertEqual(book.Worksheets(1).Range("B1").Value2, "QuoteStore")
            finally:
                if book is not None:
                    book.Close(False)
                self.excel.Calculation = previous_mode

    def test_manual_mode_failed_edits_and_cleared_cells(self):
        book = self.excel.Workbooks.Add()
        previous_mode = self.excel.Calculation
        try:
            sheet = book.Worksheets(1)
            sheet.Range("B1").Value2 = "2026-01-15"
            cell = sheet.Range("A1")
            cell.Formula = '=QS.QUOTES.CREATE("manual",B1)'
            cell.Calculate()
            original = cell.Value2
            self.assertTrue(original.startswith("QSObject:"), original)
            self.excel.Calculation = -4135  # xlCalculationManual
            sheet.Range("B1").Value2 = "2026-01-16"
            time.sleep(0.1)
            self.assertEqual(cell.Value2, original)
            self.excel.Calculate()
            self.assertNotEqual(cell.Value2, original)
            self.assertEqual(self.count(), 1)
            cell.Formula = '=QS.QUOTES.CREATE("manual","invalid date")'
            cell.Calculate()
            self.assertEqual(self.count(), 0)
            cell.Formula = '=QS.QUOTES.CREATE("manual",B1)'
            cell.Calculate()
            self.assertEqual(self.count(), 1)
            cell.ClearContents()
            self.assertEqual(self.count(), 0)
        finally:
            self.excel.Calculation = previous_mode
            book.Close(False)

    def test_renamed_formula_outputs_do_not_accumulate(self):
        book = self.excel.Workbooks.Add()
        try:
            sheet = book.Worksheets(1)
            sheet.Range("B1").Value2 = "first"
            sheet.Range("A1").Formula = '=QS.QUOTES.CREATE(B1,"2026-01-15")'
            for position in range(20):
                sheet.Range("B1").Value2 = f"renamed{position}"
                sheet.Range("A1").Calculate()
                self.assertEqual(self.count(), 1)
            sheet.Range("A1").Formula = "=1+2"
            self.assertEqual(self.count(), 0)
        finally:
            book.Close(False)

    def test_bulk_ranges_update_and_remove_observations(self):
        book = self.excel.Workbooks.Add()
        try:
            sheet = book.Worksheets(1)
            sheet.Range("A1:B3").Value2 = (
                ("Identifier", "Mid"),
                ("FixedRateDeposit_USD_SOFR_6M", 0.04),
                ("FixedRateDeposit_USD_SOFR_1Y", 0.05),
            )
            sheet.Range("D1:F3").Value2 = (
                ("Market index", "Date", "Value"),
                ("SOFR", "2026-01-14", 0.03),
                ("SOFR", "2026-01-15", 0.04),
            )
            sheet.Range("H1").Formula = '=QS.QUOTES.FROM.RANGE("quotes","2026-01-15",A1:B3)'
            sheet.Range("H2").Formula = '=QS.FIXINGS.FROM.RANGE("fixings",D1:F3)'
            sheet.Range("I1").Formula = '=QS.QUOTES.GET(H1,"FixedRateDeposit_USD_SOFR_6M","Mid")'
            sheet.Range("I2").Formula = '=QS.FIXINGS.GET(H2,"SOFR","2026-01-14")'
            self.assertEqual(sheet.Range("I1").Value2, 0.04)
            self.assertEqual(sheet.Range("I2").Value2, 0.03)
            sheet.Range("B2").Value2 = 0.06
            sheet.Range("F2").Value2 = 0.07
            self.assertEqual(sheet.Range("I1").Value2, 0.06)
            self.assertEqual(sheet.Range("I2").Value2, 0.07)
            sheet.Range("A2:B2").ClearContents()
            sheet.Range("D2:F2").ClearContents()
            self.assertIsInstance(sheet.Range("I1").Value2, int)  # Excel error
            self.assertIsInstance(sheet.Range("I2").Value2, int)
        finally:
            book.Close(False)


if __name__ == "__main__":
    unittest.main()

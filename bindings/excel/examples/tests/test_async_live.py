"""RTD feasibility gate and live regression tests. Uses an isolated Excel instance.

QS_XLL_PATH must point to the native XLL beside its matching ribbon companion.
Never changes Excel security policy or the application-wide RTD throttle.
"""
import json
import os
from pathlib import Path
import time
import unittest
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))


@unittest.skipUnless(os.name == "nt" and os.environ.get("QS_XLL_PATH"),
                     "requires Windows Excel and QS_XLL_PATH")
class AsyncExcelTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from win32com.client import DispatchEx, gencache
        cls.app = gencache.EnsureDispatch(DispatchEx("Excel.Application"))
        cls.app.DisplayAlerts = False
        cls.book = cls.app.Workbooks.Add()
        path = Path(os.environ["QS_XLL_PATH"]).resolve()
        assert cls.app.RegisterXLL(str(path))
        assert cls.app.RegisterXLL(str(path.with_name("QuantSupport.Ribbon64.xll")))
        cls.sheet = cls.book.Worksheets(1)

    @classmethod
    def tearDownClass(cls):
        import gc
        cls.sheet = None
        cls.book.Close(False)
        cls.book = None
        cls.app.Quit()
        cls.app = None
        gc.collect()

    def status(self):
        c = self.sheet.Range("Z1")
        c.Formula = "=QS.ENGINE.STATUS()"
        c.Calculate()
        return json.loads(c.Value2)

    def await_value(self, address, expected, timeout=15):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.sheet.Range(address).Value2 == expected:
                return
            time.sleep(.025)
        self.fail(f"{address}: {self.sheet.Range(address).Value2}, {self.status()}")

    def test_01_nested_rtd_scalar_and_spill(self):
        self.app.Calculation = -4105
        began = time.monotonic()
        self.sheet.Range("A1").Formula2 = "=QS.ENGINE.PROBE(1,1)"
        self.assertLess(time.monotonic() - began, .5)
        self.assertNotEqual(self.sheet.Range("A1").Value2, 42)
        self.sheet.Range("D1").Value2 = "editable while pending"
        self.await_value("A1", 42)
        self.sheet.Range("A3").Formula2 = "=QS.ENGINE.PROBE(.1,3)"
        self.await_value("A5", 44)
        self.assertEqual(self.sheet.Range("A3:A5").Value2, ((42.0,), (43.0,), (44.0,)))
        before = self.status()["submitted"]
        self.app.CalculateFull()
        time.sleep(.2)
        self.assertEqual(self.status()["submitted"], before)

    def test_02_owned_market_and_unchanged_calculation(self):
        self.sheet.Range("G1").Formula2 = '=QS.CONTEXT.CREATE("market","2026-01-15","USD","SOFR")'
        self.sheet.Range("G2").Formula2 = '=QS.INSTRUMENT.SWAP("swap","2026-01-15","2031-01-15","USD","SOFR",1000000,.04,0,"Actual360","Simple","Annual","Annual","Annual")'
        self.sheet.Range("G3").Formula2 = '=QS.TRADE.CREATE("trade",G2,"2026-01-15","Receive")'
        self.sheet.Range("J1").Value2 = .03
        # Evaluation is deliberately registered before the addition.
        self.sheet.Range("G4").Formula2 = '=QS.EVALUATE(G1,G3)'
        self.sheet.Range("G5").Formula2 = '=QS.CONTEXT.ADD.ZERO.CURVE(G1,"SOFR",{"0D","10Y"},J1:J2,"Actual360","Continuous","Annual","Linear",TRUE)'
        self.sheet.Range("J2").Value2 = .03
        self.app.CalculateFull()
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            value = self.sheet.Range("G4").Value2
            if isinstance(value, (float, int)) and value > 0:
                break
            time.sleep(.025)
        self.assertGreater(value, 0, self.status())
        original = value
        root = self.sheet.Range("G1").Value2
        before = self.status()["submitted"]
        self.app.CalculateFull()
        time.sleep(.3)
        self.assertEqual(self.status()["submitted"], before)
        self.sheet.Range("J1:J2").Value2 = ((.02,), (.02,))
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            value = self.sheet.Range("G4").Value2
            if isinstance(value, (float, int)) and value > original + 1000:
                break
            time.sleep(.025)
        self.assertGreater(value, original + 1000, self.status())
        self.assertEqual(self.sheet.Range("G1").Value2, root)

    @unittest.skipUnless(os.environ.get("QS_PROBE_XLL"), "requires the optional live ribbon probe XLL")
    def test_03_ribbon_shares_a_ready_worksheet_price(self):
        from async_excel import await_engine
        self.sheet.Range("S1").Formula2 = '=QS.DISCOUNT.POLICY.CSA("policy","USD","SOFR")'
        self.sheet.Range("S2").Formula2 = '=QS.PRICER.CREATE("pricer","DiscountedCashflowPricer",S1)'
        self.sheet.Range("G4").Formula2 = '=QS.EVALUATE(G1,G3,,S2)'
        await_engine(self.app, self.book)
        self.book.Names.Add("EngineTest_Trade", "='" + self.sheet.Name + "'!$G$3")
        path = Path(os.environ["QS_PROBE_XLL"]).resolve()
        if path.name != "QuantSupport.Ribbon64.xll": assert self.app.RegisterXLL(str(path))
        (path.parent / "selection.txt").write_text("EngineTest_Trade\nPV", encoding="utf-8")
        report = path.parent / "probe-result.txt"
        report.unlink(missing_ok=True)
        before = self.status()["submitted"]
        self.app.Run("QS.PanelProbe.Run")
        deadline = time.monotonic() + 35
        while not report.exists() and time.monotonic() < deadline:
            time.sleep(.025)
        self.assertTrue(report.exists(), "ribbon query did not complete")
        self.assertTrue(report.read_text(encoding="utf-8-sig").startswith("PASS"), report.read_text(encoding="utf-8-sig"))
        self.assertEqual(self.status()["submitted"], before, "ribbon must share the ready worksheet computation")

    def test_04_nested_contributions_duplicates_and_clear(self):
        from async_excel import await_engine
        self.sheet.Range("P1").Formula2 = '=QS.CONTEXT.CREATE("nested","2026-01-15","USD","SOFR")'
        self.sheet.Range("P2").Formula2 = '=LEN(QS.CONTEXT.ADD.QUOTE(P1,"FixedRateDeposit_USD_SOFR_6M",.04))+LEN(QS.CONTEXT.ADD.QUOTE(P1,"FixedRateDeposit_USD_SOFR_1Y",.05))'
        self.sheet.Range("P3").Formula2 = '=QS.CONTEXT.QUOTE(P1,"FixedRateDeposit_USD_SOFR_6M")'
        self.sheet.Range("P4").Formula2 = '=QS.CONTEXT.QUOTE(P1,"FixedRateDeposit_USD_SOFR_1Y")'
        await_engine(self.app, self.book)
        self.assertEqual(self.sheet.Range("P3:P4").Value2, ((.04,), (.05,)))
        self.sheet.Range("P2").Formula2 = '=LEN(QS.CONTEXT.ADD.QUOTE(P1,"FixedRateDeposit_USD_SOFR_6M",.06))+LEN(QS.CONTEXT.ADD.QUOTE(P1,"FixedRateDeposit_USD_SOFR_1Y",.05))'
        await_engine(self.app, self.book)
        self.assertEqual(self.sheet.Range("P3").Value2, .06)
        self.sheet.Range("P2").Formula2 = '=LEN(QS.CONTEXT.ADD.QUOTE(P1,"FixedRateDeposit_USD_SOFR_6M",.04))+LEN(QS.CONTEXT.ADD.QUOTE(P1,"FixedRateDeposit_USD_SOFR_6M",.04))'
        status = await_engine(self.app, self.book, allow_errors=True)
        self.assertTrue(any("duplicate" in d.lower() for d in status["diagnostics"]))
        self.assertTrue(self.app.WorksheetFunction.IsError(self.sheet.Range("P3")))
        self.sheet.Range("P2").ClearContents()
        await_engine(self.app, self.book, allow_errors=True)
        self.assertTrue(self.app.WorksheetFunction.IsError(self.sheet.Range("P3")))
        self.sheet.Range("P1:P4").ClearContents()

    def test_05_stored_results_and_lazy_curve_views(self):
        from async_excel import await_engine
        self.sheet.Range("U1").Formula2 = '=QS.EVALUATE.RESULTS(G1,G3,{"Value","Cashflows","Sensitivities"})'
        self.sheet.Range("U2").Formula2 = '=QS.RESULT.VALUE(U1)'
        self.sheet.Range("U3").Formula2 = '=QS.CURVE.GET(G1,"SOFR")'
        self.sheet.Range("U4").Formula2 = '=QS.CURVE.DISCOUNT(U3,"2027-01-15")'
        await_engine(self.app, self.book)
        self.assertTrue(self.sheet.Range("U1").Value2.startswith("QSEngine:EvaluationResults:"))
        self.assertFalse(self.app.WorksheetFunction.IsError(self.sheet.Range("U2")), (self.status(), self.sheet.Range("U1:U4").Value2))
        self.assertAlmostEqual(self.sheet.Range("U2").Value2, self.sheet.Range("G4").Value2)
        self.assertGreater(self.sheet.Range("U4").Value2, 0)
        self.assertLess(self.sheet.Range("U4").Value2, 1)

    def test_06_five_second_job_responsiveness_and_late_cancellation(self):
        self.sheet.Range("W1").Formula2 = '=QS.ENGINE.PROBE(5,1)'
        before = self.status()["submitted"]
        deadline = time.monotonic() + 1
        while self.status()["submitted"] == before and time.monotonic() < deadline:
            time.sleep(.025)
        delays = []
        deadline = time.monotonic() + 4.5
        index = 0
        while time.monotonic() < deadline:
            began = time.perf_counter()
            self.sheet.Range("W3").Value2 = index
            delays.append((time.perf_counter()-began)*1000)
            time.sleep(.025)
            index += 1
        self.assertLess(sorted(delays)[int(.95*len(delays))], 50, delays)
        print(f"\nFive-second probe COM heartbeat: p50={sorted(delays)[len(delays)//2]:.2f} ms, p95={sorted(delays)[int(.95*len(delays))]:.2f} ms, max={max(delays):.2f} ms", flush=True)
        self.sheet.Range("W1").ClearContents()
        time.sleep(1)
        self.assertIsNone(self.sheet.Range("W1").Value2, "late completion must not resurrect a cleared formula")

    def test_07_manual_mode_delivers_dependent_stages_without_changing_settings(self):
        from async_excel import await_engine
        previous_mode = self.app.Calculation
        throttle = self.app.RTD.ThrottleInterval
        try:
            self.app.Calculation = -4135
            self.sheet.Range("J1:J2").Value2 = ((.03,), (.03,))
            await_engine(self.app, self.book)
            self.assertAlmostEqual(self.sheet.Range("U2").Value2, self.sheet.Range("G4").Value2)
            self.assertFalse(self.app.WorksheetFunction.IsError(self.sheet.Range("U4")))
            self.assertEqual(self.app.Calculation, -4135)
            self.assertEqual(self.app.RTD.ThrottleInterval, throttle)
        finally:
            self.app.Calculation = previous_mode

    def test_08_paired_context_batch_matches_rows_and_reprices_one_market(self):
        from async_excel import await_engine
        for row, day in enumerate((15, 16, 19), 1):
            self.sheet.Range(f"AA{row}").Formula2 = f'=QS.CONTEXT.CREATE("batch.{row}","2026-01-{day}","USD","SOFR")'
            self.sheet.Range(f"AB{row}").Formula2 = f'=QS.CONTEXT.ADD.ZERO.CURVE(AA{row},"SOFR",{{"0D","10Y"}},{{.03,.03}},"Actual360","Continuous","Annual","Linear",TRUE)'
            self.sheet.Range(f"AC{row}").Formula2 = f'=QS.INSTRUMENT.SWAP("batch.swap.{row}","2026-02-02","2031-02-03","USD","SOFR",1000000,.04,0,"Actual360","Simple","Annual","Annual","Annual")'
            self.sheet.Range(f"AD{row}").Formula2 = f'=QS.TRADE.CREATE("batch.trade.{row}",AC{row},"2026-01-{day}","Receive")'
            self.sheet.Range(f"AE{row}").Formula2 = f'=QS.EVALUATE(AA{row},AD{row})'
        self.sheet.Range("AG1").Formula2 = '=QS.EVALUATE.RANGE(AA1:AA3,AD1:AD3)'
        await_engine(self.app, self.book)
        self.assertEqual(self.sheet.Range("AG1:AG3").Value2, self.sheet.Range("AE1:AE3").Value2)
        before = self.sheet.Range("AG1:AG3").Value2
        submitted = self.status()["submitted"]
        self.sheet.Range("AB1").Formula2 = '=QS.CONTEXT.ADD.ZERO.CURVE(AA1,"SOFR",{"0D","10Y"},{.04,.04},"Actual360","Continuous","Annual","Linear",TRUE)'
        await_engine(self.app, self.book)
        after = self.sheet.Range("AG1:AG3").Value2
        self.assertNotEqual(before[0], after[0])
        self.assertEqual(before[1:], after[1:])
        self.assertEqual(after, self.sheet.Range("AE1:AE3").Value2)
        self.assertEqual(self.status()["submitted"], submitted + 1)
        self.sheet.Range("AG5").Formula2 = '=QS.EVALUATE.RANGE(AA1:AA3,AD1:AD3,,,,TRUE)'
        await_engine(self.app, self.book)
        self.assertEqual(self.sheet.Range("AG5:AI5").Value2, (tuple(row[0] for row in after),))
        self.sheet.Range("AG10").Formula2 = '=QS.EVALUATE.RANGE(AA1:AA2,AD1:AD3)'
        await_engine(self.app, self.book, allow_errors=True)
        self.assertTrue(self.app.WorksheetFunction.IsError(self.sheet.Range("AG10")))
        self.sheet.Range("AG10").ClearContents()

    @unittest.skipUnless(os.environ.get("QS_PROBE_XLL"), "requires the optional live ribbon probe")
    def test_09_cashflow_paging_uses_the_real_pane(self):
        from async_excel import await_engine
        self.sheet.Range("AH1").Formula2 = '=QS.INSTRUMENT.SWAP("paged.swap","2026-02-02","2046-02-02","USD","SOFR",1000000,.04,0,"Actual360","Simple","Monthly","Monthly","Annual")'
        self.sheet.Range("AH2").Formula2 = '=QS.TRADE.CREATE("paged.trade",AH1,"2026-01-15","Receive")'
        await_engine(self.app, self.book)
        self.book.Names.Add("EnginePaged_Trade", "='" + self.sheet.Name + "'!$AH$2")
        path = Path(os.environ["QS_PROBE_XLL"]).resolve().parent
        (path / "selection.txt").write_text("EnginePaged_Trade\nCashflows\nmarket\npricer", encoding="utf-8")
        report = path / "probe-result.txt"
        report.unlink(missing_ok=True)
        self.app.Run("QS.PanelProbe.Run")
        deadline = time.monotonic() + 35
        while not report.exists() and time.monotonic() < deadline:
            time.sleep(.025)
        self.assertTrue(report.exists())
        result = report.read_text(encoding="utf-8-sig")
        self.assertTrue(result.startswith("PASS"), result)
        rows = int(next(line for line in result.splitlines() if line.startswith("Rows:")).split(":")[1])
        self.assertGreater(rows, 256)

if __name__ == "__main__":
    unittest.main()

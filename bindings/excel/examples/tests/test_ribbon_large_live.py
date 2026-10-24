"""Large-pane responsiveness regression; runs in its own Excel instance.

Build the companion with -p:QuantSupportLiveTests=true and set QS_PROBE_XLL
and QS_XLL_PATH to the matching XLLs in the same directory.
"""
import gc
import json
import math
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))


@unittest.skipUnless(os.name == "nt" and os.environ.get("QS_XLL_PATH")
                     and os.environ.get("QS_PROBE_XLL"),
                     "requires Windows Excel and the live ribbon probe")
class LargeRibbonTests(unittest.TestCase):
    def test_open_refresh_search_and_dispose_with_10000_contexts(self):
        from win32com.client import DispatchEx, gencache
        from async_excel import await_engine, retry_busy
        from create_origination_example import generate

        native = Path(os.environ["QS_XLL_PATH"]).resolve()
        companion = Path(os.environ["QS_PROBE_XLL"]).resolve()
        self.assertEqual(native.parent, companion.parent)
        report = companion.parent / "browser-refresh.txt"
        report.unlink(missing_ok=True)
        with tempfile.TemporaryDirectory(prefix="qs-ribbon-large-") as directory:
            path = generate(Path(directory) / "large.xlsx", layout="batch")
            app = gencache.EnsureDispatch(DispatchEx("Excel.Application"))
            app.DisplayAlerts = False
            anchor = app.Workbooks.Add()
            app.Calculation = -4135
            book = None
            try:
                self.assertTrue(app.RegisterXLL(str(native)))
                self.assertTrue(app.RegisterXLL(str(companion)))
                book = app.Workbooks.Open(str(path))
                status = await_engine(app, book)
                self.assertEqual(status["context_builds"], 10000)
                self.assertEqual(status["curve_builds"], 10000)
                prices = book.Worksheets("Origination").Range("G2:G10001").Value2
                self.assertTrue(all(isinstance(row[0], float) and math.isfinite(row[0]) for row in prices))
                throttle = app.RTD.ThrottleInterval
                app.Run("QS.PanelProbe.RefreshLargeBrowser")
                delays = []
                deadline = time.monotonic() + 45
                while time.monotonic() < deadline:
                    began = time.perf_counter()
                    retry_busy(lambda: setattr(book.Worksheets(1).Range("Z3"), "Value2", "responsive"),
                               time.monotonic() + 10)
                    delays.append(1000 * (time.perf_counter() - began))
                    output = report.read_text(encoding="utf-8-sig") if report.exists() else ""
                    if "PASS" in output or "FAIL" in output:
                        break
                    time.sleep(.025)
                self.assertIn("PASS", output, output)
                values = dict(line.split("=", 1) for line in output.splitlines() if "=" in line)
                self.assertEqual(int(values["objects"]), 40000)
                self.assertEqual(int(values["pricing_objects"]), 20000)
                self.assertEqual(int(values["contexts"]), 10000)
                for key in ("open_return_ms", "cached_refresh_ms", "changed_refresh_return_ms",
                            "pricing_popup_ms", "context_popup_ms", "pricing_search_ms", "context_search_ms"):
                    self.assertLess(float(values[key]), 1500, output)
                self.assertLess(max(delays), 1500, f"COM heartbeat blocked: {max(delays):.1f} ms\n{output}")
                metrics = book.Worksheets(1).Range("Z1")
                metrics.Formula2 = "=QS.ENGINE.STATUS()"
                metrics.Calculate()
                self.assertEqual(json.loads(metrics.Value2)["submitted"], status["submitted"],
                                 "browsing must not submit pricing jobs")
                self.assertEqual(app.Calculation, -4135)
                self.assertEqual(app.RTD.ThrottleInterval, throttle)
                metrics = None
                print(output + f"heartbeat_max_ms={max(delays):.1f}")
            finally:
                if book is not None:
                    retry_busy(lambda: book.Close(False), time.monotonic() + 30)
                retry_busy(lambda: anchor.Close(False), time.monotonic() + 30)
                retry_busy(app.Quit, time.monotonic() + 30)
                book = anchor = app = None
                gc.collect()


if __name__ == "__main__":
    unittest.main()

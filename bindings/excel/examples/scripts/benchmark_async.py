"""Live Windows Excel benchmark: distinct swaps at distinct origination dates.

Run with the bundled development Python (pywin32, xlsxwriter, psutil):
  python benchmark_async.py --xll PATH --count 10000 --workers 8 --output REPORT.json
Add --baseline for the pre-engine worksheet API. Load no other Excel add-ins.
The report separates the blocking definition pass, engine completion and RTD
display latency. CalculationState is never used as the engine completion barrier.
"""
import argparse
import hashlib
from importlib.metadata import version
import json
import math
import os
from pathlib import Path
import platform
import statistics
import threading
import time

import psutil
from win32com.client import DispatchEx, gencache
import win32process

from create_origination_example import generate
from async_excel import retry_busy


def percentile(values, p):
    return sorted(values)[min(len(values)-1, int(len(values)*p))] if values else None

def run(args):
    output = Path(args.output).resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    workbook = output.with_suffix(".xlsx")
    generate(workbook, args.count, args.baseline, args.layout)
    os.environ["QS_ENGINE_WORKERS"] = str(args.workers)
    # Generated COM wrappers retain DISPIDs when Excel temporarily rejects calls.
    # Dynamic dispatch can misreport that transient busy state as AttributeError.
    app = gencache.EnsureDispatch(DispatchEx("Excel.Application"))
    app.DisplayAlerts = False
    anchor = app.Workbooks.Add()
    app.Calculation = -4135
    xll = Path(args.xll).resolve()
    assert app.RegisterXLL(str(xll)), xll
    if not args.baseline:
        assert app.RegisterXLL(str(xll.with_name("QuantSupport.Ribbon64.xll")))
    process = psutil.Process(win32process.GetWindowThreadProcessId(app.Hwnd)[1])
    peaks = []
    stop = threading.Event()
    def sample():
        while not stop.wait(.02):
            try: peaks.append(process.memory_info().rss)
            except psutil.Error: return
    monitor = threading.Thread(target=sample, daemon=True)
    monitor.start()
    book = None
    report = {"count": args.count, "distinct_origination_dates": args.count, "context_count": args.count,
              "layout": args.layout,
              "product": "five-year receive-fixed USD SOFR swaps, effective seven days after origination", "workers": 0 if args.baseline else args.workers,
              "baseline": args.baseline, "cpu": platform.processor(), "logical_cpus": os.cpu_count(),
              "python_version": platform.python_version(), "pywin32_version": version("pywin32"), "psutil_version": version("psutil"),
              "ribbon_sha256": None if args.baseline else hashlib.sha256(xll.with_name("QuantSupport.Ribbon64.xll").read_bytes()).hexdigest(),
              "excel_version": app.Version, "excel_build": app.Build, "rtd_throttle_ms": app.RTD.ThrottleInterval,
              "native_sha256": hashlib.sha256(xll.read_bytes()).hexdigest(), "runs": {}}
    try:
        book = app.Workbooks.Open(str(workbook), UpdateLinks=0)
        report["force_full_calculation"] = book.ForceFullCalculation
        report["calculation_mode"] = app.Calculation
        print("opened", workbook, flush=True)
        sheet = book.Worksheets("Origination")
        status_cell = sheet.Range("Z1")
        if not args.baseline: status_cell.Formula2 = "=QS.ENGINE.STATUS()"
        # Manual mode is deliberately retained; completion refresh is explicit.
        def status():
            deadline = time.monotonic() + 10
            retry_busy(status_cell.Dirty, deadline)
            retry_busy(status_cell.Calculate, deadline)
            return json.loads(retry_busy(lambda: status_cell.Value2, deadline))
        def values():
            data = sheet.Range(f"G2:G{args.count+1}").Value2
            return [data] if args.count == 1 else [row[0] for row in data]
        def calculate(label, action):
            began = time.perf_counter()
            cpu_before = sum(process.cpu_times()[:2])
            action()
            definition_seconds = time.perf_counter() - began
            print(label, "definition pass", round(definition_seconds, 3), "seconds", flush=True)
            if not args.baseline:
                app.Run("QS._ENGINE.DELIVER")
            heartbeat = []
            ready_at = None
            deadline = began + args.timeout
            last_status = {}
            while time.perf_counter() < deadline:
                start = time.perf_counter()
                def write_heartbeat():
                    sheet.Range("Z3").Value2 = time.perf_counter()
                retry_busy(write_heartbeat, time.monotonic() + 30)
                heartbeat.append((time.perf_counter() - start)*1000)
                if not args.baseline:
                    last_status = status()
                    if last_status.get("pending", 1) == 0 and last_status.get("settled", True):
                        if ready_at is None:
                            ready_at = time.perf_counter()
                            print(label, "engine ready", round(ready_at-began, 3), "seconds", flush=True)
                        if last_status.get("pending_delivery", 1) == 0:
                            result = values()
                            break
                else:
                    ready_at = time.perf_counter()
                    result = values()
                    break
                time.sleep(.025)
            else: raise RuntimeError(f"engine timeout: {last_status}")
            failures = [(i+2, v) for i,v in enumerate(result) if type(v) not in (int, float) or not math.isfinite(v) or v < -1e8]
            if failures:
                raise AssertionError(f"failed valuations: {failures[:10]}, inputs: {sheet.Range('A'+str(failures[0][0])+':G'+str(failures[0][0])).Value2}, {last_status}")
            elapsed = time.perf_counter()-began
            data = {"definition_pass_seconds": definition_seconds, "engine_ready_seconds": ready_at-began,
                    "final_values_seconds": elapsed, "excel_cpu_seconds": sum(process.cpu_times()[:2])-cpu_before,
                    "heartbeat_com_ms_p50": statistics.median(heartbeat), "heartbeat_com_ms_p95": percentile(heartbeat,.95),
                    "heartbeat_com_ms_max": max(heartbeat), "metrics": last_status,
                    "pv_sum": sum(result), "first_pv": result[0], "last_pv": result[-1]}
            report["runs"][label] = data
            print(label, json.dumps(data), flush=True)
            return result
        cold = calculate("cold", app.CalculateFullRebuild)
        warm = calculate("warm", app.CalculateFull)
        assert cold == warm, "unchanged recalculation changed results"
        report["values"] = cold
        if not args.baseline:
            before = report["runs"]["cold"]["metrics"]["submitted"]
            after = report["runs"]["warm"]["metrics"]["submitted"]
            assert before == after, "warm recalculation submitted work"
        report["peak_excel_rss_mb"] = max(peaks, default=process.memory_info().rss)/1024**2
        if args.reference:
            reference = json.loads(Path(args.reference).read_text(encoding="utf-8"))["values"]
            differences = [abs(a-b) for a,b in zip(cold, reference)]
            assert len(reference) == len(cold)
            report["max_absolute_pv_difference"] = max(differences)
            assert max(differences) < 1e-7, report["max_absolute_pv_difference"]
        output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    finally:
        stop.set()
        monitor.join()
        try:
            if book is not None: retry_busy(lambda: book.Close(False), time.monotonic() + 30)
            retry_busy(lambda: anchor.Close(False), time.monotonic() + 30)
        finally: retry_busy(app.Quit, time.monotonic() + 30)
    print(f"report: {output}", flush=True)

if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--xll", required=True)
    p.add_argument("--output", required=True)
    p.add_argument("--count", type=int, default=10000)
    p.add_argument("--workers", type=int, default=8)
    p.add_argument("--timeout", type=float, default=900)
    p.add_argument("--baseline", action="store_true")
    p.add_argument("--layout", choices=("rows", "batch"), default="rows")
    p.add_argument("--reference")
    run(p.parse_args())

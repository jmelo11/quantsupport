"""Completion barrier for automation, including dependent stored-result formulas."""
import json
import time


def retry_busy(action, deadline):
    """Retry only Excel's transient COM busy responses within the caller's deadline."""
    from pywintypes import com_error
    while True:
        try:
            return action()
        except com_error as error:
            if error.hresult not in (-2147418111, -2147417846) or time.monotonic() >= deadline:
                raise
            time.sleep(.025)

def await_engine(excel, workbook, timeout=120, allow_errors=False):
    scratch = workbook.Worksheets(1).Range("Z1")
    old_formula = scratch.Formula2
    scratch.Formula2 = "=QS.ENGINE.STATUS()"
    deadline = time.monotonic() + timeout
    try:
        excel.CalculateFull()
        excel.Run("QS._ENGINE.DELIVER")
        while time.monotonic() < deadline:
            # STATUS is nonvolatile. Range.Calculate alone can return Excel's
            # cached value, so explicitly dirty this one cell before polling.
            retry_busy(scratch.Dirty, deadline)
            retry_busy(scratch.Calculate, deadline)
            status = json.loads(retry_busy(lambda: scratch.Value2, deadline))
            if status["pending"] == 0 and status.get("pending_delivery", 0) == 0 and status["settled"]:
                if status["errors"] and not allow_errors:
                    raise AssertionError(status["diagnostics"])
                return status
            time.sleep(.025)
        raise TimeoutError(f"QuantSupport completion timeout: {status}")
    finally:
        scratch.Formula2 = old_formula

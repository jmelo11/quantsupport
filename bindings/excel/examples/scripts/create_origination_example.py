"""Create a large, inspectable worksheet example; no Excel installation required."""
import argparse
from datetime import date, timedelta
from pathlib import Path

import xlsxwriter


def generate(path, count=10000, baseline=False, layout="rows"):
    if not 1 <= count <= 100000:
        raise ValueError("count must be between 1 and 100000")
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    book = xlsxwriter.Workbook(str(path))
    book.set_properties({"title": f"{count:,} valuations at origination", "author": "QuantSupport"})
    book.set_calc_mode("manual")
    title = book.add_format({"bold": True, "font_size": 22, "font_color": "#17365D"})
    heading = book.add_format({"bold": True, "font_color": "white", "bg_color": "#17365D", "text_wrap": True})
    body = book.add_format({"text_wrap": True, "valign": "top"})
    input_date = book.add_format({"font_color": "#1763A6", "num_format": "@"})
    input_number = book.add_format({"font_color": "#1763A6", "num_format": "#,##0"})
    input_rate = book.add_format({"font_color": "#1763A6", "num_format": "0.00%"})
    handle = book.add_format({"font_color": "#666666", "bg_color": "#F2F2F2"})
    pv = book.add_format({"num_format": "#,##0.00;[Red](#,##0.00)", "bg_color": "#E2F0D9"})
    start = book.add_worksheet("Start Here")
    start.hide_gridlines(2)
    start.set_column("A:A", 3)
    start.set_column("B:B", 28)
    start.set_column("C:H", 13)
    start.merge_range("B2:H3", f"{count:,} valuations at origination", title)
    last_date = date(1990, 1, 1) + timedelta(days=count - 1)
    notes = [
        (5, "Open and calculate", "Load quantsupport.xll and its matching QuantSupport.Ribbon64.xll in 64-bit Windows Excel. This workbook opens in Manual mode. Use QuantSupport > Refresh Pricing and wait for the pending results to finish."),
        (8, "What is being valued", f"{count:,} different five-year receive-fixed USD SOFR swaps, {count:,} distinct origination dates and {count:,} separate contexts. Every PV uses its own row's origination date, from 1990-01-01 through {last_date.isoformat()}."),
        (11, "Follow one row", "A: valuation/origination date. C: dated context. D: curve contribution owned by C. E: swap. F: trade anchored at origination. G: PV. Select any formula to inspect its actual inputs."),
        (14, "Synthetic market", "All dates use the same six synthetic calibration rates in K2:L7. Each context independently bootstraps its own dated SOFR curve. These are illustrative rates, not historical observations."),
        (17, "Editable assumptions", "Blue cells hold dates, notionals, fixed rates and market rates. Swap effective dates are seven days after origination to avoid requiring past fixings. Origination remains the valuation date."),
        (20, "Background results", "Blank prices in an uncalculated file and #N/A while pending are expected. Green cells show completed prices. Context handles stay stable when market inputs change. Manual mode is retained; Refresh Pricing also updates dependent formulas."),
        (23, "Benchmark this workbook", "Run examples/scripts/benchmark_async.py with --count 10000 and paths to the matching XLL and a JSON report. It measures cold and unchanged recalculation, context/curve builds, worksheet delivery, memory and COM responsiveness."),
    ]
    if layout == "batch" and not baseline:
        notes[2] = (11, "Follow one row", "A: valuation/origination date. C: dated context. D: owned curve contribution. E: swap. F: trade. G: PV. One QS.EVALUATE.RANGE formula in G2 pairs all C and F rows and spills the prices in row order; Rust distributes the dated markets across workers.")
    for row, label, description in notes:
        start.write(row - 1, 1, label, book.add_format({"bold": True, "font_color": "#17365D"}))
        start.merge_range(row, 1, row + 1, 7, description, body)
        start.set_row(row, 25)
        start.set_row(row + 1, 25)
    start.write_url("B27", "internal:'Origination'!A1", string=f"Open the {count:,} valuation rows →")
    start.write("B29", "Completed numeric PVs")
    start.write_formula("C29", f"=COUNT(Origination!G2:G{count+1})", None, "")
    start.write("B30", "Sum of PVs (USD)")
    start.write_formula("C30", f"=SUM(Origination!G2:G{count+1})", pv, "")
    start.write("B32", "Engine status")
    if not baseline:
        start.merge_range("B33:H36", "", body)
        start.write_formula("B33", "=QS.ENGINE.STATUS()", body, "")
    sheet = book.add_worksheet("Origination")
    sheet.hide_gridlines(2)
    sheet.freeze_panes(1, 2)
    sheet.set_row(0, 32)
    sheet.write_row(0, 0, ["Origination / as of", "Maturity", "Context", "Market contribution", "Instrument", "Trade", "PV at origination (USD)", "Effective date", "Notional (USD)", "Fixed rate", "Calibration instrument", "Market rate"], heading)
    quotes = ["FixedRateDeposit_USD_SOFR_1D"] + [f"OIS_USD_SOFR_{n}Y" for n in (1, 2, 5, 10, 20)]
    for i, quote in enumerate(quotes, 1):
        sheet.write(i, 10, quote, input_date)
        sheet.write(i, 11, .03 + .0005 * i, input_rate)
    first = date(1990, 1, 1)
    for i in range(count):
        row = i + 2
        origination = first + timedelta(days=i)
        effective = origination + timedelta(days=7)
        try:
            maturity = effective.replace(year=effective.year + 5)
        except ValueError:
            maturity = effective.replace(year=effective.year + 5, day=28)
        sheet.write(i + 1, 0, origination.isoformat(), input_date)
        sheet.write(i + 1, 1, maturity.isoformat(), input_date)
        sheet.write(i + 1, 7, effective.isoformat(), input_date)
        sheet.write(i + 1, 8, 1000000 + 100 * i, input_number)
        sheet.write(i + 1, 9, .035 + .0001 * (i % 50), input_rate)
        sheet.write_formula(i + 1, 2, f'=QS.CONTEXT.CREATE("market.{i}",A{row},"USD","SOFR")', handle, "")
        sheet.write_formula(i + 1, 3, f'=QS.CONTEXT.ADD.CURVE(C{row},"SOFR",$K$2:$K$7,$L$2:$L$7,,,"Actual360","LogLinear",TRUE)', handle, "")
        sheet.write_formula(i + 1, 4, f'=QS.INSTRUMENT.SWAP("swap.{i}",C{row},H{row},B{row},"USD","SOFR",I{row},J{row},0,"Actual360","Simple","Annual","Annual","Annual")', handle, "")
        sheet.write_formula(i + 1, 5, f'=QS.TRADE.CREATE("trade.{i}",C{row},E{row},A{row},"Receive")', handle, "")
        extra = f',"Value",D{row}' if baseline else ''
        if layout == "batch" and not baseline:
            sheet.write_blank(i + 1, 6, None, pv)
        else:
            sheet.write_formula(i + 1, 6, f'=QS.EVALUATE(C{row},F{row}{extra})', pv, "")
    if layout == "batch" and not baseline:
        sheet.write_dynamic_array_formula(1, 6, 1, 6, f'=QS.EVALUATE.RANGE(C2:C{count+1},F2:F{count+1})', pv, "")
    sheet.autofilter(0, 0, count, 9)
    sheet.set_column("A:B", 16)
    sheet.set_column("C:F", 23)
    sheet.set_column("G:G", 23)
    sheet.set_column("H:H", 16)
    sheet.set_column("I:J", 17)
    sheet.set_column("K:K", 34)
    sheet.set_column("L:L", 16)
    book.close()
    return path


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--count", type=int, default=10000)
    parser.add_argument("--layout", choices=("rows", "batch"), default="rows")
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "workbooks/origination_10000_valuations.xlsx")
    args = parser.parse_args()
    print(generate(args.output, args.count, layout=args.layout))

"""Generate the Excel example workbooks with Python.

Install: python -m pip install -r bindings/excel/examples/requirements.txt
Run: python bindings/excel/examples/create_examples.py
Generation works without Excel or the XLL. Open the workbooks in Windows Excel
with quantsupport.xll loaded and press Ctrl+Alt+F9 to calculate their formulas.
Use --xll-path on Windows for optional native Excel validation (requires pywin32).
"""

import argparse
from datetime import datetime
import json
import math
from pathlib import Path
import sys
from tempfile import TemporaryDirectory

import xlsxwriter


def formula(name, *arguments):
    return "=" + name + "(" + ",".join(arguments) + ")"

def historical_cells():
    cells = {
        "A1": "Market reference date", "A2": "Observation timestamp (UTC)",
        "A4": "Pillar", "B4": "Zero rate: first market", "C4": "Zero rate: second market",
        "A5": "0D", "A6": "1Y", "A7": "10Y", "A9": "Quote store (reference date)",
        "A10": "Curve configuration store", "A11": "Explicit zero-rate configuration",
        "A12": "Base market context", "A13": "Captured market snapshot",
        "A15": "Complete snapshot archive", "A17": "One unchanged swap trade",
        "A19": "Requested timestamp", "A20": "Native snapshot selector",
        "A21": "Selected historical context", "A22": "Actual matched timestamp",
        "A23": "Actual valuation date", "A24": "Swap present value",
        "A25": "Construction quote side",
        "A27": "Edit B5:C7 to reprice. Rates are decimals; cells display percentages.",
        "A28": "The swap starts after both valuation dates, so this example requires no historical fixings.",
        "A29": "Change Exact to Previous in a selector to query a timestamp absent from the archive.",
        "A30": "Previous preserves the matched market's valuation date; it does not roll curves to the requested date.",
        "B15": '=QS.SNAPSHOTS.CREATE("history_archive",B13:C13)',
        "B17": '=QS.TRADE.SWAP("history_swap","2026-01-15","2026-01-19","2031-01-19","USD","SOFR",1000000,0.04,0,"Receive","Actual360","Simple","Semiannual","Semiannual","Quarterly")',
    }
    for column, day, rates in [("B", 15, [0.040, 0.039, 0.038]), ("C", 16, [0.041, 0.040, 0.039])]:
        date = f"2026-01-{day}"
        cells[f"{column}1"] = date
        cells[f"{column}2"] = date + "T16:00:00Z"
        for row, rate in enumerate(rates, 5):
            cells[f"{column}{row}"] = rate
        cells[f"{column}9"] = formula("QS.QUOTES.CREATE", f'"history_quotes_"&{column}$1', f"{column}$1")
        cells[f"{column}10"] = formula("QS.CURVE.CONFIGS.CREATE", f'"history_curves_"&{column}$1')
        cells[f"{column}11"] = formula("QS.CURVE.CONFIGS.ADD.ZERO", f"{column}10", '"SOFR"', '"Actual365"', '"Linear"', "FALSE", "$A$5:$A$7", f"{column}5:{column}7", '"Continuous"', '"Annual"')
        cells[f"{column}12"] = formula("QS.CONTEXT.CREATE", f'"history_market_"&{column}$1', f"{column}9", f"{column}11", *([""] * 9), '"USD"', '"SOFR"')
        cells[f"{column}13"] = formula("QS.SNAPSHOT.FROM.CONTEXT", f'"history_snapshot_"&{column}$1', f"{column}2", f"{column}12")
        cells[f"{column}19"] = date + "T16:00:00Z"
        cells[f"{column}20"] = formula("QS.SNAPSHOT.SELECTOR", f'"history_selector_"&{column}$1', f"{column}19", '"Exact"', '"Mid"')
        cells[f"{column}21"] = formula("QS.CONTEXT.FROM.SNAPSHOT", f'"history_selected_"&{column}$1', "$B$15", f"{column}20", "")
        cells[f"{column}22"] = formula("QS.CONTEXT.SNAPSHOT.TIMESTAMP", f"{column}21")
        cells[f"{column}23"] = formula("QS.CONTEXT.EVALUATION.DATE", f"{column}21")
        cells[f"{column}24"] = formula("QS.PRICE", "$B$17", f"{column}21")
        cells[f"{column}25"] = formula("QS.CONTEXT.QUOTE.LEVEL", f"{column}21")
    return cells


def formats(workbook):
    common = {"font_name": "Calibri", "font_size": 11, "text_wrap": True, "valign": "top"}
    variants = {
        "text": {},
        "title": {"bold": True, "font_size": 18, "font_color": "white", "bg_color": "#003070"},
        "header": {"bold": True, "font_color": "white", "bg_color": "#2C5AA0", "border": 1},
        "section": {"bold": True, "font_color": "#003070", "bg_color": "#CFD9E8"},
        "input": {"bg_color": "#D6E4FC", "border": 1},
        "input_rate": {"bg_color": "#D6E4FC", "border": 1, "num_format": "0.0000%"},
        "input_number": {"bg_color": "#D6E4FC", "border": 1, "num_format": "#,##0.00"},
        "rate": {"num_format": "0.0000%"},
        "number": {"num_format": "#,##0.00"},
        "factor": {"num_format": "0.000000"},
    }
    return {name: workbook.add_format(common | properties) for name, properties in variants.items()}


def write(sheet, cell, value, cell_format=None):
    if isinstance(value, str) and value.startswith("="):
        if value.startswith("=QS."):
            # Match Excel's Formula2 storage for XLL functions and preserve spills.
            sheet.write_dynamic_array_formula(cell, value.replace("=QS.", "=_xll.QS.", 1), cell_format)
        else:
            sheet.write_formula(cell, value, cell_format)
    else:
        sheet.write(cell, value, cell_format)


def heading(sheet, cell_range, text, cell_format):
    sheet.merge_range(cell_range, text, cell_format)
    if cell_range.startswith("A1:"):
        sheet.set_row(0, 28)


def new_workbook(path):
    workbook = xlsxwriter.Workbook(path)
    workbook.set_properties({"title": "QuantSupport Excel examples", "created": datetime(2026, 1, 1)})
    workbook.set_calc_mode("auto")
    return workbook


def create_historical(path):
    with new_workbook(path) as workbook:
        style = formats(workbook)
        sheet = workbook.add_worksheet("Historical Pricing")
        sheet.hide_gridlines(2)
        sheet.freeze_panes(4, 1)
        sheet.set_column("A:A", 38, style["text"])
        sheet.set_column("B:C", 55, style["text"])
        for address, value in historical_cells().items():
            row = int(address[1:])
            kind = "header" if row in (1, 4, 19) else "text"
            if address[0] != "A":
                if row in (5, 6, 7):
                    kind = "input_rate"
                elif row == 24:
                    kind = "number"
                elif row in (1, 2, 19):
                    kind = "input"
            write(sheet, address, value, style[kind])


SOFR_QUOTES = [
    ("FixedRateDeposit_USD_SOFR_1D", 0.04500),
    ("OIS_USD_SOFR_1Y", 0.04837), ("OIS_USD_SOFR_2Y", 0.04528),
    ("OIS_USD_SOFR_3Y", 0.04260), ("OIS_USD_SOFR_5Y", 0.04077),
    ("OIS_USD_SOFR_7Y", 0.04033), ("OIS_USD_SOFR_10Y", 0.03898),
    ("OIS_USD_SOFR_30Y", 0.03738),
]
ESTR_QUOTES = [
    ("FixedRateDeposit_EUR_ESTR_1D", 0.02900),
    ("OIS_EUR_ESTR_3M", 0.02880), ("OIS_EUR_ESTR_6M", 0.02800),
    ("OIS_EUR_ESTR_1Y", 0.02650), ("OIS_EUR_ESTR_2Y", 0.02500),
    ("OIS_EUR_ESTR_3Y", 0.02450), ("OIS_EUR_ESTR_4Y", 0.02430),
    ("OIS_EUR_ESTR_5Y", 0.02420), ("OIS_EUR_ESTR_7Y", 0.02450),
    ("OIS_EUR_ESTR_10Y", 0.02520), ("OIS_EUR_ESTR_15Y", 0.02580),
    ("OIS_EUR_ESTR_20Y", 0.02550), ("OIS_EUR_ESTR_30Y", 0.02400),
]


def create_swap(path):
    with new_workbook(path) as workbook:
        style = formats(workbook)
        sheets = {name: workbook.add_worksheet(name) for name in (
            "README", "Curve Setup", "Trade Inputs", "Pricing", "Curve Results", "Outputs", "Function Help",
        )}
        for sheet in sheets.values():
            sheet.hide_gridlines(2)
            sheet.set_column("A:N", 18, style["text"])
            sheet.freeze_panes(3, 0)

        readme = sheets["README"]
        heading(readme, "A1:F1", "QuantSupport: SOFR and ESTR Curve Construction", style["title"])
        heading(readme, "A3:F3", "How to use this workbook", style["section"])
        instructions = {
            4: "1. Build and load bindings/excel/dist/quantsupport.xll in 64-bit Windows Excel.",
            5: "2. Open this workbook after the add-in is loaded, then press Ctrl+Alt+F9 once.",
            6: "3. Change the blue SOFR or ESTR quotes on Curve Setup, or edit Trade Inputs. Curves and prices recalculate.",
            7: "4. Pricing shows the object chain. Curve Results shows both curves. Outputs shows cashflows and sensitivities.",
            10: "Curve Setup contains USD SOFR and EUR ESTR deposit/OIS quotes plus interpolation, day count, extrapolation, and pillars.",
            11: "The QuoteStore chain loads SOFR from A6:A13, then ESTR from H6:H18. The configuration chain adds both curves.",
            12: "QS.CONTEXT.CREATE bootstraps both curves from worksheet handles. No JSON or external curve file is used.",
            13: "Pressing F9 with unchanged inputs reuses every object handle. Edited quotes or trade inputs advance revisions.",
            15: "Excel's Function Arguments dialog shows guidance for every parameter.",
            16: 'Use =QS.HELP("QS.CURVE.CONFIGS.ADD") or =QS.FUNCTIONS.DETAILS() for an in-sheet reference.',
        }
        for row, text in instructions.items():
            readme.write_string(f"A{row}", text, style["text"])
            readme.set_row(row - 1, 32)
        heading(readme, "A9:F9", "Both curves are configured in this workbook", style["section"])
        heading(readme, "A14:F14", "Function guidance", style["section"])
        readme.set_column("A:A", 115)

        curve = sheets["Curve Setup"]
        heading(curve, "A1:M1", "USD SOFR and EUR ESTR curve setup", style["title"])
        curve.write("A3", "Valuation date")
        curve.write("B3", "2026-02-24", style["input"])
        curve.write("E3", "Initial quote store")
        write(curve, "F3", '=QS.QUOTES.CREATE("SwapExample.Quotes",B3)')
        for columns, quotes, upstream in [("ABC", SOFR_QUOTES, "$F$3"), ("HIJ", ESTR_QUOTES, "$C$13")]:
            identifier_column, rate_column, handle_column = columns
            for column, label in zip(columns, ["Quote identifier / calibration instrument", "Mid", "QuoteStore revision"]):
                curve.write(f"{column}5", label, style["header"])
            for row, (identifier, rate) in enumerate(quotes, 6):
                curve.write(f"{identifier_column}{row}", identifier)
                curve.write(f"{rate_column}{row}", rate, style["input_rate"])
                previous = upstream if row == 6 else f"{handle_column}{row - 1}"
                write(curve, f"{handle_column}{row}", formula("QS.QUOTES.SET", previous, f"{identifier_column}{row}", f"{rate_column}{row}", "", ""))
        for label_column, value_column, index in [("E", "F", "SOFR"), ("L", "M", "ESTR")]:
            curve.write(f"{label_column}5", "Curve parameter", style["header"])
            curve.write(f"{value_column}5", "Value / handle", style["header"])
            for row, label, value in [(6, "Market index", index), (7, "Day counter", "Actual360"), (8, "Interpolator", "LogLinear"), (9, "Enable extrapolation", True)]:
                curve.write(f"{label_column}{row}", label)
                curve.write(f"{value_column}{row}", value, style["input"])
        for cell, value in {
            "E10": "Configuration store", "F10": '=QS.CURVE.CONFIGS.CREATE("SwapExample.Curves")',
            "E11": "Configured SOFR curve", "F11": "=QS.CURVE.CONFIGS.ADD(F10,F6,F7,F8,F9,A6:A13)",
            "L10": "SOFR configuration input", "M10": "=F11",
            "L11": "Configured ESTR curve", "M11": "=QS.CURVE.CONFIGS.ADD(M10,M6,M7,M8,M9,H6:H18)",
        }.items():
            write(curve, cell, value)
        for columns, width in [("A:A", 42), ("B:B", 14), ("C:C", 48), ("D:D", 3), ("E:E", 24), ("F:F", 50), ("G:G", 3), ("H:H", 42), ("I:I", 14), ("J:J", 48), ("K:K", 3), ("L:L", 24), ("M:M", 50)]:
            curve.set_column(columns, width, style["text"])

        inputs = sheets["Trade Inputs"]
        heading(inputs, "A1:D1", "Swap inputs", style["title"])
        inputs.write("A3", "Parameter", style["header"])
        inputs.write("B3", "Value", style["header"])
        values = [
            ("Notional", 10000000), ("Fixed coupon", 0.0425),
            ("Start date", "2026-02-24"), ("Maturity date", "2031-02-24"),
            ("Currency", "USD"), ("Market index", "SOFR"), ("Side", "Receive"),
            ("Day counter", "Actual360"), ("Coupon compounding", "Simple"),
            ("Rate frequency", "Semiannual"), ("Fixed-leg frequency", "Semiannual"),
            ("Floating-leg frequency", "Quarterly"),
        ]
        for row, (label, value) in enumerate(values, 4):
            inputs.write(f"A{row}", label)
            kind = "input_number" if row == 4 else "input_rate" if row == 5 else "input"
            inputs.write(f"B{row}", value, style[kind])
        inputs.write("D3", "Rates are decimals: 0.03 = 3%. Dates are ISO text (YYYY-MM-DD).")
        for columns, width in [("A:A", 27), ("B:B", 23), ("D:D", 42)]:
            inputs.set_column(columns, width, style["text"])

        pricing = sheets["Pricing"]
        heading(pricing, "A1:D1", "Pricing chain", style["title"])
        for cell, label in [("A3", "Step"), ("B3", "Result"), ("C3", "Purpose")]:
            pricing.write(cell, label, style["header"])
        steps = [
            ("1. Quote store", "='Curve Setup'!J18", "Latest revision after loading SOFR and ESTR quotes"),
            ("2. Curve configurations", "='Curve Setup'!M11", "Both curves' conventions and pillar ranges"),
            ("3. Pricing context", formula("QS.CONTEXT.CREATE", '"SwapExample.Context"', "B4", "B5", *([""] * 9), '"USD"', '"SOFR"'), "Bootstraps the configured curves"),
            ("4. Swap trade", '=QS.TRADE.SWAP("SwapExample.Swap",\'Curve Setup\'!B3,\'Trade Inputs\'!B6,\'Trade Inputs\'!B7,\'Trade Inputs\'!B8,\'Trade Inputs\'!B9,\'Trade Inputs\'!B4,\'Trade Inputs\'!B5,0,\'Trade Inputs\'!B10,\'Trade Inputs\'!B11,\'Trade Inputs\'!B12,\'Trade Inputs\'!B13,\'Trade Inputs\'!B14,\'Trade Inputs\'!B15)', "Receive-fixed USD SOFR swap"),
            ("5. Present value", "=QS.PRICE(B7,B6)", "Swap NPV in USD"),
            ("6. Fair fixed rate", "=QS.FAIR.RATE(B7,B6)", "Par coupon implied by the curve"),
            ("7. SOFR curve", "=QS.CURVE.GET(B6,'Trade Inputs'!B9)", "Reusable bootstrapped SOFR curve handle"),
            ("8. SOFR maturity discount", "=QS.CURVE.DISCOUNT(B10,'Trade Inputs'!B7)", "SOFR discount factor at maturity"),
            ("9. ESTR curve", '=QS.CURVE.GET(B6,"ESTR")', "Reusable bootstrapped ESTR curve handle"),
            ("10. ESTR maturity discount", "=QS.CURVE.DISCOUNT(B12,'Trade Inputs'!B7)", "ESTR discount factor at maturity"),
        ]
        for row, (label, expression, purpose) in enumerate(steps, 4):
            pricing.write(f"A{row}", label)
            kind = "number" if row == 8 else "rate" if row == 9 else "factor" if row in (11, 13) else "text"
            write(pricing, f"B{row}", expression, style[kind])
            pricing.write(f"C{row}", purpose)
        for columns, width in [("A:A", 24), ("B:B", 58), ("C:C", 40)]:
            pricing.set_column(columns, width, style["text"])
        pricing.activate()
        pricing.set_selection("A4")

        results = sheets["Curve Results"]
        heading(results, "A1:N1", "Bootstrapped SOFR and ESTR discount curves", style["title"])
        for area, cell, index_cell, label in [("A3:E3", "A4", "B10", "SOFR curve nodes"), ("G3:K3", "G4", "B12", "ESTR curve nodes")]:
            heading(results, area, label, style["section"])
            write(results, cell, f'=QS.CURVE.TABLE(Pricing!{index_cell},"Simple","Annual")')
        for columns, width, kind in [("A:A", 38, "text"), ("B:B", 16, "text"), ("C:C", 20, "factor"), ("D:E", 20, "rate"), ("G:G", 38, "text"), ("H:H", 16, "text"), ("I:I", 20, "factor"), ("J:K", 20, "rate"), ("M:M", 22, "text"), ("N:N", 58, "text")]:
            results.set_column(columns, width, style[kind])
        heading(results, "M3:N3", "Point queries", style["section"])
        queries = [
            ("SOFR curve handle", "=Pricing!B10", "text"),
            ("Discount date", "='Trade Inputs'!B7", "text"),
            ("SOFR discount factor", "=QS.CURVE.DISCOUNT(Pricing!B10,N5)", "factor"),
            ("ESTR curve handle", "=Pricing!B12", "text"),
            ("ESTR discount factor", "=QS.CURVE.DISCOUNT(Pricing!B12,N5)", "factor"),
            ("Forward start", "='Trade Inputs'!B6", "text"),
            ("Forward end", "='Trade Inputs'!B7", "text"),
            ("SOFR forward rate", '=QS.CURVE.FORWARD(Pricing!B10,N9,N10,"Simple","Annual")', "rate"),
            ("ESTR forward rate", '=QS.CURVE.FORWARD(Pricing!B12,N9,N10,"Simple","Annual")', "rate"),
        ]
        for row, (label, expression, kind) in enumerate(queries, 4):
            results.write(f"M{row}", label)
            write(results, f"N{row}", expression, style[kind])

        outputs = sheets["Outputs"]
        heading(outputs, "A1:L1", "Swap outputs", style["title"])
        heading(outputs, "A3:I3", "Cashflows", style["section"])
        write(outputs, "A4", "=QS.CASHFLOWS(Pricing!B7,Pricing!B6)")
        heading(outputs, "K3:L3", "Sensitivities", style["section"])
        write(outputs, "K4", "=QS.SENSITIVITIES(Pricing!B7,Pricing!B6)")

        help_sheet = sheets["Function Help"]
        heading(help_sheet, "A1:H1", "Live worksheet-function help", style["title"])
        for area, cell, label, function_name in [("A3:B3", "A4", "Curve construction", "QS.CURVE.CONFIGS.ADD"), ("D3:E3", "D4", "Swap construction", "QS.TRADE.SWAP"), ("G3:H3", "G4", "Curve inspection", "QS.CURVE.TABLE")]:
            heading(help_sheet, area, label, style["section"])
            write(help_sheet, cell, f'=QS.HELP("{function_name}")')
        for columns, width in [("A:A", 25), ("B:B", 76), ("D:D", 25), ("E:E", 76), ("G:G", 25), ("H:H", 76)]:
            help_sheet.set_column(columns, width, style["text"])


def study_sheets(workbook, names, title, notes):
    style = formats(workbook)
    sheets = {name: workbook.add_worksheet(name) for name in ("README", *names)}
    for sheet in sheets.values():
        sheet.hide_gridlines(2)
        sheet.freeze_panes(4, 0)
        sheet.set_column("A:L", 22, style["text"])
    readme = sheets["README"]
    readme.set_column("A:A", 115, style["text"])
    heading(readme, "A1:F1", title, style["title"])
    for row, note in enumerate([
        "Load quantsupport.xll in 64-bit Windows Excel, then open this workbook and press Ctrl+Alt+F9.",
        "Blue cells are editable inputs. Rates and spreads are annual decimals; dates are ISO text.",
        *notes,
        "These are illustrative assumptions, following the questions explored in the Python notebook studies.",
        "Table functions end with transpose,headers. Defaults are FALSE,TRUE; TRUE,FALSE transposes without headers.",
    ], 4):
        readme.write(f"A{row}", note)
        readme.set_row(row - 1, 34)
    return style, sheets


def zero_market(sheet, style, prefix, currency, index):
    """Editable explicit-zero inputs, with native pillar sensitivities."""
    for cell, value in {
        "A3": "Market reference date", "B3": "2026-02-24",
        "A5": "Pillar", "B5": "Continuous zero rate",
        "E3": "Quote store", "F3": f'=QS.QUOTES.CREATE("{prefix}.Quotes",B3)',
        "E4": "Configuration store", "F4": f'=QS.CURVE.CONFIGS.CREATE("{prefix}.Curves")',
        "E6": "Discount curve configuration",
        "F6": f'=QS.CURVE.CONFIGS.ADD.ZERO(F4,"{index}","Actual360","Linear",TRUE,A6:A10,B6:B10,"Continuous","Annual")',
        "E11": "Initialized pricing context",
        "F11": formula("QS.CONTEXT.CREATE", f'"{prefix}.Context"', "F3", "F6", *([""] * 9), f'"{currency}"', f'"{index}"'),
        "E12": "Discount curve handle", "F12": f'=QS.CURVE.GET(F11,"{index}")',
        "A14": "Change a zero-rate pillar and inspect prices, cashflows, and raw sensitivities.",
    }.items():
        write(sheet, cell, value, style["input"] if cell == "B3" else style["text"])
    for row, tenor, rate in [(6, "0D", 0.03), (7, "5Y", 0.03), (8, "10Y", 0.032), (9, "30Y", 0.034), (10, "35Y", 0.035)]:
        sheet.write(f"A{row}", tenor, style["input"])
        sheet.write(f"B{row}", rate, style["input_rate"])
    sheet.set_column("E:E", 32)
    sheet.set_column("F:F", 58)


def create_loans(path):
    with new_workbook(path) as workbook:
        style, sheets = study_sheets(workbook,
            ("Market", "Loan Inputs", "Comparison", "Cashflows", "Risk", "Layouts"),
            "Loans: monthly payments, prepayment, borrower credit, and funding", [
                "Compare French, bullet, and 25% balloon principal; then floating and introductory-fixed coupons.",
                "The lender's price is future receipt PV. Net origination value subtracts the original advance.",
                "CPR reduces future principal and interest; borrower survival and recovery use an illustrative CDS proxy.",
                "Funding discount factors multiply EURIBOR12m by a separate treasury-spread curve.",
                "FairRate solves a uniform fixed coupon or floating margin at origination. Introductory/bounded schedules omit this unsupported solve.",
                "Cashflows include scheduled receipts, prepayments, and recovery in legs 0, 1, and 2. Bounded coupons exclude option time value.",
            ])
        market = sheets["Market"]
        zero_market(market, style, "Loans", "EUR", "ESTR")
        market.write("C5", "EURIBOR12m zero", style["header"])
        market.write("D5", "Funding zero spread", style["header"])
        for row, rate, spread in [(6, .04, .005), (7, .037, .01), (8, .04, .011), (9, .0415, .014), (10, .0415, .015)]:
            market.write(f"C{row}", rate, style["input_rate"])
            market.write(f"D{row}", spread, style["input_rate"])
        for cell, value in {
            "E7": "Projection configuration", "F7": '=QS.CURVE.CONFIGS.ADD.ZERO(F6,"EURIBOR12m","Actual360","Linear",TRUE,A6:A10,C6:C10,"Continuous","Annual")',
            "E8": "Funding spread configuration", "F8": '=QS.CURVE.CONFIGS.ADD.ZERO(F7,"FUNDINGSPREAD","Actual360","Linear",TRUE,A6:A10,D6:D10,"Continuous","Annual")',
            "E9": "Composite funding configuration", "F9": '=QS.CURVE.CONFIGS.ADD.COMPOSITE(F8,"FUNDING","EURIBOR12m","FUNDINGSPREAD")',
            "H5": "Borrower CDS quote", "I5": "Spread", "J5": "Quote store revision",
            "E10": "Borrower credit configuration", "F10": '=QS.CREDIT.CURVE.CONFIGS.FROM.JSON("Loans.Credit",H16)',
            "H15": "Native credit configuration JSON (illustrative borrower proxy)",
        }.items():
            write(market, cell, value)
        credit_quotes = []
        for row, (tenor, spread) in enumerate(zip(("1Y", "3Y", "5Y", "10Y", "20Y", "30Y", "35Y"), (.005, .007, .009, .011, .014, .016, .017)), 6):
            identifier = f"Cds_BORROWER_EUR_{tenor}"
            credit_quotes.append(identifier)
            market.write(f"H{row}", identifier)
            market.write(f"I{row}", spread, style["input_rate"])
            previous = "$F$3" if row == 6 else f"J{row-1}"
            write(market, f"J{row}", f"=QS.QUOTES.SET({previous},H{row},I{row},,)")
        market.write_string("H16", json.dumps([{"market_index": {"Credit": "BORROWER"}, "currency": "EUR", "discount_index": "ESTR", "recovery": .4, "quotes": credit_quotes}]), style["input"])
        write(market, "F11", formula("QS.CONTEXT.CREATE", '"Loans.Context"', "J12", "F9", "F10", *([""] * 8), '"EUR"', '"ESTR"'))
        market.set_column("H:H", 42)
        market.set_column("J:J", 58)

        inputs = sheets["Loan Inputs"]
        for row, (label, value) in enumerate([
            ("Principal (EUR)", 250000), ("Fixed coupon", .06), ("Floating margin", .01),
            ("Start date", "2026-02-24"), ("Maturity date", "2056-02-24"),
            ("Payment frequency", "Monthly"), ("Day counter", "Actual360"), ("Currency", "EUR"),
        ], 4):
            inputs.write(f"A{row}", label)
            inputs.write(f"B{row}", value, style["input_rate"] if row in (5, 6) else style["input"])
        inputs.set_column("A:A", 30)
        inputs.write("A14", "Floating principal is agreed at a 5% initial all-in rate; annual resets use EURIBOR12m metadata.")
        inputs.write("A15", "The default zero fixing lag makes the first observation today's projected fixing; no historical fixing is required.")

        comparison = sheets["Comparison"]
        for column, label in enumerate(("Case", "Coupon / spread", "Projection index", "Repayment", "Options JSON", "Loan trade", "Receipt PV (EUR)", "PV less advance", "Fair coupon / spread")):
            comparison.write(3, column, label, style["header"])
        cases = [
            ("French", False, "French", {}), ("Bullet", False, "Bullet", {}),
            ("25% balloon", False, "Balloon", {"balloon_fraction": .25}),
            ("Floating", True, "French", {"amortization_rate": .05}),
            ("5-year introductory fixed", True, "French", {"introductory_periods": 60, "introductory_rate": .04}),
            ("Bounded floating", True, "French", {"amortization_rate": .05, "floor": .02, "cap": .045}),
            ("15% annual CPR", False, "French", {"prepayment": {"Cpr": .15}}),
            ("Borrower credit", False, "French", {"credit_index": {"Credit": "BORROWER"}, "recovery": .4}),
            ("Funded fixed", False, "French", {"discount_index": {"Other": "FUNDING"}}),
            ("Funded floating", True, "French", {"amortization_rate": .05, "discount_index": {"Other": "FUNDING"}}),
        ]
        for row, (label, floating, repayment, options) in enumerate(cases, 5):
            comparison.write(f"A{row}", label)
            write(comparison, f"B{row}", "='Loan Inputs'!B6" if floating else "='Loan Inputs'!B5", style["rate"])
            comparison.write(f"C{row}", "EURIBOR12m" if floating else "", style["input"])
            comparison.write(f"D{row}", repayment, style["input"])
            comparison.write_string(f"E{row}", json.dumps(options), style["input"])
            write(comparison, f"F{row}", formula("QS.TRADE.LOAN", f'"Loans.Case{row}"', "Market!$B$3", "'Loan Inputs'!$B$7", "'Loan Inputs'!$B$8", "'Loan Inputs'!$B$11", "'Loan Inputs'!$B$4", f"B{row}", f"C{row}", f"D{row}", "'Loan Inputs'!$B$9", "'Loan Inputs'!$B$10", '"Receive"', f"E{row}"))
            write(comparison, f"G{row}", f"=QS.PRICE(F{row},Market!$F$11)", style["number"])
            write(comparison, f"H{row}", f"=G{row}-'Loan Inputs'!$B$4", style["number"])
            if row not in (9, 10):
                write(comparison, f"I{row}", f"=QS.FAIR.RATE(F{row},Market!$F$11)", style["rate"])
            else:
                comparison.write(f"I{row}", "Unsupported for mixed/bounded coupons")
        comparison.set_column("A:A", 31)
        comparison.set_column("E:E", 70)
        comparison.set_column("F:F", 48)
        comparison.activate()

        for cell, title, expression in [
            ("A4", "French contractual receipts", "=QS.CASHFLOWS(Comparison!F5,Market!F11)"),
            ("K4", "Expected receipts with prepayment", "=QS.CASHFLOWS(Comparison!F11,Market!F11)"),
        ]:
            sheets["Cashflows"].write(cell.replace("4", "2"), title, style["section"])
            write(sheets["Cashflows"], cell, expression)
        for cell, label, trade in [("A4", "Funding risk: raw EUR per unit pillar", "F13"), ("E4", "Floating funding risk", "F14")]:
            sheets["Risk"].write(cell.replace("4", "2"), label, style["section"])
            write(sheets["Risk"], cell, f"=QS.SENSITIVITIES(Comparison!{trade},Market!F11)")
        sheets["Risk"].write("A25", "Multiply a decimal-rate derivative by 0.0001 for an approximate +1 bp value change.")
        layouts = sheets["Layouts"]
        layouts.write("A2", "Funding risk transposed, headers hidden", style["section"])
        write(layouts, "A4", "=QS.SENSITIVITIES(Comparison!F13,Market!F11,TRUE,FALSE)")
        layouts.write("A9", "Same risk with headers (row labels after transposing)", style["section"])
        write(layouts, "A11", "=QS.SENSITIVITIES(Comparison!F13,Market!F11,TRUE,TRUE)")
        layouts.write("A16", "First year payment dates as a row (list, no headers)", style["section"])
        write(layouts, "A18", '=QS.SCHEDULE("2026-02-24","2027-02-24","Monthly",TRUE)')


def create_bonds(path):
    with new_workbook(path) as workbook:
        style, sheets = study_sheets(workbook, ("Market", "Bond Inputs", "Valuation", "Cashflows", "Risk"),
            "Straight bonds: price anchors, duration, spread, and curve risk", [
                "Compare the same fixed bond anchored at zero Z-spread, 100 bp Z-spread, and clean price 100.",
                "QS.BOND.PRICE is monetary PV; CleanPrice/DirtyPrice anchors use quotation units per 100 face.",
                "Duration is yield-modified duration. Spread derivatives and curve derivatives retain their actual units.",
            ])
        zero_market(sheets["Market"], style, "Bonds", "USD", "SOFR")
        inputs = sheets["Bond Inputs"]
        for row, (label, value) in enumerate([("Start", "2026-02-24"), ("Maturity", "2036-02-24"), ("Notional", 1000000), ("Coupon", .04)], 4):
            inputs.write(f"A{row}", label)
            inputs.write(f"B{row}", value, style["input_rate"] if row == 7 else style["input"])
        valuation = sheets["Valuation"]
        for column, title in enumerate(("Anchor", "Anchor value", "Trade", "PV (USD)", "Modified duration", "Z-spread")):
            valuation.write(3, column, title, style["header"])
        for row, anchor, value in [(5, "ZSpread", 0.0), (6, "ZSpread", .01), (7, "CleanPrice", 100.0)]:
            valuation.write(f"A{row}", anchor, style["input"])
            valuation.write(f"B{row}", value, style["input"])
            write(valuation, f"C{row}", formula("QS.TRADE.FIXED.BOND", f'"Bonds.Case{row}"', "Market!$B$3", "'Bond Inputs'!$B$4", "'Bond Inputs'!$B$5", '"USD"', '"SOFR"', "'Bond Inputs'!$B$6", "'Bond Inputs'!$B$7", '"Receive"', '"Actual360"', '"Compounded"', '"Semiannual"', '"Semiannual"', f"A{row}", f"B{row}"))
            for column, function in [("D", "QS.BOND.PRICE"), ("E", "QS.BOND.MODIFIED.DURATION"), ("F", "QS.BOND.Z.SPREAD")]:
                write(valuation, f"{column}{row}", f"={function}(C{row},Market!$F$11)", style["rate"] if column == "F" else style["number"])
        valuation.set_column("C:C", 55)
        valuation.activate()
        write(sheets["Cashflows"], "A4", "=QS.CASHFLOWS(Valuation!C5,Market!F11)")
        write(sheets["Risk"], "A4", "=QS.BOND.SENSITIVITIES(Valuation!C6,Market!F11)")
        sheets["Risk"].write("E2", "Same data without headers, transposed", style["section"])
        write(sheets["Risk"], "E4", "=QS.BOND.SENSITIVITIES(Valuation!C6,Market!F11,TRUE,FALSE)")


def create_interpolation(path):
    with new_workbook(path) as workbook:
        style, sheets = study_sheets(workbook, ("One Dimension", "Two Dimensions", "Schedules"),
            "Interpolation and schedules: nodes, methods, and worksheet layouts", [
                "Compare linear, log-linear, and natural cubic-spline interpolation on a positive discount-factor grid.",
                "The two-dimensional example reproduces a surface with an x*y interaction at nodes and between them.",
                "Schedules compares unadjusted and TARGET-adjusted dates; lists can be transposed directly.",
            ])
        one = sheets["One Dimension"]
        for cell, value in {"A4": "Time", "B4": "Discount factor", "D4": "Query", "E4": "Linear", "F4": "Log-linear", "G4": "Cubic spline"}.items():
            one.write(cell, value, style["header"])
        for row, x, y in [(5, 0, 1), (6, 1, .97), (7, 3, .91), (8, 5, .85), (9, 10, .70)]:
            one.write(f"A{row}", x, style["input"])
            one.write(f"B{row}", y, style["input"])
        for row, point in enumerate((0, .5, 1, 2, 4, 7, 10), 5):
            one.write(f"D{row}", point, style["input"])
            for column, function in [("E", "QS.INTERPOLATE.LINEAR"), ("F", "QS.INTERPOLATE.LOG.LINEAR"), ("G", "QS.INTERPOLATE.CUBIC.SPLINE")]:
                write(one, f"{column}{row}", f"={function}(D{row},$A$5:$A$9,$B$5:$B$9,FALSE)", style["factor"])
        two = sheets["Two Dimensions"]
        for column, label in zip("ABC", ("x", "y", "2 + x + 3*y + x*y")):
            two.write(f"{column}4", label, style["header"])
        for row, (x, y) in enumerate(((0, 0), (0, 2), (1, 0), (1, 2)), 5):
            two.write(f"A{row}", x, style["input"])
            two.write(f"B{row}", y, style["input"])
            write(two, f"C{row}", f"=2+A{row}+3*B{row}+A{row}*B{row}")
        for cell, value in {"E4": "Query x", "F4": "Query y", "G4": "Native bilinear", "H4": "Analytic value", "E5": .4, "F5": .7,
                            "G5": '=QS.INTERPOLATE.BILINEAR(E5,F5,A5:A8,B5:B8,C5:C8,"Reject")', "H5": "=2+E5+3*F5+E5*F5"}.items():
            write(two, cell, value, style["input"] if cell in ("E5", "F5") else style["text"])
        schedules = sheets["Schedules"]
        schedules.write("A2", "Unadjusted quarterly dates", style["section"])
        write(schedules, "A4", '=QS.SCHEDULE("2026-02-24","2027-02-24","Quarterly")')
        schedules.write("C2", "Same dates transposed", style["section"])
        write(schedules, "C4", '=QS.SCHEDULE("2026-02-24","2027-02-24","Quarterly",TRUE)')
        schedules.write("C8", "Following TARGET adjustment", style["section"])
        write(schedules, "C10", '=QS.SCHEDULE.ADJUSTED("2026-02-28","2027-02-28","Quarterly","TARGET","Following",TRUE)')
        one.activate()


STUDY_CHECKS = {
    "loan_pricing_example": {
        "handles": [("Market", "F11"), ("Comparison", "F5"), ("Comparison", "F14")],
        "numbers": [("Comparison", f"{column}{row}") for row in range(5, 15) for column in "GH"] + [("Comparison", "I5"), ("Comparison", "I8")],
    },
    "bond_pricing_example": {
        "handles": [("Market", "F11"), ("Valuation", "C5"), ("Valuation", "C7")],
        "numbers": [("Valuation", f"{column}{row}") for row in (5, 6, 7) for column in "DEF"],
    },
    "interpolation_example": {
        "handles": [],
        "numbers": [("One Dimension", f"{column}{row}") for row in range(5, 12) for column in "EFG"] + [("Two Dimensions", "G5")],
    },
}


def validate_in_excel(path, xll_path):
    """Check the optional live XLL workflow and unchanged F9 recalculation."""
    from win32com.client import DispatchEx

    excel = DispatchEx("Excel.Application")
    workbook = None
    try:
        excel.Visible = False
        excel.DisplayAlerts = False
        if not excel.RegisterXLL(str(xll_path)):
            raise RuntimeError(f"Excel could not load {xll_path}")
        workbook = excel.Workbooks.Open(str(path.resolve()), UpdateLinks=0, ReadOnly=True)
        workbook.ForceFullCalculation = True
        excel.CalculateFullRebuild()
        historical = "Historical Pricing" in [sheet.Name for sheet in workbook.Worksheets]
        study = STUDY_CHECKS.get(path.stem)
        if study:
            handles = [workbook.Worksheets(sheet).Range(cell) for sheet, cell in study["handles"]]
            prices = [workbook.Worksheets(sheet).Range(cell) for sheet, cell in study["numbers"]]
            if path.stem == "interpolation_example":
                sheet = workbook.Worksheets("Two Dimensions")
                if abs(sheet.Range("G5").Value2 - sheet.Range("H5").Value2) > 1e-10:
                    raise RuntimeError("Bilinear result disagrees with the analytic surface")
            elif path.stem == "loan_pricing_example":
                sheet = workbook.Worksheets("Comparison")
                if sheet.Range("G13").Value2 >= sheet.Range("G5").Value2:
                    raise RuntimeError("Higher funding rates did not reduce fixed-loan PV")
                layouts = workbook.Worksheets("Layouts")
                if layouts.Range("A11").Value2 != "Risk factor":
                    raise RuntimeError("Transposed sensitivity table lost its header")
                for without_header, with_header in (("A4", "B11"), ("A5", "B12")):
                    if layouts.Range(without_header).Value2 != layouts.Range(with_header).Value2:
                        raise RuntimeError("Header-free and transposed sensitivity values do not align")
        elif historical:
            sheet = workbook.Worksheets("Historical Pricing")
            handles = [sheet.Range(f"{column}{row}") for column in "BC" for row in (9, 11, 12, 13, 20, 21)]
            prices = [sheet.Range(f"{column}24") for column in "BC"]
            for column in "BC":
                if sheet.Range(f"{column}23").Value2 != sheet.Range(f"{column}1").Value2:
                    raise RuntimeError("Historical valuation date does not match its market")
        else:
            sheet = workbook.Worksheets("Pricing")
            handles = [sheet.Range(f"B{row}") for row in (4, 5, 6, 7, 10, 12)]
            prices = [sheet.Range(f"B{row}") for row in (8, 9, 11, 13)]
            results = workbook.Worksheets("Curve Results")
            numeric = [results.Range(cell).Value2 for cell in ("C5", "E6", "N6", "N8", "N11", "N12")]
            for value in numeric:
                if not isinstance(value, float) or not math.isfinite(value):
                    raise RuntimeError(f"Curve query did not return a finite number: {value!r}")
            if numeric[0] != 1.0 or abs(numeric[2] - prices[2].Value2) > 1e-8:
                raise RuntimeError("Curve table and point-query discount factors disagree")
            for output_sheet, cell in [("Outputs", "A4"), ("Outputs", "K4"), ("Function Help", "A4")]:
                if not isinstance(workbook.Worksheets(output_sheet).Range(cell).Value2, str):
                    raise RuntimeError(f"{output_sheet}!{cell} did not spill a header")
        before = [cell.Value2 for cell in handles]
        if not all(isinstance(value, str) and value.startswith("QSObject:") for value in before):
            raise RuntimeError(f"Expected native object handles: {before!r}")
        values = [cell.Value2 for cell in prices]
        if not all(isinstance(value, float) and math.isfinite(value) for value in values):
            raise RuntimeError(f"Pricing did not return finite numbers: {values!r}")
        if not historical and not study:
            notional = workbook.Worksheets("Trade Inputs").Range("B4").Value2
            if abs(values[0]) > notional or abs(values[1]) > 0.5:
                raise RuntimeError("Swap price or fair rate is implausible for the example")
            if not all(0.0 < discount <= 1.5 for discount in values[2:]):
                raise RuntimeError("Maturity discount factors are outside the example's expected range")
            if any(abs(forward) > 0.5 for forward in numeric[4:]):
                raise RuntimeError("Curve point forwards are outside the example's expected range")
        excel.Calculate()
        if [cell.Value2 for cell in handles] != before:
            raise RuntimeError("Unchanged F9 recalculation changed object handles")
        if any(abs(cell.Value2 - value) > 1e-8 for cell, value in zip(prices, values)):
            raise RuntimeError("Unchanged F9 recalculation changed prices")
    finally:
        if workbook is not None:
            workbook.Close(SaveChanges=False)
        excel.Quit()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--example", choices=("all", "swap", "historical", "loans", "bonds", "interpolation"), default="all")
    parser.add_argument("--output-dir", type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument("--xll-path", type=Path, help="Validate with native Windows Excel; requires pywin32")
    args = parser.parse_args()
    if args.xll_path:
        if sys.platform != "win32":
            parser.error("--xll-path requires Windows Excel")
        args.xll_path = args.xll_path.resolve()
        if not args.xll_path.is_file():
            parser.error(f"XLL file does not exist: {args.xll_path}")
    args.output_dir.mkdir(parents=True, exist_ok=True)
    for name, filename, generate in EXAMPLES:
        if args.example not in ("all", name):
            continue
        with TemporaryDirectory(dir=args.output_dir) as temporary:
            path = Path(temporary) / filename
            generate(path)
            if args.xll_path:
                validate_in_excel(path, args.xll_path)
            path.replace(args.output_dir / filename)
        print(f"Created {args.output_dir / filename}")


EXAMPLES = [
    ("swap", "swap_pricing_example.xlsx", create_swap),
    ("historical", "historical_pricing_example.xlsx", create_historical),
    ("loans", "loan_pricing_example.xlsx", create_loans),
    ("bonds", "bond_pricing_example.xlsx", create_bonds),
    ("interpolation", "interpolation_example.xlsx", create_interpolation),
]


if __name__ == "__main__":
    main()

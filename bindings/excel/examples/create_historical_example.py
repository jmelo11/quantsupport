"""Generate the historical-pricing workbook with Python's standard library.

Run: python bindings/excel/examples/create_historical_example.py
Load quantsupport.xll in Windows Excel, open the workbook, then Ctrl+Alt+F9.
The workbook stores formulas and inputs; it does not embed process-local handles.
"""

from pathlib import Path
from xml.sax.saxutils import escape
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo


def formula(name, *arguments):
    return "=" + name + "(" + ",".join(arguments) + ")"


def workbook_cells():
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


def create(path):
    namespace = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    rows = []
    cells = workbook_cells()
    for row in range(1, 31):
        values = []
        for column in "ABC":
            address = f"{column}{row}"
            if address not in cells:
                continue
            value = cells[address]
            style = 2 if row in (1, 4, 19) else 1 if row in (5, 6, 7) and column != "A" else 3 if row == 24 and column != "A" else 0
            attributes = f'r="{address}" s="{style}"'
            if isinstance(value, (int, float)):
                content = f"<v>{value}</v>"
            elif value.startswith("="):
                content = f"<f>{escape(value[1:])}</f>"
            else:
                attributes += ' t="inlineStr"'
                content = f"<is><t>{escape(value)}</t></is>"
            values.append(f"<c {attributes}>{content}</c>")
        rows.append(f'<row r="{row}">' + "".join(values) + "</row>")
    parts = {
        "[Content_Types].xml": '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>',
        "_rels/.rels": '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>',
        "xl/workbook.xml": f'<workbook xmlns="{namespace}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Historical Pricing" sheetId="1" r:id="rId1"/></sheets><calcPr calcMode="auto" fullCalcOnLoad="1" forceFullCalc="1"/></workbook>',
        "xl/_rels/workbook.xml.rels": '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>',
        "xl/styles.xml": f'<styleSheet xmlns="{namespace}"><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf/></cellStyleXfs><cellXfs count="4"><xf xfId="0"/><xf xfId="0" numFmtId="10" applyNumberFormat="1"/><xf xfId="0" fontId="1" applyFont="1"/><xf xfId="0" numFmtId="4" applyNumberFormat="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>',
        "xl/worksheets/sheet1.xml": f'<worksheet xmlns="{namespace}"><dimension ref="A1:C30"/><sheetViews><sheetView workbookViewId="0"><pane xSplit="1" ySplit="4" topLeftCell="B5" activePane="bottomRight" state="frozen"/></sheetView></sheetViews><cols><col min="1" max="1" width="38" customWidth="1"/><col min="2" max="3" width="55" customWidth="1"/></cols><sheetData>' + "".join(rows) + '</sheetData></worksheet>',
    }
    with ZipFile(path, "w", compression=ZIP_DEFLATED) as archive:
        for name, content in parts.items():
            info = ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            archive.writestr(info, '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' + content)


if __name__ == "__main__":
    create(Path(__file__).with_name("historical_pricing_example.xlsx"))

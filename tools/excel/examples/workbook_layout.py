"""Consistent navigation, named inputs, and readable calculation steps."""

from datetime import datetime
import re

import xlsxwriter
from xlsxwriter.utility import xl_range_abs


def text(value):
    return '"' + str(value).replace('"', '""') + '"'


def formula(name, *arguments):
    return "=" + name + "(" + ",".join(str(argument) for argument in arguments) + ")"


def context(root):
    return "=" + root


class Study:
    def __init__(self, path, prefix, title, purpose, routes, experiments):
        self.prefix = prefix
        self.workbook = xlsxwriter.Workbook(path)
        self.workbook.set_properties({"title": title, "created": datetime(2026, 1, 1)})
        self.workbook.set_calc_mode("auto")
        base = {"font_name": "Calibri", "font_size": 11, "valign": "vcenter", "text_wrap": True}
        variants = {
            "body": {"font_color": "#273B4A"},
            "title": {"bold": True, "font_size": 20, "font_color": "white", "bg_color": "#17324D"},
            "subtitle": {"font_color": "#526779", "font_size": 11},
            "header": {"bold": True, "font_color": "white", "bg_color": "#176B75", "bottom": 1},
            "section": {"bold": True, "font_size": 12, "bg_color": "#E5EDF2", "font_color": "#17324D"},
            "input": {"bg_color": "#E8F1FC", "font_color": "#174EA6", "border": 1, "border_color": "#CBDBEE"},
            "input_rate": {"bg_color": "#E8F1FC", "font_color": "#174EA6", "border": 1, "border_color": "#CBDBEE", "num_format": "0.00%"},
            "input_number": {"bg_color": "#E8F1FC", "font_color": "#174EA6", "border": 1, "border_color": "#CBDBEE", "num_format": "#,##0.00"},
            "result": {"bg_color": "#EAF5EF", "font_color": "#145C43"},
            "money": {"bg_color": "#EAF5EF", "font_color": "#145C43", "num_format": "#,##0.00;[Red](#,##0.00)"},
            "rate": {"bg_color": "#EAF5EF", "font_color": "#145C43", "num_format": "0.0000%"},
            "number": {"bg_color": "#EAF5EF", "font_color": "#145C43", "num_format": "0.0000"},
            "factor": {"num_format": "0.000000", "font_color": "#145C43"},
            "handle": {"font_name": "Consolas", "font_size": 9, "bg_color": "#F0F2F5", "font_color": "#526779"},
            "formula": {"font_name": "Consolas", "font_size": 9, "font_color": "#526779", "bg_color": "#F7F9FB", "valign": "top"},
            "link": {"font_color": "#126B9A", "underline": True},
            "note": {"font_size": 10, "font_color": "#526779"},
        }
        self.formats = {name: self.workbook.add_format(base | values) for name, values in variants.items()}
        self.sheets = {}
        self.cells = {}
        self.routes = routes
        start = self.workbook.add_worksheet("Start Here")
        self.sheets["Start Here"] = start
        self._start(start, title, purpose, experiments)

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.sheets["Start Here"].activate()
        self.sheets["Start Here"].set_first_sheet()
        self.workbook.close()

    def name(self, key):
        return self.prefix + "_" + key

    def bind(self, key, sheet, cell):
        name = self.name(key)
        target = "='" + sheet.replace("'", "''") + "'!" + re.sub(r"([A-Z]+)(\d+)", r"$\1$\2", cell)
        self.workbook.define_name(name, target)
        self.cells[name] = (sheet, cell)
        return name

    def range(self, key, sheet, first_row, first_col, last_row, last_col):
        self.workbook.define_name(self.name(key), "='" + sheet + "'!" + xl_range_abs(
            first_row - 1, first_col, last_row - 1, last_col))
        return self.name(key)

    def write(self, sheet, cell, value, kind="body", key=None, comment=None):
        ws = self.sheets[sheet]
        fmt = self.formats[kind]
        if isinstance(value, str) and value.startswith("="):
            expression = re.sub(r"(?<!_xll\.)(QS\.[A-Z0-9.]+)\(", r"_xll.\1(", value)
            if value.startswith("=QS."):
                ws.write_dynamic_array_formula(cell, expression, fmt, "")
            else:
                ws.write_formula(cell, expression, fmt, "")
        elif value is None:
            ws.write_blank(cell, None, fmt)
        else:
            ws.write(cell, value, fmt)
        if comment:
            ws.write_comment(cell, comment, {"width": 320, "height": 120})
        if key:
            return self.bind(key, sheet, cell)

    def merge(self, sheet, area, value, kind="note"):
        self.sheets[sheet].merge_range(area, value, self.formats[kind])

    def headers(self, sheet, row, labels):
        ws = self.sheets[sheet]
        ws.write_row(row - 1, 0, labels, self.formats["header"])
        ws.set_row(row - 1, 36)

    def link(self, sheet, cell, destination, label):
        self.sheets[sheet].write_url(cell, "internal:" + destination,
                                     self.formats["link"], label)

    def sheet(self, name, title, purpose, widths=(34, 48, 48), color="#78909C"):
        ws = self.workbook.add_worksheet(name)
        self.sheets[name] = ws
        ws.hide_gridlines(2)
        ws.set_zoom(85)
        ws.set_default_row(24)
        ws.set_tab_color(color)
        for column, width in enumerate(widths):
            ws.set_column(column, column, width, self.formats["body"])
        last = xlsxwriter.utility.xl_col_to_name(len(widths) - 1)
        self.merge(name, f"A1:{last}1", title, "title")
        ws.set_row(0, 38)
        self.merge(name, f"A2:{last}3", purpose, "subtitle")
        self.link(name, "A4", "'Start Here'!A1", "← Start Here")
        route_names = [route[0] for route in self.routes]
        index = route_names.index(name)
        if index + 1 < len(route_names):
            next_name = route_names[index + 1]
            self.link(name, f"{last}4", f"'{next_name}'!A1", "Next → " + next_name)
            ws.set_row(3, 36)
        ws.freeze_panes(6, 1)
        ws.set_landscape()
        ws.set_paper(9)
        ws.fit_to_pages(1, 0)
        ws.repeat_rows(0, 5)
        ws.set_margins(0.3, 0.3, 0.4, 0.4)
        ws.set_footer("&LQuantSupport&C" + title + "&R&P / &N")
        return ws

    def input(self, sheet, row, label, value, key, meaning, kind="input", choices=None):
        self.write(sheet, f"A{row}", label)
        name = self.write(sheet, f"B{row}", value, kind, key, meaning)
        self.write(sheet, f"C{row}", meaning, "note")
        self.sheets[sheet].set_row(row - 1, 32)
        if choices:
            self.sheets[sheet].data_validation(f"B{row}", {
                "validate": "list", "source": choices, "error_type": "stop",
                "error_title": "Choose a listed value", "error_message": "Use the dropdown for this example.",
            })
        return name

    def step(self, sheet, row, label, expression, key, explanation, column="B"):
        self.write(sheet, f"A{row}", label, "section")
        self.write(sheet, f"{column}{row}", expression, "handle", key, explanation)
        if column == "B":
            self.write(sheet, f"C{row}", explanation, "note")
        self.sheets[sheet].set_row(row - 1, 38)
        self.merge(sheet, f"A{row+1}:C{row+1}", "", "formula")
        self.sheets[sheet].write_formula(f"A{row+1}", f'SUBSTITUTE(_xlfn.FORMULATEXT({column}{row}),",",", ")',
                                          self.formats["formula"], expression.replace(",", ", "))
        self.sheets[sheet].set_row(row, 34 if len(expression) < 165 else 48)
        return row + 3

    def result(self, sheet, row, label, expression, key, meaning, kind="money"):
        self.write(sheet, f"A{row}", label)
        self.write(sheet, f"B{row}", expression, kind, key, meaning)
        self.write(sheet, f"C{row}", meaning, "note")
        self.sheets[sheet].set_row(row - 1, 34)

    def _start(self, ws, title, purpose, experiments):
        ws.hide_gridlines(2)
        ws.set_zoom(90)
        ws.set_default_row(26)
        for column, width in enumerate((8, 22, 17, 22, 22, 18)):
            ws.set_column(column, column, width, self.formats["body"])
        self.merge("Start Here", "A1:F2", title, "title")
        ws.set_row(0, 30)
        ws.set_row(1, 30)
        self.merge("Start Here", "A3:F4", purpose, "subtitle")
        self.merge("Start Here", "A6:F7", "Before you start: load quantsupport.xll in 64-bit Windows Excel, then press Ctrl+Alt+F9. Results are blank until Excel calculates. Dates are ISO text; enter 4% as 0.04.", "section")
        self.merge("Start Here", "A9:F9", "Follow the numbered tabs from left to right", "header")
        for row, (name, explanation) in enumerate(self.routes, 10):
            self.write("Start Here", f"A{row}", str(row - 9), "section")
            self.merge("Start Here", f"B{row}:C{row}", "")
            self.link("Start Here", f"B{row}", f"'{name}'!A1", name)
            self.merge("Start Here", f"D{row}:F{row}", explanation, "body")
            ws.set_row(row - 1, 34)
        row = 12 + len(self.routes)
        self.merge("Start Here", f"A{row}:F{row}", "Cell colours", "header")
        for offset, kind, label, meaning in [
            (1, "input", "Blue · edit", "Assumptions and choices you can change."),
            (2, "result", "Green · read", "Financial results calculated by the add-in."),
            (3, "handle", "Grey · build", "Object references on the Build tab. Each step shows its Excel formula below."),
        ]:
            self.merge("Start Here", f"A{row+offset}:C{row+offset}", label, kind)
            self.merge("Start Here", f"D{row+offset}:F{row+offset}", meaning, "body")
            ws.set_row(row + offset - 1, 30)
        row += 5
        self.merge("Start Here", f"A{row}:F{row}", "Try this first", "header")
        for offset, experiment in enumerate(experiments, 1):
            self.merge("Start Here", f"A{row+offset}:F{row+offset}", f"{offset}. {experiment}", "body")
            ws.set_row(row + offset - 1, 34)
        row += len(experiments) + 2
        self.merge("Start Here", f"A{row}:F{row+1}", "Use Ctrl+A after a function name for parameter help. Inline hints use the optional Excel-DNA IntelliSense companion. The example assumptions are illustrative; tables and results use the native Rust pricers.", "note")
        ws.freeze_panes(9, 0)
        ws.set_portrait()
        ws.fit_to_pages(1, 1)
        ws.print_area(0, 0, row, 5)

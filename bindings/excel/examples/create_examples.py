"""Generate the guided Excel studies.

Install: python -m pip install -r bindings/excel/examples/requirements.txt
Run: python bindings/excel/examples/create_examples.py
Windows Excel with the matching QuantSupport XLL calculates the formulas.
Use --xll-path for optional live validation with pywin32.
"""

import argparse
from datetime import datetime
import math
from pathlib import Path
import shutil
import sys
from tempfile import TemporaryDirectory
from uuid import uuid4

from workbook_layout import Study, context, formula, text
from portfolio_example import create_portfolio


SOFR_QUOTES = [
    ("FixedRateDeposit_USD_SOFR_1D", .045), ("OIS_USD_SOFR_1Y", .04837),
    ("OIS_USD_SOFR_2Y", .04528), ("OIS_USD_SOFR_3Y", .04260),
    ("OIS_USD_SOFR_5Y", .04077), ("OIS_USD_SOFR_7Y", .04033),
    ("OIS_USD_SOFR_10Y", .03898), ("OIS_USD_SOFR_30Y", .03738),
]
ESTR_QUOTES = [
    ("FixedRateDeposit_EUR_ESTR_1D", .029), ("OIS_EUR_ESTR_3M", .0288),
    ("OIS_EUR_ESTR_6M", .028), ("OIS_EUR_ESTR_1Y", .0265),
    ("OIS_EUR_ESTR_2Y", .025), ("OIS_EUR_ESTR_3Y", .0245),
    ("OIS_EUR_ESTR_4Y", .0243), ("OIS_EUR_ESTR_5Y", .0242),
    ("OIS_EUR_ESTR_7Y", .0245), ("OIS_EUR_ESTR_10Y", .0252),
    ("OIS_EUR_ESTR_15Y", .0258), ("OIS_EUR_ESTR_20Y", .0255),
    ("OIS_EUR_ESTR_30Y", .024),
]
ZERO_PILLARS = [("0D", .03), ("5Y", .03), ("10Y", .032), ("30Y", .034), ("35Y", .035)]


def requests(study, sheet, row, names=("Value", "Cashflows", "Sensitivities")):
    study.headers(sheet, row, ["Requested measure", "What the result contains", ""])
    descriptions = {"Value": "Monetary present value", "Cashflows": "Resolved payments and receipts",
                    "Sensitivities": "Raw derivatives per unit market input", "ModifiedDuration": "Yield-modified duration in years",
                    "ZSpread": "Annual decimal spread implied by the bond anchor"}
    for offset, name in enumerate(names, row + 1):
        study.write(sheet, f"A{offset}", name)
        study.write(sheet, f"B{offset}", descriptions[name], "note")
    study.range("Requests", sheet, row + 1, 0, row + len(names), 0)


def inputs(study, sheet, rows, purpose):
    study.sheet(sheet, "Contract and position inputs", purpose, (34, 26, 62), "#337AB7")
    study.headers(sheet, 6, ["Parameter", "Value", "Meaning"])
    for row, (label, value, key, meaning, kind) in enumerate(rows, 7):
        study.input(sheet, row, label, value, key, meaning, kind)


def step(study, sheet, row, label, name, args, key, explanation):
    return study.step(sheet, row, label, formula(name, *args), key, explanation)


def simple_market(study, sheet, currency, index, loan=False):
    """Market assumptions first; the construction sequence follows below them."""
    widths = (38, 42, 36, 24) if loan else (38, 42, 44)
    study.sheet(sheet, "Market assumptions and curve construction",
                "Edit the blue rates. The steps below the input tables construct ordinary curves from those cells.", widths, "#337AB7")
    study.headers(sheet, 6, ["Pillar", f"{index} zero", "EURIBOR12m zero", "Treasury spread"] if loan else ["Pillar", "Continuous zero rate", ""])
    for row, (pillar, rate) in enumerate(ZERO_PILLARS, 7):
        study.write(sheet, f"A{row}", pillar, "input")
        study.write(sheet, f"B{row}", rate, "input_rate")
        if loan:
            study.write(sheet, f"C{row}", [.04, .037, .04, .0415, .0415][row-7], "input_rate")
            study.write(sheet, f"D{row}", [.005, .01, .011, .014, .015][row-7], "input_rate")
    study.range("Pillars", sheet, 7, 0, 11, 0)
    study.range("ZeroRates", sheet, 7, 1, 11, 1)
    if loan:
        study.range("ForwardRates", sheet, 7, 2, 11, 2)
        study.range("FundingSpreads", sheet, 7, 3, 11, 3)
        study.merge(sheet, "A13:D14", "These are illustrative continuous zero rates on Actual360. FUNDING is an ordinary composite curve: EURIBOR12m discount factors × treasury-spread discount factors.")
        study.headers(sheet, 17, ["Borrower CDS quote", "Annual spread", "", ""])
        for row, (tenor, spread) in enumerate(zip(("1Y", "3Y", "5Y", "10Y", "20Y", "30Y", "35Y"), (.005, .007, .009, .011, .014, .016, .017)), 18):
            study.write(sheet, f"A{row}", f"Cds_BORROWER_EUR_{tenor}")
            study.write(sheet, f"B{row}", spread, "input_rate")
        study.range("CreditQuotes", sheet, 18, 0, 24, 0)
        study.input(sheet, 27, "Borrower recovery", .4, "Recovery", "Fraction recovered on borrower default.", "input_rate")
        row = 31
    else:
        study.merge(sheet, "A13:C14", "Illustrative continuous zero rates on Actual360, linearly interpolated with extrapolation enabled. Changing a pillar rebuilds the context and dependent prices.")
        row = 17
    study.headers(sheet, row, ["Construction step", "Object reference", "Why it is needed"])
    row += 1
    row = step(study, sheet, row, "1. Create quote store", "QS.QUOTES.CREATE", [text(study.prefix + ".Quotes"), study.name("ValuationDate")], "InitialQuotes", "Sets the valuation date for the market.")
    previous = study.name("InitialQuotes")
    if loan:
        for offset in range(7):
            key = f"CreditQuote{offset+1}"
            row = step(study, sheet, row, f"2.{offset+1} Load CDS quote", "QS.QUOTES.SET",
                       [previous, f"'{sheet}'!A{18+offset}", f"'{sheet}'!B{18+offset}", "", ""], key, "Each update consumes the preceding store revision.")
            previous = study.name(key)
    study.workbook.define_name(study.name("Quotes"), "=" + previous)
    row = step(study, sheet, row, "3. Create curve configurations", "QS.CURVE.CONFIGS.CREATE", [text(study.prefix + ".Curves")], "InitialCurves", "Curve construction choices are separate from contract terms.")
    row = step(study, sheet, row, f"4. Configure {index}", "QS.CURVE.CONFIGS.ADD.ZERO",
               [study.name("InitialCurves"), text(index), '"Actual360"', '"Linear"', "TRUE", study.name("Pillars"), study.name("ZeroRates"), '"Continuous"', '"Annual"'], "BaseCurves", "The context builds this curve from the blue input range.")
    if loan:
        row = step(study, sheet, row, "5. Configure projection curve", "QS.CURVE.CONFIGS.ADD.ZERO",
                   [study.name("BaseCurves"), '"EURIBOR12m"', '"Actual360"', '"Linear"', "TRUE", study.name("Pillars"), study.name("ForwardRates"), '"Continuous"', '"Annual"'], "ForwardCurves", "EURIBOR12m projects floating coupon fixings.")
        row = step(study, sheet, row, "6. Configure treasury spread", "QS.CURVE.CONFIGS.ADD.ZERO",
                   [study.name("ForwardCurves"), '"FUNDINGSPREAD"', '"Actual360"', '"Linear"', "TRUE", study.name("Pillars"), study.name("FundingSpreads"), '"Continuous"', '"Annual"'], "SpreadCurves", "The spread is constructed with the same curve API.")
        row = step(study, sheet, row, "7. Configure funding curve", "QS.CURVE.CONFIGS.ADD.COMPOSITE",
                   [study.name("SpreadCurves"), '"FUNDING"', '"EURIBOR12m"', '"FUNDINGSPREAD"'], "Curves", "The discount policy chooses whether to use ESTR or FUNDING.")
        row = step(study, sheet, row, "8. Create credit configurations", "QS.CREDIT.CURVE.CONFIGS.CREATE", [text(study.prefix + ".Credit")], "InitialCredit", "Borrower survival is calibrated from the CDS quotes.")
        step(study, sheet, row, "9. Configure borrower credit", "QS.CREDIT.CURVE.CONFIGS.ADD",
             [study.name("InitialCredit"), '"Credit:BORROWER"', text(currency), text(index), study.name("Recovery"), study.name("CreditQuotes")], "Credit", "Only the borrower-credit case attaches this credit index to its contract.")
    else:
        study.workbook.define_name(study.name("Curves"), "=" + study.name("BaseCurves"))


def detail_sheet(study, sheet, title, result, measure, selected=None):
    cashflows = measure == "CASHFLOWS"
    widths = (16, 28, 18, 14, 12, 12, 12, 12, 8) if cashflows else (62, 24, 24)
    ws = study.sheet(sheet, title,
        "Signed, undiscounted receipts and payments in each row's currency. Loan legs 0 / 1 / 2 are scheduled receipts / prepayment / recovery." if cashflows else
        "Raw value derivatives per unit input. Multiply a decimal-rate derivative by 0.0001 to estimate a +1 bp move. Preserve each factor's units.", widths, "#78909C")
    if selected:
        study.write(sheet, "A6", "Selected case")
        study.write(sheet, "B6", "=" + study.name("SelectedCase"), "result")
        study.link(sheet, "C6", selected, "Change case →")
    study.write(sheet, "A7", formula("QS.RESULT." + measure, result))
    ws.freeze_panes(7, 2 if cashflows else 1)
    ws.repeat_rows(0, 6)
    ws.set_column(2 if cashflows else 1, 2 if cashflows else 1, 18 if cashflows else 24, study.formats["money"])
    if cashflows:
        for column in (3, 6, 7):
            ws.set_column(column, column, 12, study.formats["rate"])
        ws.set_column(4, 4, 12, study.formats["number"])


def selected_case(study, build, row, results, labels, result_keys, selector_row):
    study.headers(build, row, ["Case lookup for detail sheets", "Evaluation result", ""])
    for offset, (label, result) in enumerate(zip(labels, result_keys), row + 1):
        study.write(build, f"A{offset}", label)
        study.write(build, f"B{offset}", "=" + study.name(result), "handle")
    study.range("ResultHandles", build, row + 1, 1, row + len(labels), 1)
    study.range("CaseNames", results, 7, 0, 6 + len(labels), 0)
    study.write(results, f"A{selector_row}", "Inspect one case", "section")
    study.write(results, f"B{selector_row}", labels[0], "input", "SelectedCase")
    study.sheets[results].data_validation(f"B{selector_row}", {"validate": "list", "source": "=" + study.name("CaseNames")})
    study.merge(results, f"C{selector_row}:F{selector_row+1}", "Choose a case, then open Cashflows or Risk. Both detail sheets follow this selection.")
    expression = f"=INDEX({study.name('ResultHandles')},MATCH({study.name('SelectedCase')},{study.name('CaseNames')},0))"
    study.step(build, row + len(labels) + 3, "Selected case for detail sheets", expression, "SelectedResult", "The dropdown on Results chooses an existing evaluation result.")


def create_swap(path):
    routes = [("1 Inputs", "Set the contract and position."), ("2 Market", "Review quotes, then construct the market inputs."),
              ("3 Build", "Follow the instrument → trade → policy → pricer → results sequence."), ("4 Results", "Read PV, fair quotes and curve checks."),
              ("5 Cashflows", "Inspect the swap's signed cashflows."), ("6 Risk", "Inspect raw market sensitivities."), ("7 Curves", "Inspect the bootstrapped nodes.")]
    with Study(path, "Swap", "Swap pricing · a guided example", "Value one receive-fixed USD swap under USD/SOFR collateral. Build a separate EUR/ESTR market for curve comparisons.", routes,
               ["Change the fixed coupon on Inputs and read the PV on Results.", "Change a SOFR quote on Market and compare the fair fixed coupon.", "Compare FairRate(0), the fixed coupon, with FairRate(1), the floating spread."]) as s:
        inputs(s, "1 Inputs", [
            ("Valuation date", "2026-02-24", "ValuationDate", "Market reference date (ISO text).", "input"),
            ("Notional (USD)", 10_000_000, "Notional", "Contract principal and default position size.", "input_number"),
            ("Fixed coupon", .0425, "FixedRate", "Annual decimal fixed coupon received by the position.", "input_rate"),
            ("Floating spread", 0, "Spread", "Annual decimal spread paid over SOFR.", "input_rate"),
            ("Start date", "2026-02-24", "Start", "Accrual starts on this date.", "input"),
            ("Maturity date", "2031-02-24", "Maturity", "Final contractual date.", "input"),
            ("Currency", "USD", "Currency", "This study prices a USD contract in the USD/SOFR context.", "body"),
            ("Projection index", "SOFR", "Index", "SOFR projects this swap; ESTR has a separate comparison context.", "body"),
            ("Position side", "Receive", "Side", "Receive means receive fixed / pay floating.", "input"),
            ("Day counter", "Actual360", "DayCounter", "Contractual interest accrual convention.", "input"),
            ("Coupon compounding", "Simple", "Compounding", "Native coupon rate convention.", "input"),
            ("Rate frequency", "Semiannual", "RateFrequency", "Rate convention, separate from payment frequency.", "input"),
            ("Fixed payment frequency", "Semiannual", "FixedFrequency", "Fixed-leg payment schedule.", "input"),
            ("Floating payment frequency", "Quarterly", "FloatFrequency", "Floating-leg payment schedule.", "input"),
        ], "Change blue cells; read their units and meaning in the right-hand column.")
        requests(s, "1 Inputs", 23)
        market = "2 Market"
        s.sheet(market, "Market quotes and construction", "Blue Mid quotes are annual decimals. Quote loading and curve configurations appear below the input tables.", (42, 42, 42), "#337AB7")
        s.headers(market, 6, ["USD SOFR quote", "Mid", ""])
        for row, (identifier, rate) in enumerate(SOFR_QUOTES, 7):
            s.write(market, f"A{row}", identifier)
            s.write(market, f"B{row}", rate, "input_rate")
        s.range("SofrQuotes", market, 7, 0, 14, 0)
        s.headers(market, 17, ["EUR ESTR quote", "Mid", ""])
        for row, (identifier, rate) in enumerate(ESTR_QUOTES, 18):
            s.write(market, f"A{row}", identifier)
            s.write(market, f"B{row}", rate, "input_rate")
        s.range("EstrQuotes", market, 18, 0, 30, 0)
        s.headers(market, 33, ["Construction step", "Object reference", ""])
        row = step(s, market, 34, "1. Create quote store", "QS.QUOTES.CREATE", [text("Swap.Quotes"), s.name("ValuationDate")], "InitialQuotes", "All quotes share this valuation date.")
        previous = s.name("InitialQuotes")
        for offset, source in enumerate([*range(7, 15), *range(18, 31)], 1):
            key = f"Quote{offset}"
            row = step(s, market, row, f"2.{offset} Load quote", "QS.QUOTES.SET", [previous, f"'{market}'!A{source}", f"'{market}'!B{source}", "", ""], key, "Updates form one ordered store chain.")
            previous = s.name(key)
        s.workbook.define_name(s.name("Quotes"), "=" + previous)
        row = step(s, market, row, "3. Create USD configurations", "QS.CURVE.CONFIGS.CREATE", [text("Swap.UsdCurves")], "InitialCurves", "USD/SOFR and EUR/ESTR use separate bootstrap collateral assumptions.")
        row = step(s, market, row, "4. Configure SOFR", "QS.CURVE.CONFIGS.ADD", [s.name("InitialCurves"), '"SOFR"', '"Actual360"', '"LogLinear"', "TRUE", s.name("SofrQuotes")], "SofrConfig", "Bootstraps the visible USD deposit / OIS quotes.")
        row = step(s, market, row, "5. Create EUR configurations", "QS.CURVE.CONFIGS.CREATE", [text("Swap.EurCurves")], "InitialEstrCurves", "An independent configuration collection for the EUR/ESTR context.")
        step(s, market, row, "6. Configure ESTR", "QS.CURVE.CONFIGS.ADD", [s.name("InitialEstrCurves"), '"ESTR"', '"Actual360"', '"LogLinear"', "TRUE", s.name("EstrQuotes")], "EstrConfig", "EUR OIS quotes bootstrap under EUR/ESTR collateral in their own context.")
        build = "3 Build"
        s.sheet(build, "Build the pricing objects", "Read from top to bottom. Grey cells contain object references; the exact formula is shown immediately below each step.")
        s.headers(build, 6, ["Step", "Object reference", "Purpose"])
        row = s.step(build, 7, "1.1 Initialize USD context", context("Swap.Context", s.name("Quotes"), s.name("SofrConfig"), s.name("Currency"), s.name("Index")), "Context", "Bootstraps SOFR under USD collateral; this context prices the swap.")
        row = s.step(build, row, "1.2 Initialize EUR context", context("Swap.EstrContext", s.name("Quotes"), s.name("EstrConfig"), '"EUR"', '"ESTR"'), "EstrContext", "Bootstraps ESTR under EUR collateral for comparison only.")
        row = step(s, build, row, "2. Define the instrument", "QS.INSTRUMENT.SWAP", [text("Swap.Instrument"), *[s.name(k) for k in ("Start", "Maturity", "Currency", "Index", "Notional", "FixedRate", "Spread", "DayCounter", "Compounding", "RateFrequency", "FixedFrequency", "FloatFrequency")]], "Instrument", "The contract contains dates, rates and indices.")
        row = step(s, build, row, "3. Create the position", "QS.TRADE.CREATE", [text("Swap.Trade"), s.name("Instrument"), s.name("ValuationDate"), s.name("Side")], "Trade", "The trade adds execution date and direction.")
        row = step(s, build, row, "4. Choose discount policy", "QS.DISCOUNT.POLICY.CSA", [text("Swap.Policy"), s.name("Currency"), s.name("Index")], "Policy", "The policy selects the native CSA discount curve.")
        row = step(s, build, row, "5. Create the pricer", "QS.PRICER.CREATE", [text("Swap.Pricer"), '"DiscountedCashflowPricer"', s.name("Policy")], "Pricer", "Pricing assumptions remain separate from the contract.")
        row = step(s, build, row, "6. Request value and tables", "QS.EVALUATE", [text("Swap.Results"), s.name("Pricer"), s.name("Trade"), s.name("Context"), s.name("Requests")], "Result", "Value, Cashflows and Sensitivities are listed on Inputs.")
        for target, label in [(0, "fixed coupon"), (1, "floating spread")]:
            row = step(s, build, row, f"7.{target+1} Select {label}", "QS.REQUEST.FAIR.RATE", [str(target)], f"Request{target}", f"Zero-based leg {target}; one fair-rate target per evaluation.")
            row = step(s, build, row, f"8.{target+1} Solve {label}", "QS.EVALUATE", [text(f"Swap.Fair{target}"), s.name("Pricer"), s.name("Trade"), s.name("Context"), s.name(f"Request{target}")], f"Fair{target}", "Only the selected leg's quote is adjusted to zero total PV.")
        for index, key, source in [("SOFR", "SofrCurve", "Context"), ("ESTR", "EstrCurve", "EstrContext")]:
            row = step(s, build, row, "9. Inspect " + index, "QS.CURVE.GET", [s.name(source), text(index)], key, "Reads the curve from its matching collateral context.")
        result = "4 Results"
        s.sheet(result, "Read the swap results", "Green cells contain financial measures. The Build tab explains how each native result was produced.", (34, 24, 65), "#21845B")
        s.headers(result, 6, ["Measure", "Calculated value", "Interpretation"])
        s.result(result, 7, "Present value (USD)", formula("QS.RESULT.VALUE", s.name("Result")), "PV", "Signed monetary PV of this receive-fixed position.")
        s.result(result, 8, "Fair fixed coupon · leg 0", formula("QS.RESULT.FAIR.RATE", s.name("Fair0")), "FairFixed", "Annual fixed coupon that makes total trade PV zero.", "rate")
        s.result(result, 9, "Fair floating spread · leg 1", formula("QS.RESULT.FAIR.RATE", s.name("Fair1")), "FairFloat", "Spread over the floating index that makes total PV zero.", "rate")
        s.headers(result, 12, ["Curve check", "Calculated value", "Interpretation"])
        for row, label, expression, key, meaning, kind in [
            (13, "SOFR discount at maturity", formula("QS.CURVE.DISCOUNT", s.name("SofrCurve"), s.name("Maturity")), "SofrDiscount", "Dimensionless discount factor.", "factor"),
            (14, "ESTR discount at maturity", formula("QS.CURVE.DISCOUNT", s.name("EstrCurve"), s.name("Maturity")), "EstrDiscount", "The EUR curve is constructed but does not price the USD swap.", "factor"),
            (15, "SOFR start-to-maturity forward", formula("QS.CURVE.FORWARD", s.name("SofrCurve"), s.name("Start"), s.name("Maturity"), '"Simple"', '"Annual"'), "SofrForward", "Simple annual forward over the full interval.", "rate"),
            (16, "ESTR start-to-maturity forward", formula("QS.CURVE.FORWARD", s.name("EstrCurve"), s.name("Start"), s.name("Maturity"), '"Simple"', '"Annual"'), "EstrForward", "Same query against ESTR.", "rate"),
        ]: s.result(result, row, label, expression, key, meaning, kind)
        s.link(result, "A20", "'1 Inputs'!B9", "Edit the fixed coupon →")
        detail_sheet(s, "5 Cashflows", "Swap cashflows", s.name("Result"), "CASHFLOWS")
        detail_sheet(s, "6 Risk", "Swap market risk", s.name("Result"), "SENSITIVITIES")
        s.sheet("7 Curves", "Bootstrapped curve nodes", "Dates are ISO text; rates are decimals. Each table contains its own field headers.", (48, 18, 20, 20, 24))
        for row, label, key in [(6, "USD SOFR", "SofrCurve"), (26, "EUR ESTR", "EstrCurve")]:
            s.merge("7 Curves", f"A{row}:E{row}", label, "section")
            s.write("7 Curves", f"A{row+1}", formula("QS.CURVE.TABLE", s.name(key), '"Simple"', '"Annual"'))


LOAN_CASES = [
    ("French", False, "French", "ESTR", [], [], "Baseline monthly amortization."),
    ("Bullet", False, "Bullet", "ESTR", [], [], "Repay all principal at maturity."),
    ("25% balloon", False, "Balloon", "ESTR", [("Balloon fraction", .25)], [], "Reserve one quarter of principal for maturity."),
    ("Floating", True, "French", "ESTR", [("Amortization rate", .05)], [], "EURIBOR12m plus a spread; principal is agreed at 5%."),
    ("5-year introductory fixed", True, "French", "ESTR", [], [(60, .04, ""), (None, "=Loan_FloatingSpread", "EURIBOR12m")], "Fixed for 60 payments, then floating through maturity."),
    ("Bounded floating", True, "French", "ESTR", [("Amortization rate", .05), ("Floor", .02), ("Cap", .045)], [], "Bounded projected coupons; fair rate is not supported."),
    ("15% annual CPR", False, "French", "ESTR", [("Annual CPR", .15)], [], "Conditional prepayment reduces future principal and interest."),
    ("Borrower credit", False, "French", "ESTR", [("Credit index", "Credit:BORROWER"), ("Recovery", "=Loan_Recovery")], [], "Survival and recovery use the illustrative CDS curve."),
    ("Funded fixed", False, "French", "FUNDING", [], [], "Same fixed contract; a different pricer policy selects funding."),
    ("Funded floating", True, "French", "FUNDING", [("Amortization rate", .05)], [], "Same floating contract; discount using FUNDING."),
    ("Stepped fixed", False, "French", "ESTR", [], [(60, .03, ""), (60, .04, ""), (None, .05, "")], "Three consecutive fixed coupon segments."),
    ("Stepped then floating", False, "French", "ESTR", [], [(60, .03, ""), (60, .04, ""), (None, .01, "EURIBOR12m")], "Two fixed segments followed by a floating margin."),
]


def create_loans(path):
    routes = [("1 Inputs", "Review shared loan terms and position direction."), ("2 Cases", "Edit one case's coupon, settings or schedule in its own block."),
              ("3 Market", "Review ordinary projection, discount, funding and credit curves."), ("4 Build", "Follow the native construction and pricing steps."),
              ("5 Results", "Compare all cases; select one for the detail sheets."), ("6 Cashflows", "Inspect receipts for the selected case."),
              ("7 Risk", "Inspect risk for the same selected case."), ("8 Layouts", "Compare transpose and header options on a small curve table.")]
    with Study(path, "Loan", "Loans · compare one change at a time", "Start with the French loan. Compare repayment, floating coupons, prepayment, credit and funding, then inspect stepped schedules.", routes,
               ["Compare French, Bullet and 25% balloon on Results.", "Compare French with Funded fixed: only the discount policy changes.", "Choose Stepped then floating on Results; inspect its three fair-rate targets, Cashflows and Risk."]) as s:
        inputs(s, "1 Inputs", [
            ("Valuation date", "2026-02-24", "ValuationDate", "Valuation at origination supports the fair-rate solves.", "input"),
            ("Principal (EUR)", 250_000, "Notional", "Contractual principal; the trade does not override it.", "input_number"),
            ("Default fixed coupon", .06, "FixedRate", "Used by the fixed-rate cases unless overridden.", "input_rate"),
            ("Default floating spread", .01, "FloatingSpread", "Spread over EURIBOR12m.", "input_rate"),
            ("Start date", "2026-02-24", "Start", "Contractual origination date.", "input"),
            ("Maturity date", "2056-02-24", "Maturity", "Thirty-year teaching example.", "input"),
            ("Payment frequency", "Monthly", "Frequency", "Case schedule counts refer to payment periods.", "input"),
            ("Day counter", "Actual360", "DayCounter", "Contractual accrual and amortization convention.", "input"),
            ("Currency", "EUR", "Currency", "These market assumptions are configured in EUR.", "body"),
            ("Position side", "Receive", "Side", "Lender receipts are positive; Pay reverses the direction.", "input"),
        ], "Shared inputs apply to all cases. Blue cells are editable; each case's specific terms are on the next tab.")
        requests(s, "1 Inputs", 20)
        s.headers("1 Inputs", 27, ["Policy currency", "Ordinary discount index", ""])
        for row, index, suffix in [(28, "ESTR", "RiskFree"), (29, "FUNDING", "Funding")]:
            s.write("1 Inputs", f"A{row}", "EUR", "body", suffix + "Currency")
            s.write("1 Inputs", f"B{row}", index, "input", suffix + "Index")
        s.range("RiskFreeMapping", "1 Inputs", 28, 0, 28, 1)
        s.range("FundingMapping", "1 Inputs", 29, 0, 29, 1)
        s.sheet("2 Cases", "Case inputs · one block per agreement", "Each block keeps its contractual settings and coupon schedule together. Discount-curve choice selects a pricer policy; it is never passed to the instrument.", (26, 20, 24, 18, 18, 22), "#337AB7")
        s.sheets["2 Cases"].freeze_panes(5, 0)
        case_rows = []
        row = 6
        for index, (label, floating, repayment, discount, settings, segments, description) in enumerate(LOAN_CASES, 1):
            case_rows.append(row)
            key = f"C{index:02}"
            s.merge("2 Cases", f"A{row}:F{row}", f"{index:02}. {label}", "section")
            s.merge("2 Cases", f"A{row+1}:F{row+1}", description)
            for cell, value in [(f"A{row+2}", "Coupon / spread"), (f"C{row+2}", "Projection index"), (f"E{row+2}", "Repayment"), (f"A{row+3}", "Pricing discount curve")]:
                s.write("2 Cases", cell, value)
            rate = f"=E{row+6}" if segments else "=" + s.name("FloatingSpread" if floating else "FixedRate")
            s.write("2 Cases", f"B{row+2}", rate, "rate" if segments else "input_rate", key + "_Rate")
            market_index = f'=IF(F{row+6}="","",F{row+6})' if segments else ("EURIBOR12m" if floating else "")
            s.write("2 Cases", f"D{row+2}", market_index, "body" if segments else "input", key + "_Index")
            s.write("2 Cases", f"F{row+2}", repayment, "input", key + "_Repayment")
            s.write("2 Cases", f"B{row+3}", discount, "input", key + "_Discount")
            s.sheets["2 Cases"].data_validation(f"B{row+3}", {"validate": "list", "source": ["ESTR", "FUNDING"]})
            s.sheets["2 Cases"].data_validation(f"F{row+2}", {"validate": "list", "source": ["French", "Bullet", "Balloon"]})
            if settings:
                for column, header in [("A", "Setting"), ("B", "Value")]:
                    s.write("2 Cases", f"{column}{row+5}", header, "header")
                for offset, (setting, value) in enumerate(settings, row + 6):
                    s.write("2 Cases", f"A{offset}", setting)
                    s.write("2 Cases", f"B{offset}", value, "input" if isinstance(value, str) and not value.startswith("=") else "input_rate")
                s.range(key + "_Settings", "2 Cases", row + 5, 0, row + 5 + len(settings), 1)
            else:
                s.merge("2 Cases", f"A{row+5}:C{row+6}", "Contract settings: native defaults.")
            if segments:
                for column, header in zip("DEF", ("Periods", "Rate / spread", "Market index")):
                    s.write("2 Cases", f"{column}{row+5}", header, "header")
                for offset, (periods, coupon, market_index) in enumerate(segments, row + 6):
                    s.write("2 Cases", f"D{offset}", periods, "input")
                    s.write("2 Cases", f"E{offset}", coupon, "input_rate")
                    s.write("2 Cases", f"F{offset}", market_index, "input")
                s.range(key + "_Coupons", "2 Cases", row + 5, 3, row + 5 + len(segments), 5)
                s.merge("2 Cases", f"D{row+10}:F{row+11}", "Edit this schedule; the scalar above follows its first row. Blank index = fixed; blank final count fills maturity. Fair-rate targets follow segment order.")
            else:
                s.merge("2 Cases", f"D{row+5}:F{row+6}", "Coupon schedule: use the scalar rate / spread and projection index above.")
            footer = row + (12 if segments else 7 + max(1, len(settings)))
            s.link("2 Cases", f"A{footer}", f"'5 Results'!A{index+6}", "See this case's results →")
            row = footer + 2
        simple_market(s, "3 Market", "EUR", "ESTR", loan=True)
        build = "4 Build"
        s.sheet(build, "Build and evaluate the loan agreements", "Shared context and pricers come first. Each case then follows instrument → trade → pricer selection → evaluation. Use the links on Results to jump directly to a case.")
        s.headers(build, 6, ["Step", "Object reference", "Purpose"])
        row = s.step(build, 7, "1. Initialize pricing context", context("Loan.Context", s.name("Quotes"), s.name("Curves"), '"EUR"', '"ESTR"', s.name("Credit")), "Context", "Constructs the projection, discount, funding and credit curves.")
        for number, suffix, label in [(2, "RiskFree", "ESTR"), (4, "Funding", "FUNDING")]:
            row = step(s, build, row, f"{number}. {label} discount policy", "QS.DISCOUNT.POLICY.FIXED.INCOME", [text(f"Loan.{suffix}Policy"), s.name(suffix + "Currency"), s.name(suffix + "Index")], suffix + "Policy", "Maps EUR to an ordinary market curve.")
            row = step(s, build, row, f"{number+1}. {label} loan pricer", "QS.PRICER.CREATE", [text(f"Loan.{suffix}Pricer"), '"LoanPricer"', s.name(suffix + "Policy")], suffix + "Pricer", "Same native LoanPricer, with an explicit policy.")
        row = step(s, build, row, "6. Get ESTR for layout examples", "QS.CURVE.GET", [s.name("Context"), '"ESTR"'], "EstrCurve", "A small five-node table for the layout lesson.")
        for target in range(3):
            row = step(s, build, row, f"7.{target+1} Fair-rate request {target}", "QS.REQUEST.FAIR.RATE", [str(target)], f"Request{target}", "Returns one requested segment's scalar; earlier segments are bootstrapped first.")
        result_keys, build_rows = [], []
        for index, (label, floating, repayment, discount, settings, segments, description) in enumerate(LOAN_CASES, 1):
            key = f"C{index:02}"
            build_rows.append(row)
            s.merge(build, f"A{row}:C{row}", f"{index:02}. {label}", "section")
            s.link(build, f"A{row+1}", f"'2 Cases'!A{case_rows[index-1]}", "Edit these case inputs →")
            row += 3
            args = [text(f"Loan.{key}.Instrument"), *[s.name(k) for k in ("Start", "Maturity", "Currency", "Notional")],
                    s.name(key + "_Rate"), s.name(key + "_Index"), s.name(key + "_Repayment"), s.name("Frequency"), s.name("DayCounter"),
                    s.name(key + "_Settings") if settings else "", s.name(key + "_Coupons") if segments else ""]
            row = step(s, build, row, "1. Define this instrument", "QS.INSTRUMENT.LOAN", args, key + "_Instrument", description)
            row = step(s, build, row, "2. Create this position", "QS.TRADE.CREATE", [text(f"Loan.{key}.Trade"), s.name(key + "_Instrument"), s.name("ValuationDate"), s.name("Side")], key + "_Trade", "The contractual principal stays in the instrument.")
            row = s.step(build, row, "3. Select explicit pricer", f'=IF({s.name(key+"_Discount")}="FUNDING",{s.name("FundingPricer")},{s.name("RiskFreePricer")})', key + "_Pricer", "The case's discount choice selects an existing policy-configured pricer.")
            row = step(s, build, row, "4. Evaluate value and tables", "QS.EVALUATE", [text(f"Loan.{key}.Result"), s.name(key + "_Pricer"), s.name(key + "_Trade"), s.name("Context"), s.name("Requests")], key + "_Result", "Future receipt PV excludes the original principal advance.")
            result_keys.append(key + "_Result")
            if label != "Bounded floating":
                for target in range(len(segments) if segments else 1):
                    row = step(s, build, row, f"5.{target+1} Solve segment {target}", "QS.EVALUATE", [text(f"Loan.{key}.Fair{target}"), s.name(key + "_Pricer"), s.name(key + "_Trade"), s.name("Context"), s.name(f"Request{target}")], key + f"_Fair{target}", "Repay remaining principal at each prefix end, lock earlier solved rates, then extend.")
            row += 2
        results = "5 Results"
        s.sheet(results, "Compare the loan results", "PV is future receipt value in EUR; net origination value subtracts principal. Fair-rate columns are annual decimal coupon / spread levels. Blank means that segment does not exist.", (29, 17, 17, 15, 15, 15, 12, 12), "#21845B")
        s.headers(results, 6, ["Case", "Receipt PV (EUR)", "PV less advance", "Fair segment 0", "Fair segment 1", "Fair segment 2", "Edit inputs", "Read build"])
        for index, (label, _, _, _, _, segments, _) in enumerate(LOAN_CASES, 1):
            row_result = index + 6
            key = f"C{index:02}"
            s.write(results, f"A{row_result}", label)
            s.write(results, f"B{row_result}", formula("QS.RESULT.VALUE", s.name(key + "_Result")), "money", key + "_PV")
            s.write(results, f"C{row_result}", f"={s.name(key+'_PV')}-{s.name('Notional')}", "money", key + "_Net")
            for target in range(3):
                column = "DEF"[target]
                if label == "Bounded floating" and target == 0:
                    s.write(results, f"{column}{row_result}", "Bounded: n/a", "note")
                elif label != "Bounded floating" and target < (len(segments) if segments else 1):
                    s.write(results, f"{column}{row_result}", formula("QS.RESULT.FAIR.RATE", s.name(key + f"_Fair{target}")), "rate", key + f"_FairRate{target}")
                else:
                    s.write(results, f"{column}{row_result}", None, "result")
            s.link(results, f"G{row_result}", f"'2 Cases'!A{case_rows[index-1]}", "Edit →")
            s.link(results, f"H{row_result}", f"'4 Build'!A{build_rows[index-1]}", "Build →")
            s.sheets[results].set_row(row_result - 1, 34)
        selected_case(s, build, row, results, [case[0] for case in LOAN_CASES], result_keys, 22)
        s.merge(results, "A26:H28", "FairRate(index) returns one scalar. For mixed or stepped loans, earlier rate segments are solved and fixed before the selected segment is solved. The original agreement remains unchanged. Bounded coupons have no fair-rate output.")
        detail_sheet(s, "6 Cashflows", "Selected loan · expected receipts", s.name("SelectedResult"), "CASHFLOWS", "'5 Results'!B22")
        detail_sheet(s, "7 Risk", "Selected loan · market risk", s.name("SelectedResult"), "SENSITIVITIES", "'5 Results'!B22")
        s.sheet("8 Layouts", "Table layout · transpose and headers", "The same small ESTR curve table is shown four ways. Header removal happens before transposition. All other report tables accept the same flags.", (28, 18, 20, 20, 24, 24))
        for row_layout, label, transpose, headers in [(6, "Default · rows with headers", "FALSE", "TRUE"), (16, "Rows without headers", "FALSE", "FALSE"), (26, "Transposed with headers", "TRUE", "TRUE"), (36, "Transposed without headers", "TRUE", "FALSE")]:
            s.merge("8 Layouts", f"A{row_layout}:F{row_layout}", label, "section")
            s.write("8 Layouts", f"A{row_layout+1}", formula("QS.CURVE.TABLE", s.name("EstrCurve"), '"Simple"', '"Annual"', transpose, headers))


def create_bonds(path):
    routes = [("1 Inputs", "Set one contract and three alternative bond anchors."), ("2 Market", "Review and build the SOFR zero curve."),
              ("3 Build", "Create one instrument, then separately anchored trades and results."), ("4 Results", "Compare PV, duration and Z-spread; choose a detail case."),
              ("5 Cashflows", "Inspect the selected anchored position."), ("6 Risk", "Inspect curve and spread derivatives for the same position.")]
    anchors = [("Zero spread", "ZSpread", 0.0), ("100 bp spread", "ZSpread", .01), ("Clean price 100", "CleanPrice", 100.0)]
    with Study(path, "Bond", "Bond anchors · contract, trade and value", "Use one fixed bond to compare spread and price anchors. Pricing anchors belong to trades; the discount policy belongs to the pricer.", routes,
               ["Compare Zero spread and 100 bp spread on Results.", "Change the clean-price anchor on Inputs and inspect the solved Z-spread.", "Choose a case on Results, then inspect its cashflows and risk."]) as s:
        inputs(s, "1 Inputs", [
            ("Valuation date", "2026-02-24", "ValuationDate", "Market and execution date.", "input"),
            ("Start date", "2026-02-24", "Start", "Contractual start date.", "input"),
            ("Maturity date", "2036-02-24", "Maturity", "Final contractual principal repayment.", "input"),
            ("Face principal (USD)", 1_000_000, "Notional", "Principal used to construct the instrument's cashflows.", "input_number"),
            ("Fixed coupon", .04, "Coupon", "Annual decimal coupon.", "input_rate"),
            ("Position side", "Receive", "Side", "Receive holds the bond; Pay reverses the position.", "input"),
        ], "Blue cells are editable. Clean/dirty price anchors are per 100 face; spread anchors are annual decimals.")
        s.headers("1 Inputs", 16, ["Anchor case", "Native anchor type", "Anchor value"])
        for index, (label, anchor, value) in enumerate(anchors, 1):
            row = 16 + index
            s.write("1 Inputs", f"A{row}", label)
            s.write("1 Inputs", f"B{row}", anchor, "input", f"C{index:02}_Anchor")
            s.write("1 Inputs", f"C{row}", value, "input_rate" if anchor == "ZSpread" else "input_number", f"C{index:02}_AnchorValue")
        requests(s, "1 Inputs", 22, ("Value", "ModifiedDuration", "ZSpread", "Cashflows", "Sensitivities"))
        s.headers("1 Inputs", 30, ["Policy currency", "Ordinary discount index", ""])
        s.write("1 Inputs", "A31", "USD", "body", "DiscountCurrency")
        s.write("1 Inputs", "B31", "SOFR", "input", "DiscountIndex")
        s.range("DiscountMapping", "1 Inputs", 31, 0, 31, 1)
        simple_market(s, "2 Market", "USD", "SOFR")
        build = "3 Build"
        s.sheet(build, "Build one contract and three positions", "The shared instrument is created once. Each trade adds its own anchor, and the explicit BondPricer evaluates each position.")
        s.headers(build, 6, ["Step", "Object reference", "Purpose"])
        row = s.step(build, 7, "1. Initialize market context", context("Bond.Context", s.name("Quotes"), s.name("Curves"), '"USD"', '"SOFR"'), "Context", "Constructs the visible SOFR zero curve.")
        row = step(s, build, row, "2. Define the bond instrument", "QS.INSTRUMENT.FIXED.BOND", [text("Bond.Instrument"), s.name("Start"), s.name("Maturity"), '"USD"', "", s.name("Notional"), s.name("Coupon"), '"Actual360"', '"Compounded"', '"Semiannual"', '"Semiannual"'], "Instrument", "No position side or pricing anchor is stored in this contract.")
        row = step(s, build, row, "3. Choose discount policy", "QS.DISCOUNT.POLICY.FIXED.INCOME", [text("Bond.Policy"), s.name("DiscountCurrency"), s.name("DiscountIndex")], "Policy", "Maps USD to the ordinary SOFR curve.")
        row = step(s, build, row, "4. Create native bond pricer", "QS.PRICER.CREATE", [text("Bond.Pricer"), '"BondPricer"', s.name("Policy")], "Pricer", "The same configured pricer evaluates all three positions.")
        result_keys, build_rows = [], []
        for index, (label, _, _) in enumerate(anchors, 1):
            key = f"C{index:02}"
            build_rows.append(row)
            row = step(s, build, row, f"5.{index} Position · {label}", "QS.TRADE.CREATE", [text(f"Bond.{key}.Trade"), s.name("Instrument"), s.name("ValuationDate"), s.name("Side"), "", "", s.name(key + "_Anchor"), s.name(key + "_AnchorValue")], key + "_Trade", "The trade supplies the bond's price or spread anchor.")
            row = step(s, build, row, f"6.{index} Evaluate · {label}", "QS.EVALUATE", [text(f"Bond.{key}.Result"), s.name("Pricer"), s.name(key + "_Trade"), s.name("Context"), s.name("Requests")], key + "_Result", "Produces the measures listed on Inputs.")
            result_keys.append(key + "_Result")
        result = "4 Results"
        s.sheet(result, "Compare the anchored bond positions", "PV is monetary USD value. Duration is measured in years. Z-spread is an annual decimal spread; the same contract is used in all cases.", (30, 22, 18, 18, 16, 16), "#21845B")
        s.headers(result, 6, ["Case", "PV (USD)", "Modified duration", "Z-spread", "Edit anchor", "Read build"])
        for index, (label, _, _) in enumerate(anchors, 1):
            key = f"C{index:02}"
            row_result = index + 6
            s.write(result, f"A{row_result}", label)
            for column, method, suffix, kind in [("B", "VALUE", "PV", "money"), ("C", "MODIFIED.DURATION", "Duration", "number"), ("D", "Z.SPREAD", "ZSpread", "rate")]:
                s.write(result, f"{column}{row_result}", formula("QS.RESULT." + method, s.name(key + "_Result")), kind, key + "_" + suffix)
            s.link(result, f"E{row_result}", f"'1 Inputs'!C{index+16}", "Edit →")
            s.link(result, f"F{row_result}", f"'3 Build'!A{build_rows[index-1]}", "Build →")
            s.sheets[result].set_row(row_result - 1, 34)
        selected_case(s, build, row + 2, result, [anchor[0] for anchor in anchors], result_keys, 13)
        s.merge(result, "A17:F19", "A clean-price anchor fixes the quoted price and solves a consistent spread. A Z-spread anchor fixes that spread and produces a monetary PV. Neither quotation is a replacement for position sizing or direction.")
        detail_sheet(s, "5 Cashflows", "Selected bond · cashflows", s.name("SelectedResult"), "CASHFLOWS", "'4 Results'!B13")
        detail_sheet(s, "6 Risk", "Selected bond · curve and spread risk", s.name("SelectedResult"), "SENSITIVITIES", "'4 Results'!B13")


def create_historical(path):
    routes = [("1 Inputs", "Set one unchanged swap and two timestamp queries."), ("2 Markets", "Edit the two dated zero-rate observations."),
              ("3 Archive", "Build each market, capture its snapshot and collect the archive."), ("4 Pricing", "Select historical contexts and value the same trade."),
              ("5 Results", "Compare requested time, matched time, valuation date and PV.")]
    with Study(path, "History", "Historical pricing · follow the timestamps", "Record two coherent markets, then reprice one unchanged swap. A timestamp query selects a recorded market; it does not roll its valuation date.", routes,
               ["Compare the two matched markets on Results.", "Move query 2 after the second observation and change Exact to Previous.", "Edit the second market's zero rates and compare the same trade's PV."]) as s:
        inputs(s, "1 Inputs", [
            ("Trade date", "2026-01-15", "TradeDate", "The position remains unchanged across both market observations.", "input"),
            ("Start date", "2026-01-19", "Start", "Starts after both observations, so no historical fixing is needed.", "input"),
            ("Maturity date", "2031-01-19", "Maturity", "Final contractual date.", "input"),
            ("Notional (USD)", 1_000_000, "Notional", "Contractual principal.", "input_number"),
            ("Fixed coupon", .04, "FixedRate", "Annual fixed coupon received.", "input_rate"),
            ("Side", "Receive", "Side", "Receive fixed and pay SOFR.", "input"),
        ], "Contract and trade inputs are common to both valuations. Timestamp queries are separate inputs below.")
        s.headers("1 Inputs", 16, ["Query", "Requested UTC timestamp", "Matching rule"])
        for index, day in [(1, 15), (2, 16)]:
            s.write("1 Inputs", f"A{16+index}", f"Query {index}")
            s.write("1 Inputs", f"B{16+index}", f"2026-01-{day}T16:00:00Z", "input", f"Query{index}")
            s.write("1 Inputs", f"C{16+index}", "Exact", "input", f"Rule{index}")
            s.sheets["1 Inputs"].data_validation(f"C{16+index}", {"validate": "list", "source": ["Exact", "Previous", "Closest"]})
        s.merge("1 Inputs", "A21:C23", "Previous selects the latest observation at or before the request. Closest can select a future observation. The selected market's reference date remains the valuation date.")
        market = "2 Markets"
        s.sheet(market, "Two dated market observations", "Both markets use a USD SOFR explicit zero curve, Actual365, continuous compounding, linear interpolation and no extrapolation.", (34, 30, 30), "#337AB7")
        s.headers(market, 6, ["Observation", "First market", "Second market"])
        s.write(market, "A7", "Market reference date")
        s.write(market, "A8", "Observation timestamp (UTC)")
        s.headers(market, 11, ["Pillar", "First zero curve", "Second zero curve"])
        for row, pillar in enumerate(("0D", "1Y", "10Y"), 12):
            s.write(market, f"A{row}", pillar, "input")
        s.range("Pillars", market, 12, 0, 14, 0)
        for index, column, day in [(1, "B", 15), (2, "C", 16)]:
            s.write(market, f"{column}7", f"2026-01-{day}", "input", f"Date{index}")
            s.write(market, f"{column}8", f"2026-01-{day}T16:00:00Z", "input", f"Timestamp{index}")
            for row, rate in enumerate(([.04, .039, .038] if index == 1 else [.041, .040, .039]), 12):
                s.write(market, f"{column}{row}", rate, "input_rate")
            s.range(f"Rates{index}", market, 12, index, 14, index)
        archive = "3 Archive"
        s.sheet(archive, "Construct and archive each market", "Each snapshot captures its own quote date and curve assumptions. Read the two market blocks in order, then the archive-creation step.")
        s.headers(archive, 6, ["Step", "Object reference", "Purpose"])
        row = 7
        for index in [1, 2]:
            row = step(s, archive, row, f"{index}.1 Create dated quote store", "QS.QUOTES.CREATE", [text(f"History.Quotes{index}"), s.name(f"Date{index}")], f"Quotes{index}", "The quote-store reference date is the valuation date.")
            row = step(s, archive, row, f"{index}.2 Create configurations", "QS.CURVE.CONFIGS.CREATE", [text(f"History.Curves{index}")], f"InitialCurves{index}", "A separate configuration is used for each recorded market.")
            row = step(s, archive, row, f"{index}.3 Configure dated zero curve", "QS.CURVE.CONFIGS.ADD.ZERO", [s.name(f"InitialCurves{index}"), '"SOFR"', '"Actual365"', '"Linear"', "FALSE", s.name("Pillars"), s.name(f"Rates{index}"), '"Continuous"', '"Annual"'], f"Curves{index}", "Use the visible curve from the matching observation.")
            row = s.step(archive, row, f"{index}.4 Initialize dated market", context(f"History.Context{index}", s.name(f"Quotes{index}"), s.name(f"Curves{index}"), '"USD"', '"SOFR"'), f"OriginalContext{index}", "Rebuild the curve using that market's reference date.")
            row = step(s, archive, row, f"{index}.5 Capture market snapshot", "QS.SNAPSHOT.FROM.CONTEXT", [text(f"History.Snapshot{index}"), s.name(f"Timestamp{index}"), s.name(f"OriginalContext{index}")], f"Snapshot{index}", "Record the complete construction inputs at its observation timestamp.")
        s.headers(archive, row, ["Captured snapshot", "Object reference", ""])
        for index in [1, 2]:
            s.write(archive, f"A{row+index}", f"Market {index}")
            s.write(archive, f"B{row+index}", "=" + s.name(f"Snapshot{index}"), "handle")
        s.range("Snapshots", archive, row + 1, 1, row + 2, 1)
        step(s, archive, row + 4, "3. Collect snapshot archive", "QS.SNAPSHOTS.CREATE", [text("History.Archive"), s.name("Snapshots")], "Archive", "The archive consumes both captured snapshots.")
        build = "4 Pricing"
        s.sheet(build, "Reprice one trade against selected markets", "Build one instrument, trade and policy-configured pricer. Each timestamp query then selects its own context before valuation.")
        s.headers(build, 6, ["Step", "Object reference", "Purpose"])
        row = step(s, build, 7, "1. Define unchanged instrument", "QS.INSTRUMENT.SWAP", [text("History.Instrument"), s.name("Start"), s.name("Maturity"), '"USD"', '"SOFR"', s.name("Notional"), s.name("FixedRate"), "0", '"Actual360"', '"Simple"', '"Semiannual"', '"Semiannual"', '"Quarterly"'], "Instrument", "Contractual terms are identical for both market observations.")
        row = step(s, build, row, "2. Create unchanged position", "QS.TRADE.CREATE", [text("History.Trade"), s.name("Instrument"), s.name("TradeDate"), s.name("Side")], "Trade", "Same execution date, position size and direction.")
        row = step(s, build, row, "3. Choose CSA discount policy", "QS.DISCOUNT.POLICY.CSA", [text("History.Policy"), '"USD"', '"SOFR"'], "Policy", "Use the same native discount policy in both markets.")
        row = step(s, build, row, "4. Create the native pricer", "QS.PRICER.CREATE", [text("History.Pricer"), '"DiscountedCashflowPricer"', s.name("Policy")], "Pricer", "Only the selected market changes between evaluations.")
        for index in [1, 2]:
            row = step(s, build, row, f"{index+4}.1 Select timestamp", "QS.SNAPSHOT.SELECTOR", [text(f"History.Selector{index}"), s.name(f"Query{index}"), s.name(f"Rule{index}"), '"Mid"'], f"Selector{index}", "The requested timestamp and matching rule are editable on Inputs.")
            row = step(s, build, row, f"{index+4}.2 Rebuild selected context", "QS.CONTEXT.FROM.SNAPSHOT", [text(f"History.Selected{index}"), s.name("Archive"), s.name(f"Selector{index}")], f"Context{index}", "The market's own reference date is preserved.")
            row = step(s, build, row, f"{index+4}.3 Evaluate unchanged trade", "QS.EVALUATE", [text(f"History.Result{index}"), s.name("Pricer"), s.name("Trade"), s.name(f"Context{index}"), '"Value"'], f"Result{index}", "The same position and pricer consume this selected market.")
        result = "5 Results"
        s.sheet(result, "Compare requested and selected markets", "A later query time does not create a later valuation date. Read the actual matched timestamp and market date before comparing PV.", (38, 36, 36), "#21845B")
        s.headers(result, 6, ["Measure", "Query 1", "Query 2"])
        for row, label in [(7, "Requested timestamp"), (8, "Actual matched timestamp"), (9, "Actual valuation date"), (10, "Matching rule"), (12, "Present value (USD)"), (13, "Change from query 1 (USD)")]:
            s.write(result, f"A{row}", label)
            s.sheets[result].set_row(row - 1, 36)
        for index, column in [(1, "B"), (2, "C")]:
            s.write(result, f"{column}7", "=" + s.name(f"Query{index}"), "result")
            s.write(result, f"{column}8", formula("QS.CONTEXT.SNAPSHOT.TIMESTAMP", s.name(f"Context{index}")), "result", f"Matched{index}")
            s.write(result, f"{column}9", formula("QS.CONTEXT.EVALUATION.DATE", s.name(f"Context{index}")), "result", f"ActualDate{index}")
            s.write(result, f"{column}10", "=" + s.name(f"Rule{index}"), "result")
            s.write(result, f"{column}12", formula("QS.RESULT.VALUE", s.name(f"Result{index}")), "money", f"PV{index}")
            s.write(result, f"{column}13", f"={s.name(f'PV{index}')}-{s.name('PV1')}", "money")
        s.link(result, "A17", "'1 Inputs'!B18", "Change timestamp queries →")


def create_interpolation(path):
    routes = [("1 Inputs", "Edit curve nodes, query points, surface coordinates and dates."), ("2 Interpolation", "Compare methods and check the bilinear example."),
              ("3 Schedules", "Compare unadjusted, adjusted and transposed date lists.")]
    with Study(path, "Interpolation", "Interpolation and schedules · see what changes", "Compare three native one-dimensional methods, verify a bilinear surface, then inspect date-list layouts.", routes,
               ["Change one positive discount-factor node and compare the three methods.", "Move the bilinear query and compare the native and analytic values.", "Compare the same schedule before and after TARGET / Following adjustment."]) as s:
        inputs_sheet = "1 Inputs"
        s.sheet(inputs_sheet, "Numerical inputs", "Blue cells are editable. Positive discount factors support log-linear interpolation. The bilinear surface is 2 + x + 3y + xy.", (30, 24, 32), "#337AB7")
        s.headers(inputs_sheet, 6, ["Time", "Discount factor", ""])
        for row, (x, y) in enumerate([(0, 1), (1, .97), (3, .91), (5, .85), (10, .70)], 7):
            s.write(inputs_sheet, f"A{row}", x, "input")
            s.write(inputs_sheet, f"B{row}", y, "input_number")
        s.range("Times", inputs_sheet, 7, 0, 11, 0)
        s.range("Factors", inputs_sheet, 7, 1, 11, 1)
        s.headers(inputs_sheet, 14, ["Query point", "", ""])
        for row, point in enumerate([0, .5, 1, 2, 4, 7, 10], 15):
            s.write(inputs_sheet, f"A{row}", point, "input")
        s.headers(inputs_sheet, 24, ["Surface x", "Surface y", "Surface value"])
        for row, (x, y) in enumerate([(0, 0), (0, 2), (1, 0), (1, 2)], 25):
            s.write(inputs_sheet, f"A{row}", x, "input")
            s.write(inputs_sheet, f"B{row}", y, "input")
            s.write(inputs_sheet, f"C{row}", f"=2+A{row}+3*B{row}+A{row}*B{row}", "number")
        for key, column in [("SurfaceX", 0), ("SurfaceY", 1), ("SurfaceValues", 2)]:
            s.range(key, inputs_sheet, 25, column, 28, column)
        for row, label, value, key in [(31, "Query x", .4, "QueryX"), (32, "Query y", .7, "QueryY"), (35, "Schedule start", "2026-02-28", "Start"), (36, "Schedule end", "2027-02-28", "End"), (37, "Frequency", "Quarterly", "Frequency"), (38, "Calendar", "TARGET", "Calendar"), (39, "Business-day convention", "Following", "Convention")]:
            s.input(inputs_sheet, row, label, value, key, "Used by the corresponding result tab.")
        result = "2 Interpolation"
        s.sheet(result, "Compare native interpolation methods", "The same nodes feed all three methods. Query points must stay inside the node range because extrapolation is disabled.", (24, 24, 24, 24), "#21845B")
        s.headers(result, 6, ["Query point", "Linear", "Log-linear", "Cubic spline"])
        for row in range(7, 14):
            s.write(result, f"A{row}", f"='{inputs_sheet}'!A{row+8}", "number")
            for column, method in [("B", "LINEAR"), ("C", "LOG.LINEAR"), ("D", "CUBIC.SPLINE")]:
                s.write(result, f"{column}{row}", formula("QS.INTERPOLATE." + method, f"A{row}", s.name("Times"), s.name("Factors"), "FALSE"), "factor", f"{column}{row}")
        s.headers(result, 17, ["Bilinear surface", "Calculated value", "", ""])
        s.write(result, "A18", "Native interpolation")
        s.write(result, "B18", formula("QS.INTERPOLATE.BILINEAR", s.name("QueryX"), s.name("QueryY"), s.name("SurfaceX"), s.name("SurfaceY"), s.name("SurfaceValues"), '"Reject"'), "number", "Bilinear")
        s.write(result, "A19", "Analytic value")
        s.write(result, "B19", f"=2+{s.name('QueryX')}+3*{s.name('QueryY')}+{s.name('QueryX')}*{s.name('QueryY')}", "number", "Analytic")
        s.write(result, "A20", "Difference")
        s.write(result, "B20", f"={s.name('Bilinear')}-{s.name('Analytic')}", "number", "Difference")
        chart = s.workbook.add_chart({"type": "scatter", "subtype": "straight_with_markers"})
        for column, name, color in [(1, "Linear", "#176B75"), (2, "Log-linear", "#337AB7"), (3, "Cubic spline", "#B27C2C")]:
            chart.add_series({"name": name, "categories": [result, 6, 0, 12, 0], "values": [result, 6, column, 12, column], "line": {"color": color}, "marker": {"type": "circle", "size": 4}})
        chart.set_title({"name": "Same nodes · different interpolation"})
        chart.set_x_axis({"name": "Time"})
        chart.set_y_axis({"name": "Discount factor"})
        chart.set_legend({"position": "bottom"})
        chart.set_size({"width": 650, "height": 310})
        s.sheets[result].insert_chart("A23", chart)
        schedules = "3 Schedules"
        s.sheet(schedules, "The same schedule in different layouts", "Both date columns use the same start, end and frequency. The adjusted list adds TARGET / Following. Lists have no headers; transpose changes orientation only.", (20, 20, 20, 20, 20), "#21845B")
        s.sheets[schedules].set_column("F:AZ", 16)
        s.write(schedules, "A6", "Unadjusted", "header")
        s.write(schedules, "C6", "TARGET / Following", "header")
        s.write(schedules, "A7", formula("QS.SCHEDULE", s.name("Start"), s.name("End"), s.name("Frequency")))
        s.write(schedules, "C7", formula("QS.SCHEDULE.ADJUSTED", s.name("Start"), s.name("End"), s.name("Frequency"), s.name("Calendar"), s.name("Convention")))
        s.merge(schedules, "A16:E16", "The unadjusted list transposed into one row", "section")
        s.write(schedules, "A17", formula("QS.SCHEDULE", s.name("Start"), s.name("End"), s.name("Frequency"), "TRUE"))


def validation_names(example):
    """Stable workbook names keep live checks independent of the visual layout."""
    if example == "portfolio_aggregation_example":
        return (["Portfolio_" + key for key in ("Context", "Policy", "Pricer", "Book", "Result")]
                + [f"Portfolio_C{index:02}_{key}" for index in range(1, 7) for key in ("Instrument", "Trade", "Result")],
                ["Portfolio_" + key for key in ("PV", "SumPV", "PVDifference", "DepositRate", "SofrSpread")]
                + [f"Portfolio_C{index:02}_PV" for index in range(1, 7)])
    if example == "swap_pricing_example":
        return (["Swap_" + key for key in ("Context", "EstrContext", "SofrCurve", "EstrCurve", "Instrument", "Trade", "Policy", "Pricer", "Result", "Fair0", "Fair1")],
                ["Swap_" + key for key in ("PV", "FairFixed", "FairFloat", "SofrDiscount", "EstrDiscount", "SofrForward", "EstrForward")])
    if example == "historical_pricing_example":
        return (["History_" + key for key in ("Archive", "Trade", "Pricer", "Context1", "Context2", "Result1", "Result2")],
                ["History_PV1", "History_PV2"])
    if example == "loan_pricing_example":
        handles = ["Loan_" + key for key in ("Context", "RiskFreePricer", "FundingPricer", "SelectedResult")]
        numbers = []
        for index, case in enumerate(LOAN_CASES, 1):
            key = f"Loan_C{index:02}_"
            handles += [key + "Instrument", key + "Trade", key + "Result"]
            numbers += [key + "PV", key + "Net"]
            if case[0] != "Bounded floating":
                numbers += [key + f"FairRate{target}" for target in range(len(case[5]) or 1)]
        return handles, numbers
    if example == "bond_pricing_example":
        return (["Bond_" + key for key in ("Context", "Instrument", "Pricer", "SelectedResult")],
                [f"Bond_C{index:02}_{key}" for index in range(1, 4) for key in ("PV", "Duration", "ZSpread")])
    if example == "interpolation_example":
        return [], ([f"Interpolation_{column}{row}" for column in "BCD" for row in range(7, 14)]
                    + ["Interpolation_Bilinear", "Interpolation_Analytic", "Interpolation_Difference"])
    raise ValueError(f"Unknown example: {example}")


def validate_in_excel(path, xll_path):
    """Check native results, selected-case spills and unchanged F9 recalculation."""
    from win32com.client import DispatchEx

    excel = DispatchEx("Excel.Application")
    workbook = None
    try:
        excel.Visible = False
        excel.DisplayAlerts = False
        if not excel.RegisterXLL(str(xll_path)):
            raise RuntimeError(f"Excel could not load {xll_path}")
        # xlNormalLoad (0): do not attempt repair or data recovery.
        workbook = excel.Workbooks.Open(str(path.resolve()), UpdateLinks=0, ReadOnly=True, CorruptLoad=0)
        workbook.ForceFullCalculation = True
        excel.CalculateFullRebuild()

        def cell(name):
            return workbook.Names.Item(name).RefersToRange

        def value(name):
            return cell(name).Value2

        handle_names, number_names = validation_names(path.stem)
        before = [value(name) for name in handle_names]
        for name, result in zip(handle_names, before):
            if not isinstance(result, str) or not result.startswith("QSObject:"):
                raise RuntimeError(f"{name} did not return a native object handle: {result!r}")
        numbers = [value(name) for name in number_names]
        for name, result in zip(number_names, numbers):
            if not isinstance(result, (int, float)) or isinstance(result, bool) or not math.isfinite(result):
                raise RuntimeError(f"{name} did not return a finite number: {result!r}")
            if bool(excel.WorksheetFunction.IsError(cell(name))):
                raise RuntimeError(f"{name} returned an Excel error: {result!r}")

        if path.stem == "interpolation_example":
            if abs(value("Interpolation_Difference")) > 1e-10:
                raise RuntimeError("Bilinear result disagrees with the analytic surface")
        elif path.stem == "historical_pricing_example":
            for index in (1, 2):
                if value(f"History_ActualDate{index}") != value(f"History_Date{index}"):
                    raise RuntimeError("Historical valuation date does not match its selected market")
                matched = datetime.fromisoformat(value(f"History_Matched{index}").replace("Z", "+00:00"))
                recorded = datetime.fromisoformat(value(f"History_Timestamp{index}").replace("Z", "+00:00"))
                if matched != recorded:
                    raise RuntimeError("Exact timestamp query did not match its observation")
        else:
            for sheet in workbook.Worksheets:
                if sheet.Name.endswith((" Cashflows", " Risk")):
                    expected = "Risk factor" if sheet.Name.endswith(" Risk") else "Payment date"
                    if sheet.Range("A7").Value2 != expected:
                        raise RuntimeError(f"{sheet.Name}!A7 did not spill its expected header")
            if path.stem in ("loan_pricing_example", "bond_pricing_example"):
                prefix = "Loan" if path.stem == "loan_pricing_example" else "Bond"
                selected = cell(prefix + "_SelectedCase")
                original = selected.Value2
                try:
                    selected.Value2 = "Stepped then floating" if prefix == "Loan" else "Clean price 100"
                    excel.CalculateFullRebuild()
                    expected = value(prefix + ("_C12_Result" if prefix == "Loan" else "_C03_Result"))
                    if value(prefix + "_SelectedResult") != expected:
                        raise RuntimeError("The detail selector did not choose the requested result")
                    for sheet in workbook.Worksheets:
                        if sheet.Name.endswith((" Cashflows", " Risk")):
                            if sheet.Range("B6").Value2 != selected.Value2:
                                raise RuntimeError("Cashflows and risk do not follow the same selected case")
                finally:
                    selected.Value2 = original
                    excel.CalculateFullRebuild()
            if path.stem == "loan_pricing_example":
                if value("Loan_C09_PV") >= value("Loan_C01_PV"):
                    raise RuntimeError("Higher funding rates did not reduce fixed-loan PV")
                layouts = workbook.Worksheets("8 Layouts")
                if layouts.Range("A7").Value2 != "Pillar" or layouts.Range("A27").Value2 != "Pillar":
                    raise RuntimeError("Curve-table layouts lost their headers")
                for first, second in (("A17", "A8"), ("A37", "B27")):
                    if layouts.Range(first).Value2 != layouts.Range(second).Value2:
                        raise RuntimeError("Header-free and transposed curve values do not align")
            if path.stem == "portfolio_aggregation_example":
                if abs(value("Portfolio_PVDifference")) > 1e-8:
                    raise RuntimeError("Portfolio PV does not reconcile to individual positions")
                coupon_sheet = workbook.Worksheets("5 Coupons")
                if "%" in coupon_sheet.Range("I8").NumberFormat or "%" not in coupon_sheet.Range("J8").NumberFormat:
                    raise RuntimeError("Coupon notionals and rates have incorrect display units")
                rows = coupon_sheet.Range("A7:R200").Value2
                columns = {label: index for index, label in enumerate(rows[0])}
                coupons = [row for row in rows[1:] if row[0] is not None]
                deposits = [row for row in coupons if row[columns["Type"]] == "FixedRateCoupon"
                            and row[columns["Accrual end"]] == value("Portfolio_DepositMaturity")]
                if len(deposits) != 1 or not math.isclose(deposits[0][columns["Coupon rate"]], value("Portfolio_DepositRate"), abs_tol=1e-12):
                    raise RuntimeError("Deposit coupons did not combine at their weighted rate")
                floating = [row for row in coupons if row[columns["Type"]] == "FloatingRateCoupon"]
                if {row[columns["Fixing index"]] for row in floating} != {"SOFR", "TermSOFR3m"}:
                    raise RuntimeError("Portfolio cashflows lost their distinct fixing indices")
                paid = [row for row in floating if row[columns["Fixing index"]] == "SOFR" and row[columns["Signed notional"]] < 0]
                if not paid or any(not math.isclose(row[columns["Spread"]], value("Portfolio_SofrSpread"), abs_tol=1e-12) for row in paid):
                    raise RuntimeError("SOFR coupons did not combine at their weighted spread")
                summary = workbook.Worksheets("6 Cashflows").Range("A7:F100").Value2
                if summary[0] != ("Payment date", "Currency", "Interest", "Capital", "Other", "Total"):
                    raise RuntimeError("Portfolio cashflow summary has unexpected columns")
                payments = [row for row in summary[1:] if row[0] is not None]
                if not payments or any(not math.isclose(sum(row[2:5]), row[5], abs_tol=1e-8) for row in payments):
                    raise RuntimeError("Portfolio interest and capital do not reconcile to payments")
        excel.Calculate()
        if [value(name) for name in handle_names] != before:
            raise RuntimeError("Unchanged recalculation changed object handles")
        if any(abs(value(name) - previous) > 1e-8 for name, previous in zip(number_names, numbers)):
            raise RuntimeError("Unchanged recalculation changed numerical results")
    finally:
        if workbook is not None:
            workbook.Close(SaveChanges=False)
        excel.Quit()


def publish_workbook(source, destination):
    """Publish atomically with the destination folder's inherited permissions.

    Windows temporary directories can grant access only to their creator.
    Moving a file out of one preserves that private ACL. Copy the content to
    a normal sibling file first so the published workbook inherits its folder's
    access rules instead of the temporary directory's rules.
    """
    destination = Path(destination)
    staged = destination.with_name(f".{destination.name}.{uuid4().hex}.tmp")
    try:
        shutil.copyfile(source, staged)
        staged.replace(destination)
    finally:
        staged.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--example", choices=("all", "swap", "historical", "loans", "bonds", "interpolation", "portfolio"), default="all")
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
            publish_workbook(path, args.output_dir / filename)
        print(f"Created {args.output_dir / filename}")


EXAMPLES = [
    ("portfolio", "portfolio_aggregation_example.xlsx", create_portfolio),
    ("swap", "swap_pricing_example.xlsx", create_swap),
    ("historical", "historical_pricing_example.xlsx", create_historical),
    ("loans", "loan_pricing_example.xlsx", create_loans),
    ("bonds", "bond_pricing_example.xlsx", create_bonds),
    ("interpolation", "interpolation_example.xlsx", create_interpolation),
]


if __name__ == "__main__":
    main()

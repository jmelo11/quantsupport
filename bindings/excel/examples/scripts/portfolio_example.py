"""Guided, editable portfolio study matching the Python aggregation example."""

from workbook_layout import Study, context, formula, text


PORTFOLIO_CASES = [
    ("Deposit A", "Deposit", 100_000, .04, 0, "SOFR", "Receive"),
    ("Deposit B", "Deposit", 300_000, .06, 0, "SOFR", "Receive"),
    ("SOFR A", "Swap", 100_000, .04, .001, "SOFR", "Receive"),
    ("SOFR B", "Swap", 300_000, .06, .003, "SOFR", "Receive"),
    ("Term SOFR", "Swap", 200_000, .055, .002, "TermSOFR3m", "Receive"),
    ("Short SOFR", "Swap", 50_000, .04, .001, "SOFR", "Pay"),
]


def create_portfolio(path):
    routes = [
        ("1 Positions", "Edit sizes, quotes, indices and directions."),
        ("2 Market", "Review both explicit projection curves and SOFR discounting."),
        ("3 Build", "Build trades, supply their handle range, and evaluate the portfolio."),
        ("4 Results", "Reconcile portfolio PV and inspect the expected weighted quotes."),
        ("5 Coupons", "Inspect retained types, dates, fixing indices and weighted quotes."),
        ("6 Cashflows", "Read signed interest and principal by payment date."),
        ("7 Risk", "Read sensitivities summed by the original risk-factor labels."),
    ]
    with Study(path, "Portfolio", "Portfolio aggregation · coupons, payments and risk",
               "Aggregate six USD positions: two deposits, SOFR and Term SOFR 3M swaps, and one short swap. Compatible coupons combine while their fixing curves remain identifiable.",
               routes, [
                   "Change Deposit B's rate or notional. Compare the weighted deposit rate on Results with its coupon on Coupons.",
                   "Change SOFR B's spread. Compare paid SOFR coupons with the distinct TermSOFR3m coupons on the same dates.",
                   "Change Short SOFR from Pay to Receive. Read the new PV, coupon directions and net interest payments.",
               ]) as s:
        positions = "1 Positions"
        s.sheet(positions, "Positions and common dates",
                "Blue cells are editable. Receive deposits advance principal and receive interest/redemption; Receive swaps receive fixed and pay floating. All amounts are USD; rates and spreads are annual decimals.",
                (23, 18, 21, 16, 16, 24, 15, 18), "#337AB7")
        s.headers(positions, 6, ["Common input", "Value", "Meaning", "", "", "", "", ""])
        for row, label, value, key, note in [
            (7, "Valuation date", "2026-02-24", "ValuationDate", "Illustrative market date; no historical fixings needed."),
            (8, "Accrual start", "2026-03-02", "Start", "All positions start after the market reference date."),
            (9, "Deposit maturity", "2026-09-02", "DepositMaturity", "Six-month deposit interest and principal repayment."),
            (10, "Swap maturity", "2028-03-02", "SwapMaturity", "Two annual fixed and floating coupon periods."),
            (11, "Currency", "USD", "Currency", "Every PV is in USD; no FX conversion is performed."),
        ]:
            s.input(positions, row, label, value, key, note)
        s.headers(positions, 16, ["Position", "Instrument", "Notional (USD)", "Fixed rate", "Spread", "Projection / discount index", "Side", "Maturity"])
        for index, case in enumerate(PORTFOLIO_CASES, 1):
            row = 16 + index
            name, kind, notional, rate, spread, market_index, side = case
            for column, value, style, suffix in [
                ("A", name, "body", "Name"), ("B", kind, "body", "Kind"),
                ("C", notional, "input_number", "Notional"), ("D", rate, "input_rate", "Rate"),
                ("E", spread if kind == "Swap" else None, "input_rate" if kind == "Swap" else "body", "Spread"),
                ("F", market_index, "input", "Index"), ("G", side, "input", "Side"),
                ("H", "=" + s.name("DepositMaturity" if kind == "Deposit" else "SwapMaturity"), "body", "Maturity"),
            ]:
                s.write(positions, f"{column}{row}", value, style, f"C{index:02}_{suffix}")
            s.sheets[positions].data_validation(f"G{row}", {"validate": "list", "source": ["Receive", "Pay"]})
            s.sheets[positions].data_validation(f"C{row}", {"validate": "decimal", "criteria": ">=", "value": 0})
        s.merge(positions, "A24:H25", "The first two SOFR swaps combine their paid floating coupons at a notional-weighted spread. TermSOFR3m stays separate. Pay and receive coupons also remain separate, even when the dates match.")
        s.headers(positions, 27, ["Requested measure", "", "Description", "", "", "", "", ""])
        for row, request, note in [(28, "Value", "Monetary PV in the declared USD pricing currency."),
                                   (29, "Sensitivities", "Raw derivatives per unit of each original curve input."),
                                   (30, "Cashflows", "Typed coupons, plus a separate interest/capital summary.")]:
            s.write(positions, f"A{row}", request)
            s.merge(positions, f"C{row}:H{row}", note)
        s.range("Requests", positions, 28, 0, 30, 0)

        market = "2 Market"
        s.sheet(market, "Explicit zero curves",
                "Illustrative continuous zero rates on Actual360. SOFR projects SOFR coupons and discounts every USD payment. TermSOFR3m projects only that floating index; no bootstrap or FX inputs are needed.",
                (34, 40, 58), "#337AB7")
        s.headers(market, 6, ["Pillar", "SOFR zero rate", "TermSOFR3m zero rate"])
        for row, pillar in enumerate(["0D", "1Y", "3Y"], 7):
            s.write(market, f"A{row}", pillar)
            s.write(market, f"B{row}", .03, "input_rate")
            s.write(market, f"C{row}", .035, "input_rate")
        s.range("Pillars", market, 7, 0, 9, 0)
        s.range("SofrRates", market, 7, 1, 9, 1)
        s.range("TermRates", market, 7, 2, 9, 2)
        s.headers(market, 12, ["Construction step", "Object reference", "Purpose"])
        row = s.step(market, 13, "1. Create dated context", formula("QS.CONTEXT.CREATE", '"Portfolio.Root"', s.name("ValuationDate"), s.name("Currency"), '"SOFR"'), "Root", "The context owns its market stores.")
        first_input = row
        for index, source, key in [("SOFR", "SofrRates", "SofrCurves"), ("TermSOFR3m", "TermRates", "Curves")]:
            row = s.step(market, row, "Add " + index,
                         formula("QS.CONTEXT.ADD.ZERO.CURVE", s.name("Root"), text(index), s.name("Pillars"), s.name(source), '"Actual360"', '"Continuous"', '"Annual"', '"Linear"', "TRUE"), key, "Independent context curve addition.")
        s.range("MarketInputs", market, first_input, 1, row - 1, 1)

        build = "3 Build"
        s.sheet(build, "Build trades and aggregate a handle range",
                "Read top to bottom. Individual evaluations provide an independent PV check. The portfolio takes a contiguous range of trade handles and one broadcast pricer; it returns ordinary EvaluationResults.")
        s.headers(build, 6, ["Step", "Object reference", "Purpose"])
        row = s.step(build, 7, "1. Create pricing context",
                     context(s.name('Root')),
                     "Context", "Both explicit curves share the same valuation date.")
        row = s.step(build, row, "2. Select SOFR discounting",
                     formula("QS.DISCOUNT.POLICY.CSA", '"Portfolio.Policy"', s.name("Currency"), '"SOFR"'),
                     "Policy", "The same USD/SOFR policy prices deposits and swaps.")
        row = s.step(build, row, "3. Create shared pricer",
                     formula("QS.PRICER.CREATE", '"Portfolio.Pricer"', '"DiscountedCashflowPricer"', s.name("Policy")),
                     "Pricer", "One pricer handle can be broadcast to every portfolio member.")
        for index, case in enumerate(PORTFOLIO_CASES, 1):
            key = f"C{index:02}_"
            common = [text(f"Portfolio.C{index:02}.Instrument"), s.name("Start"), s.name(key + "Maturity"), s.name("Currency")]
            if case[1] == "Deposit":
                name = "QS.INSTRUMENT.FIXED.RATE.DEPOSIT"
                args = [*common, s.name(key + "Index"), s.name(key + "Notional"), s.name(key + "Rate"), '"Actual360"', '"Simple"', '"Annual"']
            else:
                name = "QS.INSTRUMENT.SWAP"
                args = [*common, s.name(key + "Index"), s.name(key + "Notional"), s.name(key + "Rate"), s.name(key + "Spread"),
                        '"Actual360"', '"Simple"', '"Annual"', '"Annual"', '"Annual"']
            row = s.step(build, row, case[0] + " · contract", formula(name, *args), key + "Instrument", "Dates, principal and coupon terms are contract inputs.")
            row = s.step(build, row, case[0] + " · position",
                         formula("QS.TRADE.CREATE", text(f"Portfolio.C{index:02}.Trade"), s.name(key + "Instrument"), s.name("ValuationDate"), s.name(key + "Side")),
                         key + "Trade", "Position direction is separate from the instrument.")
            row = s.step(build, row, case[0] + " · reference result",
                         formula('QS.EVALUATE.RESULTS', s.name('Context'), s.name(key + 'Trade'), s.name('Requests'), s.name('Pricer')),
                         key + "Result", "Independent single-position result used to check the portfolio PV.")
        s.headers(build, row, ["Portfolio member", "Trade handle passed to CREATE", "Reference evaluation"])
        for index, case in enumerate(PORTFOLIO_CASES, 1):
            s.write(build, f"A{row+index}", case[0])
            s.write(build, f"B{row+index}", "=" + s.name(f"C{index:02}_Trade"), "handle")
            s.write(build, f"C{row+index}", "=" + s.name(f"C{index:02}_Result"), "handle")
        s.range("Members", build, row + 1, 1, row + len(PORTFOLIO_CASES), 1)
        row += len(PORTFOLIO_CASES) + 3
        row = s.step(build, row, "4. Create portfolio from list",
                     formula("QS.PORTFOLIO.CREATE", '"Portfolio.Book"', s.name("Members")), "Book",
                     "The ordered member range holds live trade handles. Repeating a handle repeats its position.")
        s.step(build, row, "5. Aggregate completed results",
               formula("QS.EVALUATE.PORTFOLIO", s.name("Context"), s.name("Book"), s.name("Pricer"), s.name("Requests"), s.name("Currency")),
               "Result", "Returns standard EvaluationResults; numerical risk is summed after each trade's AD calculation.")

        results = "4 Results"
        s.sheet(results, "Portfolio PV and weighted quote checks",
                "PV reconciles to six independent evaluations. Expected quotes use the default receive positions; if you change direction, inspect the resulting pay/receive groups on Coupons.", (38, 24, 70), "#2E7D5B")
        s.headers(results, 6, ["Measure", "Value", "Interpretation"])
        s.result(results, 7, "Portfolio PV (USD)", formula("QS.RESULT.VALUE", s.name("Result")), "PV", "Total monetary PV under USD/SOFR discounting.")
        s.result(results, 8, "Sum of individual PVs", "=SUM(B21:B26)", "SumPV", "Independent position values listed below.")
        s.result(results, 9, "PV difference", "=" + s.name("PV") + "-" + s.name("SumPV"), "PVDifference", "Should be zero within floating-point rounding.")
        s.result(results, 11, "Expected deposit coupon rate", "=SUMPRODUCT('1 Positions'!C17:C18,'1 Positions'!D17:D18)/SUM('1 Positions'!C17:C18)",
                 "DepositRate", "Default: 400,000 at 5.5%; find its September 2026 fixed coupon on Coupons.", "rate")
        s.result(results, 12, "Expected paid SOFR spread", "=SUMPRODUCT('1 Positions'!C19:C20,'1 Positions'!E19:E20)/SUM('1 Positions'!C19:C20)",
                 "SofrSpread", "Default: 25 bp on -400,000 floating principal. TermSOFR3m stays at 20 bp on -200,000.", "rate")
        s.merge(results, "A15:C17", "Coupons retains both fixed and floating types, contractual and fixing indices, accrual dates and conventions. The short swap remains a separate direction. Cashflows nets payments by date/currency into Interest, Capital and Other. These are payment-date totals, not an accrual allocation of NII.")
        s.headers(results, 20, ["Position", "PV (USD)", "Reference result"])
        for index, case in enumerate(PORTFOLIO_CASES, 1):
            s.write(results, f"A{20+index}", case[0])
            s.write(results, f"B{20+index}", formula("QS.RESULT.VALUE", s.name(f"C{index:02}_Result")), "money", f"C{index:02}_PV")
            s.write(results, f"C{20+index}", "=" + s.name(f"C{index:02}_Result"), "handle")

        coupons = "5 Coupons"
        ws = s.sheet(coupons, "Aggregated cashflows and coupon terms",
                     "Compatible periods use notional-weighted fixed rates or floating spreads. SOFR and TermSOFR3m, distinct conventions and opposing directions stay separate. Principal rows have blank coupon terms.",
                     (17, 25, 12, 19, 17, 17, 19, 19, 21, 17, 17, 17, 14, 18, 20, 18, 46, 36))
        s.write(coupons, "A7", formula("QS.RESULT.COUPONS", s.name("Result")))
        for column in (9, 10, 11):
            ws.set_column(column, column, 17, s.formats["rate"])
        for column in (3, 8):
            ws.set_column(column, column, 21, s.formats["money"])
        ws.freeze_panes(7, 2)
        cashflows = "6 Cashflows"
        ws = s.sheet(cashflows, "Payments by date: interest and capital",
                     "Signed, undiscounted USD payments. Deposit advances are negative capital; redemptions are positive capital. Interest includes fixed and floating coupons. No accrual-period NII allocation is performed.",
                     (18, 14, 22, 22, 20, 22))
        s.write(cashflows, "A7", formula("QS.RESULT.CASHFLOW.SUMMARY", s.name("Result")))
        ws.set_column(2, 5, 22, s.formats["money"])
        risk = "7 Risk"
        ws = s.sheet(risk, "Portfolio sensitivities by original risk factor",
                     "Raw derivatives per unit input, summed after individual trade differentiation. Multiply a decimal-rate derivative by 0.0001 for an approximate +1 bp PV change. Both SOFR and TermSOFR3m risk remain labelled.",
                     (65, 26, 35))
        s.write(risk, "A7", formula("QS.RESULT.SENSITIVITIES", s.name("Result")))
        ws.set_column(1, 1, 26, s.formats["money"])

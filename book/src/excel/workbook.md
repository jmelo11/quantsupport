# Working in Excel

The Excel add-in lets a workbook construct markets, define trades and request
valuations from the native library. Input cells hold quotes and contractual
terms. Worksheet functions combine them into market and trade objects, which
pricing functions consume to calculate values and sensitivities.

Excel's formula dependencies determine recalculation order. Object handles
connect the formulas to native objects owned by the add-in.

## Handles and dependencies

A constructor returns a revisioned handle such as
`QSObject:QuoteStore:usd_quotes:3`. The add-in, not the cell, owns the native
object. Handles are process-local and must be reconstructed from formulas after
Excel restarts. Use workbook-unique names and a single ordered mutation chain:

```text
A1 =QS.QUOTES.CREATE("usd_quotes","2026-01-02")
A2 =QS.QUOTES.SET(A1,"OIS_USD_SOFR_1Y",B2,,)
A3 =QS.QUOTES.SET(A2,"OIS_USD_SOFR_2Y",B3,,)
A4 =QS.CURVE.CONFIGS.CREATE("usd_curves")
A5 =QS.CURVE.CONFIGS.ADD(A4,"SOFR","Actual360","LogLinear",TRUE,E2:E10)
A6 =QS.CONTEXT.CREATE("usd_ctx",A3,A5,,,,,,,,,,"USD","SOFR")
A7 =QS.PRICE(D2,A6)
A8 =QS.CURVE.GET(A6,"SOFR")
A9 =QS.CURVE.TABLE(A8)
```

To use this fragment, populate `E2:E10` with matching quote identifiers and `D2`
with a trade handle from a constructor such as `QS.TRADE.SWAP`. The [bundled
workbook](https://jmelo11.github.io/quantsupport/downloads/swap_pricing_example.xlsx)
supplies the complete inputs.

Unchanged recalculation reuses cached objects. A changed quote advances its
handle and invalidates dependent contexts and values; reads reject stale
revisions. Branching mutations from one old revision can make another branch
stale. Arrange the worksheet so each update consumes the preceding handle and the
final context depends on the completed input chain.

When rows are deleted or identifiers renamed, clear and repopulate a store
using `QS.OBJECT.RESET`. A version cell provides an explicit rebuild signal:

```text
A1 =QS.QUOTES.CREATE("usd_quotes",$B$1)
A2 =QS.OBJECT.RESET(A1,$Z$1)
A3 =QS.QUOTES.SET(A2,"OIS_USD_SOFR_1Y",B3,,)
```

## Financial interpretation

Dates are ISO strings. Rates and spreads are decimals, not percentage-point
inputs. A curve's day count and compounding convention do not replace the
contract's coupon conventions. An index such as `SOFR` identifies market
objects; it does not automatically turn a regular swap into a daily-compounded
OIS or select every required foreign-currency curve.

The output table's semantics matter as much as its layout. Generic DCF
cashflows show signed amounts, positive for receipts and negative for payments, and do not
supply a PV column. Quote sensitivities are derivatives in each input's units:
multiply a decimal-rate derivative by `0.0001` to estimate a +1 bp change.
Compare currencies only after an explicit, supported conversion.

Bond trades carry the valuation anchor. Supply `CleanPrice`, `DirtyPrice` or
`ZSpread` and its value to `QS.TRADE.FIXED.BOND`; analytics such as
`QS.BOND.PRICE`, `QS.BOND.MODIFIED.DURATION` and `QS.BOND.Z.SPREAD` then use the
same trade/context handles. A quote anchors the solved spread; it is not a
replacement for signed position PV.

## Scope and reproducibility

Worksheet functions cover market construction, contracts, pricing, analytics,
curve inspection and native historical snapshots. Registry operations
and pricing execute on Excel's main thread; registered scalar functions can be
thread-safe. Volatile registrations still reuse unchanged cached inputs.
There is no worksheet script constructor; the direct payoff-program interfaces
described later belong to Rust and Python.

For historical analysis, serialize coherent construction inputs rather than save
handle strings. [Historical valuation](historical-pricing.md) explains timestamp
selection, quote levels and separately supplied fixings. `QS.FUNCTIONS`,
`QS.FUNCTIONS.DETAILS` and `QS.HELP` expose worksheet help. The optional [Excel
guide](https://jmelo11.github.io/quantsupport/api/excel/guide/index.html)
develops binding-specific workflows, while the [worksheet
reference](https://jmelo11.github.io/quantsupport/api/excel/functions/index.html)
provides signatures without changing the dependency model described here.

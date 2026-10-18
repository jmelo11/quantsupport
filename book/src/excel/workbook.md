# Working in Excel

The XLL turns the native library into a worksheet dependency graph. Some cells
hold observations or contractual terms, some construct market objects, and
others request valuation outputs. The distinction between an input and a
constructed market remains the same as in Rust and Python; Excel adds
recalculation and object-lifetime rules.

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

This is a worksheet fragment, not a complete market: `E2:E10` must contain
matching quote identifiers, and `D2` must contain a trade handle such as one
constructed by `QS.TRADE.SWAP`. The
[bundled workbook](https://jmelo11.github.io/quantsupport/downloads/swap_pricing_example.xlsx)
supplies the complete inputs.

Unchanged recalculation reuses cached objects. A changed quote advances its
handle and invalidates dependent contexts and values; reads reject stale
revisions. Branching mutations from one old revision can make another branch
stale. Worksheet layout should therefore show financial dependency order, not
hide state changes inside unrelated formula chains.

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
cashflows show underlying amounts before leg-side multiplication and do not
supply a PV column. Quote sensitivities are derivatives in each input's units:
multiply a decimal-rate derivative by `0.0001` to estimate a +1 bp change.
Compare currencies only after an explicit, supported conversion.

Bond trades carry the valuation anchor. Supply `CleanPrice`, `DirtyPrice` or
`ZSpread` and its value to `QS.TRADE.FIXED.BOND`; analytics such as
`QS.BOND.PRICE`, `QS.BOND.MODIFIED.DURATION` and `QS.BOND.Z.SPREAD` then use the
same trade/context handles. A quote anchors the solved spread; it is not a
replacement for signed position PV.

## Scope and reproducibility

The worksheet surface includes market construction, contracts, pricing,
analytics, curve inspection and native historical snapshots. It is not an
automatic export of every Rust trait or Python facility. Registry operations
and pricing execute on Excel's main thread; registered scalar functions can be
thread-safe. Volatile registrations still reuse unchanged cached inputs.
There is no worksheet script constructor; the direct payoff-program interfaces
described later belong to Rust and Python.

For historical analysis, serialize coherent construction inputs rather than
save handle strings. [Historical valuation](historical-pricing.md) explains
timestamp selection, quote levels and separately supplied fixings.
`QS.FUNCTIONS`, `QS.FUNCTIONS.DETAILS` and `QS.HELP` expose worksheet help.
The optional [Excel guide](https://jmelo11.github.io/quantsupport/api/excel/guide/index.html)
develops binding-specific workflows, while the
[worksheet reference](https://jmelo11.github.io/quantsupport/api/excel/functions/index.html)
provides signatures without changing the dependency model described here.

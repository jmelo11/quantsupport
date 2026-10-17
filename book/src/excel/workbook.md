# Workbook Workflow

Constructors return revisioned handles such as
`QSObject:QuoteStore:usd_quotes:3`. The underlying object is owned by the
add-in. Handles are process-local and are recreated from workbook formulas
after restarting Excel.

Use workbook-unique names and chain mutations in dependency order:

```text
A1 =QS.QUOTES.CREATE("usd_quotes","2026-01-02")
A2 =QS.QUOTES.SET(A1,"OIS_USD_SOFR_1Y",B2,,)
A3 =QS.QUOTES.SET(A2,"OIS_USD_SOFR_2Y",B3,,)
A4 =QS.CURVE.CONFIGS.CREATE("usd_curves")
A5 =QS.CURVE.CONFIGS.ADD(A4,"SOFR","Actual360","LogLinear",TRUE,E2:E10)
A6 =QS.CONTEXT.CREATE("usd_ctx",A3,A5,,,,,,,,,"USD","SOFR")
A7 =QS.PRICE(D2,A6)
A8 =QS.CURVE.GET(A6,"SOFR")
A9 =QS.CURVE.TABLE(A8)
```

`E2:E10` must name quotes in the quote store, and `D2` must hold a trade
handle, for example from `QS.TRADE.SWAP`. The bundled workbook provides a
complete set of matching inputs.

Unchanged recalculation reuses the current objects. Changed quotes advance
their handles, which invalidates dependent contexts and prices. Reads reject
stale revisions. Branching two mutation chains from the same store can leave
one branch stale; keep updates in a single chain.

## Reset Stores

To remove old rows or rename identifiers, put `QS.OBJECT.RESET` between the
constructor and mutation chain. Change a configuration version cell when
the store must be cleared and repopulated:

```text
A1 =QS.QUOTES.CREATE("usd_quotes",$B$1)
A2 =QS.OBJECT.RESET(A1,$Z$1)
A3 =QS.QUOTES.SET(A2,"OIS_USD_SOFR_1Y",B3,,)
```

Dates are ISO strings (`YYYY-MM-DD`), rates and spreads are decimal fractions,
and indices use names such as `SOFR` or `EURIBOR3M`. Optional worksheet
arguments can be blank. See each function's parameter table for accepted values.

## Bond Valuation

Pass `CleanPrice`, `DirtyPrice`, or `ZSpread` and its numeric value when
constructing the trade with `QS.TRADE.FIXED.BOND`. Then use `QS.BOND.PRICE`,
`QS.BOND.MODIFIED.DURATION`, `QS.BOND.Z.SPREAD`, or `QS.BOND.SENSITIVITIES`
with the trade and context handles.
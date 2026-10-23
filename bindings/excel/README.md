# QuantSupport Excel add-in

The add-in connects worksheet arrays to the Rust library's contexts, instruments,
trades, policies, pricers and evaluation results. It requires **64-bit Windows
desktop Excel**. Excel for macOS and the browser cannot load the XLL.

## Install and build

Extract the complete [Excel ZIP](https://jmelo11.github.io/quantsupport/downloads/quantsupport-excel-windows-x64.zip)
into a stable folder. In **File > Options > Add-ins > Excel Add-ins > Go > Browse**,
load `quantsupport.xll`, `QuantSupport.Ribbon64.xll` and
`ExcelDna.IntelliSense64.xll` from that folder. Keep the generated
`quantsupport.intellisense.xml` alongside them. The companions require .NET
Framework 4.6.2 or later. Restart Excel after installation.

To build on Windows, install stable Rust for 64-bit MSVC, Visual Studio Build
Tools and .NET SDK 8 or later, then run:

```powershell
rustup target add x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File bindings/excel/scripts/build.ps1
```

The distribution is written to `bindings/excel/dist`. Use workbooks and XLLs
from the same build. Load the add-in before opening a workbook; use
**Ctrl+Alt+F9** after replacing an older build.

## One context workflow

`QS.CONTEXT.CREATE` owns the quote, fixing and FX stores and construction
settings. Each `QS.CONTEXT.ADD.*` formula defines one independent contribution.
Every contribution references the same root cell. Pass the complete contribution
array to evaluation, or compose it once with `QS.CONTEXT.PREPARE`.

```text
B2 =QS.CONTEXT.CREATE("usd","2026-01-15","USD","SOFR")
B3 =QS.CONTEXT.ADD.QUOTE(B2,"FixedRateDeposit_USD_SOFR_6M",0.04,"Mid")
B4 =QS.CONTEXT.ADD.CURVE(B2,"SOFR")
B5 =QS.CONTEXT.PREPARE("usd.market",B2,B3:B4)
D2 =QS.INSTRUMENT.FIXED.RATE.DEPOSIT("deposit","2026-01-15","2026-07-15","USD",,1000,0.05,"Actual360","Simple","Annual")
D3 =QS.TRADE.CREATE("position",D2,"2026-01-15","Receive")
D4 =QS.EVALUATE(B2,D3,"Value",B3:B4)
D5 =QS.EVALUATE(B5,D3,"Cashflows")
D6 =QS.EVALUATE.RESULTS(B5,D3,{"Value","Cashflows","Sensitivities"})
D7 =QS.RESULT.VALUE(D6)
```

Contributions are declarative worksheet values. They do not mutate the root
behind Excel's dependency graph. Including the inputs in `EVALUATE` or `PREPARE`
is essential: Excel then orders calculation, propagates edits and removes deleted
observations. Duplicate quote sides, fixing observations or curve indices fail
explicitly; separate Mid/Bid/Ask additions for one identifier can be combined.
Inputs from another root are rejected.

Use separate arrays for fields, in any row or column orientation:

```text
=QS.CONTEXT.ADD.QUOTES(B2,A10:A20,TRANSPOSE(F10:F20),G10:G20,H10:H20)
=QS.CONTEXT.ADD.FIXINGS(B2,A30:A40,F30:F40,TRANSPOSE(J30:J40))
=QS.CONTEXT.ADD.FX(B2,A50:A55,F50:F55,J50:J55)
=QS.CONTEXT.ADD.CURVE(B2,"SOFR",A10:A20,F10:F20)
```

Arrays must have equal entry counts. Interior blanks retain their positions;
omitted quote sides stay absent. Fields may be nonadjacent. No rectangular
input table, Excel Table object or header row is required.

The context has additions for bootstrapped, zero-rate and composite curves,
credit curves, volatility surfaces/cubes and native model/pricer configurations.
`QS.CONTEXT.QUOTE` and `QS.CONTEXT.FIXING` inspect a prepared context.

## Evaluate consistently

| Function | Result |
| --- | --- |
| `QS.EVALUATE` | One requested scalar or cashflow/sensitivity spill |
| `QS.EVALUATE.RANGE` | One scalar per instrument/trade, in input order |
| `QS.EVALUATE.RESULTS` | One reusable native result handle for request arrays |
| `QS.EVALUATE.PORTFOLIO` | Aggregated native result handle from a portfolio |
| `QS.EVALUATE.FROM.SNAPSHOT` | Historical scalar batch at aligned timestamps |

Direct evaluation takes **context, instrument/trade, request, inputs, pricer,
policy**, then output-layout flags. Request defaults to Value. A blank pricer
selects the native product pricer. A blank policy selects the context's base
curve policy. To choose funding or model parameters explicitly, construct a
policy and pricer once and pass the pricer. Its policy is already configured;
passing both an explicit pricer and a separate policy is an error.

```text
=QS.DISCOUNT.POLICY.CSA("csa","USD","SOFR")
=QS.DISCOUNT.POLICY.FIXED.INCOME("funding","EUR","FUNDING")
=QS.PRICER.CREATE("pricer","DiscountedCashflowPricer",policy)
=QS.EVALUATE(context,trade,"FairRate(0)",inputs,pricer)
=QS.EVALUATE.RANGE(context,trades,"Value",inputs,pricer)
=QS.EVALUATE.RESULTS(context,trade,requests,inputs,pricer)
=QS.EVALUATE.PORTFOLIO(context,portfolio,pricer,requests,"USD")
```

Contracts passed directly to evaluation use Receive, their own notional and the
context date. Use `QS.TRADE.CREATE` for explicit side/date/quantity and required
bond anchors. Swaptions require an explicitly configured pricer. FairRate(index)
selects one zero-based swap leg or loan segment; use separate evaluations for
multiple targets. Sensitivities are raw derivatives per unit factor change;
for decimal rates, multiplying by `0.0001` estimates a +1 bp move.
Table results accept trailing `transpose` and `headers` flags (FALSE and TRUE).
List results accept only `transpose`.

## Snapshots and historical pricing

Prepare each complete dated context, then archive independent context and
offset-aware observation-timestamp arrays:

```text
=QS.SNAPSHOT.FROM.RANGE("history",contexts,timestamps)
=QS.EVALUATE.FROM.SNAPSHOT(history,trades,dates,pricer,"Value")
```

Each trade has one RFC 3339 timestamp, such as `2026-01-15T16:00:00Z`.
Defaults are Previous/Mid: the latest complete market at or before the request
is used. Exact and Closest are available; Closest may select a future market.
The selected quote reference date remains the valuation date. Missing Bid/Ask
never falls back to Mid. Snapshot archives preserve construction inputs, and
exclude runtime curves and fixing vintages. Supply a shared fixing contribution
or prepared context in the optional `fixings` argument.

The batch groups requests by matched snapshot and constructs each market once.
It returns scalar values in input order without storing per-row evaluation
objects. Evaluation completes synchronously during Excel calculation.

## Ribbon and panel

The ribbon groups **Market**, **Pricing**, **Workbook**, **Calculation** and
**Setup & help** actions. Browse contexts and curves, inspect a selected handle,
or open Pricing for an instrument or trade. The panel uses aligned labeled
controls, resizable columns, virtual table rows and consistent numeric formatting.
Ctrl+A/Ctrl+C and the table copy menu include headings.

Details show financial inputs and dates. Source navigation and Copy handle
remain available without displaying method traces or dependency dumps. Worksheet
errors appear with their source cell in the panel; successful recalculation or
clearing the source removes them. Explicit query errors preserve the library's
message. Browsing never triggers workbook calculation.

**Reset Memory & Rebuild** discards native objects and rebuilds all open workbooks,
including in Manual mode, while preserving the calculation-mode setting.

## Migration and examples

This API change deliberately removes overlapping constructors and aliases.
Migrate older workbooks before using this build:

| Previous API | Replacement |
| --- | --- |
| Quote/fixing/FX stores and curve configuration collections | Context creation and `QS.CONTEXT.ADD.*` |
| `QS.CONTEXT.FROM.COMPONENTS` | `QS.CONTEXT.CREATE`, additions and `QS.CONTEXT.PREPARE` |
| `QS.CONTEXT.PRICE`, `QS.CONTEXT.VALUES` | `QS.EVALUATE`, `QS.EVALUATE.RANGE` |
| Stored `QS.EVALUATE` with name/pricer/trade/context/requests | `QS.EVALUATE.RESULTS` with context/trade/requests/inputs/pricer |
| `QS.HISTORY.VALUES` | `QS.EVALUATE.FROM.SNAPSHOT` |
| `QS.SNAPSHOTS.*` | Singular `QS.SNAPSHOT.*`; archive construction is `.ARCHIVE` |
| Product trade constructors and direct pricing/risk shortcuts | `QS.INSTRUMENT.*`, `QS.TRADE.CREATE`, evaluation requests |
| Fixed bond/deposit and floating note names | `FIXED.RATE.BOND`, `FIXED.RATE.DEPOSIT`, `FLOATING.RATE.NOTE` |

[Seven editable studies](examples/README.md) cover contexts, swaps, history,
loans, bonds, portfolio aggregation and interpolation. Each opens on Start Here.
[The guide](../../docs/excel/guide/index.rst) explains the financial conventions;
the [generated reference](https://jmelo11.github.io/quantsupport/api/excel/functions/index.html)
lists exact native signatures.

```sh
python -m pip install -r bindings/excel/examples/requirements.txt
python bindings/excel/examples/create_examples.py
cargo test -p quantsupport-xll --lib --example function_catalog
python -m unittest discover -s bindings/excel/examples -p 'test_*.py'
```

Windows lifecycle tests require pywin32 and `QS_XLL_PATH`. Portable tests verify
market construction, revision/lifetime handling, historical batches and workbook
formulas. Live Excel loading and panel behavior require the Windows smoke and
lifecycle checks with the matching XLLs.

# QuantSupport Excel XLL

The QuantSupport add-in provides worksheet functions for market construction,
trade definition, valuation and risk in **64-bit Windows Excel**. Formulas
connect quotes and contractual inputs to native objects, allowing dependent
prices and sensitivities to recalculate when inputs change.

Excel for macOS and the browser cannot load the XLL.

## Guides, reference, and download

- [Self-contained Excel user guide](https://jmelo11.github.io/quantsupport/api/excel/guide/index.html)
- [Installation and complete pricing workbook](https://jmelo11.github.io/quantsupport/api/excel/guide/workbook.html)
- [Market assumptions and product workflows](https://jmelo11.github.io/quantsupport/api/excel/guide/markets-and-products.html)
- [Risk, scenarios, and historical markets](https://jmelo11.github.io/quantsupport/api/excel/guide/risk-and-history.html)
- [Exact worksheet reference](https://jmelo11.github.io/quantsupport/api/excel/functions/index.html)
- [Download Excel add-in and IntelliSense ZIP](https://jmelo11.github.io/quantsupport/downloads/quantsupport-excel-windows-x64.zip)
- [Download quantsupport.xll only](https://jmelo11.github.io/quantsupport/downloads/quantsupport.xll)

The guide explains the financial conventions alongside complete worksheet
calculations. The Book and Python studies provide further examples. Registration metadata supplies
function names, argument order, aliases, categories, and per-parameter help.

## Install the download

Extract **all files** from the ZIP into one stable local folder. It includes
`quantsupport.xll`, `quantsupport.intellisense.xml`,
`ExcelDna.IntelliSense64.xll`, `QuantSupport.Ribbon64.xll`, installation
instructions and license notices.
In Excel's **File > Options > Add-ins > Excel Add-ins > Go > Browse** dialog,
load all three XLLs, then restart Excel. Keep them and the XML in the same folder.
The ribbon and IntelliSense companions require .NET Framework 4.6.2 or later.

### QuantSupport ribbon and object browser

The **QuantSupport** tab provides **Pricing Contexts**, **Instruments & Trades**, **Price Selection**, **Curves**,
**All Objects**, **Inspect Selection**, and **Refresh Browser**, plus links to the
setup guide and function reference. The side panel lists live objects belonging
to its workbook, with search, type filters, current handles, source formulas and
dependencies. Select an object, choose a source, and click **Go to cell** to edit
its worksheet definition. **Copy handle** copies its current runtime handle.
All ribbon grids support **Ctrl+A** to select every row and **Ctrl+C** to copy
the selected values with column headings. Right-click a table for **Select all**,
**Copy values**, or **Copy entire table**; pasted values form a tabular range in Excel.

Select an instrument or trade to open **Pricing**, or select its handle cell and
click **Price Selection** in the ribbon. **Price selected** in the panel opens the
same controls. Choose an existing pricing context,
an explicit pricer and **PV**, **Cashflows**, **Fair rate** or **Sensitivities**, then click
**Run request**. Fair rates require a zero-based swap leg or loan rate segment;
the selected pricer determines which requests it supports. Results appear in
the panel without adding worksheet formulas or stored results. Changing an input
selection clears the previous result; requests run again only when you click.
**Sensitivities** displays Risk factor and Sensitivity columns as raw derivatives
per unit of each factor. For decimal-rate factors, multiply by `0.0001` to
estimate the value change from a +1 bp move. Displayed and copied numbers use
Excel's decimal separator.
For a native instrument, confirm the displayed preview trade date and Receive/Pay
side. The date initially follows the selected context's valuation date, and the
preview uses the instrument's own notional. Bond previews also require an explicit
CleanPrice, DirtyPrice or ZSpread anchor and value, entered with Excel's decimal
separator. Previews create no stored trade or result objects. Selecting an existing
trade uses its stored position terms and hides the preview inputs.
`BondPricer` leaves `FairRate` absent. For bonds, request `YieldToMaturity`,
`ModifiedDuration`, `ZSpread` or `OAS` through worksheet evaluation as supported
by the selected pricer and anchor; `QS.RESULT.YIELD` reads the yield result.

Select **Curves**, then a context, curve configuration or constructed curve/store.
Choose a curve to see its dates, discount factors, zero rates and forwards.
Rates use Simple/Annual conventions and the curve's day count; flat curves show
labelled annual samples. Switch to **Instruments & inputs** to see calibration
quote identifiers, types, market levels and pillar dates. Selecting a row shows
its instrument parameters and curve conventions. Explicit zero-rate inputs and
composite curve components are also shown. For a standalone configuration or
curve, select its pricing context to obtain constructed nodes or market inputs.

Contexts display their valuation date, historical snapshot timestamp in UTC,
quote side, base currency/index and constructed market components. Instruments
display identifiers and notionals; trades display identifiers, trade dates and
sides. These dates describe market/contract data, not object creation times.
Changed dependencies are marked **Recalculate**. The browser refreshes while
Excel is ready and never triggers workbook recalculation or keeps native objects alive.
Curve inspection and trade requests use current handles; changed inputs must be
recalculated before running a request.
Each Excel window gets its own panel, released when that window closes.

The ribbon is optional; formulas work with the native XLL alone. Load the ribbon
and native XLL from the same build and folder. This version browses and inspects
existing objects; setup continues through worksheet inputs and formulas, with
examples accessible through **Setup Guide**.

## Build and load

On Windows, install the stable 64-bit MSVC Rust toolchain, Visual Studio
Build Tools and the .NET 8 SDK or later, then run from the repository root:

```powershell
rustup target add x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File bindings/excel/scripts/build.ps1
```

The build creates `bindings/excel/dist/quantsupport-excel-windows-x64.zip`,
including the XLL, generated XML, packed ribbon and pinned Excel-DNA IntelliSense companion.
Packaging downloads the companion when it is not already cached and checks its
SHA-256 against `third-party/intellisense.json`.

Use the XLL and workbooks from the same checkout or release. The published
XLL and online guide may not include development-branch changes. When testing
branch changes, load the locally built XLL and the workbooks under
`bindings/excel/examples`.

Load `bindings/excel/dist/quantsupport.xll` through **File > Options > Add-ins >
Excel Add-ins > Go > Browse**. Follow your organization's trusted-add-in policy.
Load the add-in before the workbook, or press **Ctrl+Alt+F9** afterwards.

### Parameter hints while typing

Inline function signatures and active-parameter descriptions use
[Excel-DNA IntelliSense](https://github.com/Excel-DNA/IntelliSense/releases).
Set it up once in 64-bit Windows Excel:

1. Extract the [Excel ZIP](https://jmelo11.github.io/quantsupport/downloads/quantsupport-excel-windows-x64.zip)
   or build it with the script above. Keep all add-in files together.
   Versioned downloads are also available from
   [releases](https://github.com/jmelo11/quantsupport/releases).
2. Load the included `ExcelDna.IntelliSense64.xll` through Excel's Add-ins dialog,
   alongside `quantsupport.xll`.
3. Restart Excel with both add-ins enabled, then type `=QS.CONTEXT.PRICE` followed
   by an opening parenthesis to see the parameter list and help for the current argument.

The metadata is generated from QuantSupport's function catalog, covering every
function and alias. If you rename the XLL, give the XML the same base name.
The companion requires .NET Framework 4.6.2 or later. Its
[setup documentation](https://github.com/Excel-DNA/IntelliSense/wiki/Usage-Instructions)
explains the external metadata format.

Excel's Function Arguments dialog also shows parameter names and descriptions:
type a function name and press **Ctrl+A**, or click **fx**. **Ctrl+Shift+A** inserts
its argument names into the formula. These built-in shortcuts work without the
IntelliSense companion.

```text
=QS.VERSION()
=QS.FUNCTIONS()
=QS.FUNCTIONS.DETAILS()
=QS.HELP("QS.INSTRUMENT.SWAP")
```

The [swap workbook](https://jmelo11.github.io/quantsupport/downloads/swap_pricing_example.xlsx) and
[historical workbook](https://jmelo11.github.io/quantsupport/downloads/historical_pricing_example.xlsx) contain inputs
and formulas; Windows Excel calculates them after the XLL is loaded.
Additional studies cover [portfolio aggregation](examples/portfolio_aggregation_example.xlsx),
[loans](examples/loan_pricing_example.xlsx),
[bond analytics](examples/bond_pricing_example.xlsx), and
[interpolation and schedules](examples/interpolation_example.xlsx).
Each opens on **Start Here**, with linked, numbered tabs, editable blue inputs,
explained construction steps and green financial results. The loan and bond
workbooks have a case selector shared by their cashflow and risk tables.
See the [example walkthrough](examples/README.md) for the suggested reading order.

Table functions accept trailing `transpose` and `headers` flags, defaulting to
`FALSE,TRUE`. For example, `=QS.RESULT.CASHFLOWS(result,TRUE,FALSE)` exchanges
rows and columns and omits headers. `=QS.RESULT.SENSITIVITIES(result,FALSE,FALSE)`
keeps the usual layout without headers. Lists such as `QS.SCHEDULE` and
`QS.SIMULATION.PATH` accept only `transpose` because they have no headers.

## Context-first pricing

Create a root context, add independent inputs, then request a value directly.
All additions reference the **same root cell**. The pricing formula takes the
**complete input range**, so Excel orders recalculation and propagates edits.
There is no last-update handle to find.

With the market date in B1 and Identifier/Mid quotes in A3:B6:

```text
H2 =QS.CONTEXT.CREATE("usd.context",$B$1,"USD","SOFR")
H3 =QS.CONTEXT.ADD.CURVE(H2,"SOFR",A3:B6)
H5 =QS.INSTRUMENT.SWAP("swap",$B$1,"2031-02-24","USD","SOFR",10000000,0.04,0,"Actual360","Simple","Annual","Semiannual","Quarterly")
H6 =QS.CONTEXT.PRICE(H2,H5,"Value",H3)
H7 =QS.CONTEXT.PRICE(H2,H5,"FairRate(0)",H3)
H8 =QS.CONTEXT.PRICE(H2,H5,"Cashflows",H3)
```

A curve input includes its definition and quotes. Alternatively, keep a shared
quote table in one addition and let each curve select matching identifiers:

```text
H3 =QS.CONTEXT.ADD.QUOTES(H2,A3:B6)
H4 =QS.CONTEXT.ADD.CURVE(H2,"SOFR")
H6 =QS.CONTEXT.PRICE(H2,H5,"Value",H3:H4)
```

Other additions cover fixings, FX, credit curves, explicit zero-rate curves,
composite curves, volatility surfaces and cubes. Additions are worksheet-owned
market definitions; they do not mutate the root as a hidden side effect. Blank
input-range cells are ignored. Duplicate quotes/curves and inputs from another
root fail explicitly. Include every relevant addition in prices and snapshots.

`QS.CONTEXT.PRICE` returns the requested number or table, with no evaluation
object. `QS.CONTEXT.VALUES` prices a range of instruments against one prepared
market. Native product pricers use the context base discount curve by default;
supply an explicit pricer to choose funding or model settings. Contracts use a
Receive position at the context date and their contractual notional. Pass a trade
for another side/date, quantity override, or bond anchor. Swaptions require an
explicit configured pricer.

Loan options are discoverable through `QS.LOAN.SETTINGS` named arguments;
pass its result to the loan constructor's settings argument. `QS.LOAN.PRICER`
exposes discount, refinancing and fair-rate amortization parameters directly.
Legacy Setting/Value tables remain accepted.

For large origination studies, use a batch instead of 50,000 worksheet contexts:

```text
=QS.HISTORY.VALUES(archive,instruments,origination_timestamps,loan_pricer)
```

Each instrument has one offset-aware timestamp. The batch returns values in
input order and constructs each distinct matched market once. It defaults to
Previous/Mid and preserves the selected market's actual reference date; it does
not roll a Friday market to a weekend origination date. Fixings are supplied
separately and must represent the intended historical information set.
Separately named `QS.CONTEXT.FROM.SNAPSHOT` objects also share native calibrated
markets when archive revision, matched snapshot, quote side and fixing revision
match. Different names or requested instants alone do not rebuild curves.

Open [context_pricing_example.xlsx](examples/context_pricing_example.xlsx) for
context inputs, direct loan prices, cashflows, sensitivities and batch history.
See the [context-first guide](../../docs/excel/guide/context-first.rst).

**Migration:** the former 15-argument `QS.CONTEXT.CREATE` is now
`QS.CONTEXT.FROM.COMPONENTS`, with the same argument order. Existing custom
workbooks using that signature must rename the function or adopt the new
context-first flow. The bundled component-based studies use the renamed
constructor. The legacy evaluation/result APIs remain available for stored,
multi-measure evaluations.

Historical batching and market reuse are synchronous. RTD/background execution
has not been added: asynchronous return alone does not parallelize calibration
or make the registry and AD state safe to access from worker threads.

## Calculation and financial conventions

Quote identifiers and their typed fields accept case variants, including lookup
and replacement in native Rust quote stores. Original display identifiers remain
available.

Constructors return revisioned handles such as
`QSObject:QuoteStore:w7~usd.quotes:3`. Names are isolated per workbook; two open
workbooks can use the same names. Keep handles in formulas or cell references:
they identify runtime objects and cannot be saved as permanent data.

Each worksheet cell owns its outputs and references to its inputs. Unchanged
inputs reuse objects and results. Replacing or clearing formulas releases their
previous objects once no live cell references them. Closing a workbook releases
its entire registry, including when Excel stays open with no workbooks. A native
observer processes cell edits and checks completed workbook closures on Excel's
main thread. On opening a workbook, it marks saved QuantSupport formulas dirty
so runtime handles are rebuilt under Excel's current calculation mode. It never
calls Calculate. Keep Excel's
`Application.EnableEvents` enabled for cleanup when editing or clearing cells;
workbook-close cleanup also runs independently of that setting.

Store update formulas own their individual quote, fixing, FX, curve, or
configuration contribution. Changing its key replaces the old contribution;
a failed edit or deleted formula removes it. An overwritten observation imported
from JSON is restored. Two cells cannot define the same store entry. Structural
configuration errors fail before changing the collection; errors requiring market
levels are reported when the context is built. Correct the responsible input and
recalculate without creating another store.

Functions owning objects and running valuations are nonvolatile. Excel's normal
dependency graph determines when they run, including Manual calculation mode.
F9/Calculate explicitly runs calculations; Excel can also evaluate a newly entered
formula in Manual mode. Changing calculation mode does not cancel an evaluation
already in progress. Use `QS.OBJECT.COUNT()` to inspect the calling workbook's
live object count, or `QS.OBJECT.COUNT(TRUE)` for the whole Excel instance.
`QS.OBJECT.RESET(store, reset_key)` / `QS.STORE.RESET` remain
available for an explicit reset of an entire mutable collection.

For aggregation of an ordered trade range, create a portfolio and evaluate it:

```text
=QS.PORTFOLIO.CREATE("Book",A2:A1001)
=QS.PORTFOLIO.EVALUATE("Book.Results",portfolio,pricer,context,requests,"USD")
=QS.RESULT.VALUE(result)
=QS.RESULT.SENSITIVITIES(result)
=QS.RESULT.CASHFLOWS(result)
=QS.RESULT.COUPONS(result)
=QS.RESULT.CASHFLOW.SUMMARY(result)
```

Open the guided [portfolio workbook](examples/portfolio_aggregation_example.xlsx)
for editable deposits and swaps, weighted coupon quotes, distinct fixing
indices, PV reconciliation, sensitivities and interest/capital payments.

Here `portfolio` and `result` refer to the first and second formula's handle
cells. `requests` is a cell or range containing **Value**, **Sensitivities** and/or
**Cashflows**. `pricer` may be one handle for every member or a range with one
pricer per trade, allowing mixed instruments. Repeated trade handles count as
repeated positions. Trade and pricer edits invalidate dependent portfolio results.

The result is the same **EvaluationResults** type used by `QS.EVALUATE`.
Unsupported aggregate requests remain absent, and a measure omitted by any
member remains absent for the whole portfolio. Monetary PVs and raw derivatives
must use one common pricing currency, explicitly identified by the final argument;
this label performs no FX conversion. Use compatible policies or separate
currency portfolios. The currency argument can be blank for cashflow-only runs.

Each member's completed numeric results are accumulated immediately. Identical
risk-factor labels are summed with compensated arithmetic, and built-in pricers
reuse their per-trade AD tape. The portfolio retains no combined AD graph or
per-trade result registry objects. Cashflows retain their original currencies,
coupon types, fixing indices, accrual start/end dates, day count, rate conventions,
overnight observation rules and payoffs. Only compatible coupons are grouped;
fixed rates and linear floating spreads use notional-weighted averages.
Pay and receive directions remain separate, avoiding undefined averages at zero
net notional. Daily-compounded spreads and different option payoffs are kept
distinct. Coupons lacking contractual metadata remain individual rows.
`QS.RESULT.COUPONS` displays the signed notional, weighted quote, contractual
coupon index, effective fixing index (including leg overrides) and conventions.
These terms are also shown in ribbon cashflow tables. In Rust,
`CouponDetails::to_coupon` reconstructs a numeric coupon; retain `forward_index`
on its containing leg when an override is present.
The summary reader shows **Payment date, Currency, Interest, Capital, Other, Total**.
Interest includes coupons; capital includes redemptions and signed advances;
unclassified payments stay in Other. Historical payments remain included.
The summary is a separate view; the detailed coupon information remains in the
standard results. These are signed undiscounted cash payments; accrual-period NII
is a separate step. PV and sensitivities continue to be computed from the original
trades, including any model-dependent schedules or prepayments.

Fixed-rate deposits are available as `QS.INSTRUMENT.FIXED.DEPOSIT`, followed by
`QS.TRADE.CREATE`, or directly as `QS.TRADE.FIXED.DEPOSIT`. Evaluate deposit positions
with `DiscountedCashflowPricer` and an explicit fixed-income discount policy.

For portfolios needing only individual PVs, return numbers directly:

```text
=QS.EVALUATE.VALUE(pricer,trade,context)
=QS.EVALUATE.VALUES(pricer,A2:A10001,context)
=QS.EVALUATE.VALUES(pricer,A2:A10001,context,TRUE)
```

The range version returns one PV per trade, as a column or transposed row. Excel
caches the values; no `EvaluationResults` objects are retained. Share market
contexts and pricers across positions. Use `QS.EVALUATE` and `QS.RESULT.*` when
cashflows, risk, fair rates or other stored measures are needed.

For large market histories, import each quote range and fixing history with one
formula instead of one `SET` formula per observation:

```text
=QS.QUOTES.FROM.RANGE("usd.quotes",B1,A3:D1002)
=QS.FIXINGS.FROM.RANGE("usd.fixings",F3:H10002)
```

Quote components can also come from separate arrays or nonadjacent ranges:

```text
=QS.QUOTES.FROM.RANGE("usd.quotes",B1,A3:A1002,H3:H1002)
=QS.QUOTES.FROM.RANGE("usd.quotes",B1,A3#,H3#,I3#,J3#)
```

The arguments after the date are **Identifiers, Mid, Bid, Ask**. Each component
can be a single cell, row array or column array, with the same number of entries
in each supplied array. Bid and Ask are optional; blanks preserve absent sides
without shifting subsequent entries. Omit Mid only when supplying another side
or using the combined-table form. The combined table remains supported when all
three side arguments are omitted.

Quote columns are **Identifier, Mid**, optionally followed by **Bid, Ask**.
Fixing columns are **Market index, Date, Value**, with dates as ISO text.
Matching headers and fully blank rows are optional. Missing quote sides remain
absent; duplicate identifiers or index/date fixings fail explicitly. Changing or
removing a row rebuilds the complete store, and unchanged inputs reuse its
handle. Share the resulting fixing handle across dated contexts: unchanged
histories share storage, while edits copy shared history before changing it.

Run `cargo run -p quantsupport-xll --release --example benchmark_history` to
measure attaching 10,000 shared fixings to 1,000 dated contexts. This benchmark
excludes Excel callbacks, curve construction and calibration.
For the 50,000-context calibration-reuse microbenchmark, run
`cargo test -p quantsupport-xll --release --lib benchmark_fifty_thousand_contexts -- --ignored --nocapture`.
It compares repeated construction of one simple deposit market with separately
named contexts sharing that market; it excludes instrument valuation and Excel.

The ribbon checks a cheap native change token before refreshing its object
list. Unchanged registries do not generate snapshots, and selecting an object
does not run pricing or curve queries. Use **Run request** or **Refresh table**
when you want those results.

Use **QuantSupport → Reset Memory & Rebuild** to discard all native objects and
AD tape storage in this Excel instance and reconstruct the open workbooks.
This explicitly calculates all open workbooks, including in Manual mode, and
keeps your calculation-mode setting. Existing handles become invalid; rebuilt
formulas create fresh handles. Save formulas and inputs normally; the button
does not change them.

`QS.INSTRUMENT.*` creates native contracts for every exposed product family.
`QS.TRADE.CREATE` adds position terms; `QS.PRICER.CREATE` selects an actual Rust
pricer and an explicit native discount policy. `QS.EVALUATE` returns an
`EvaluationResults` handle, read by `QS.RESULT.*`. Bond pricing anchors belong
to trades; loan refinancing and fair-rate amortization settings belong to pricers.
Existing `QS.TRADE.<product>` and direct valuation shortcuts remain compatibility
helpers; detailed component-based studies retain the explicit object workflow.
The context-first API also accepts contracts directly and selects a native
product pricer using the context base discount curve by default.

`QS.RESULT.VALUE` is monetary PV under the pricer's conventions. Swap cashflow
spills contain signed undiscounted amounts. `FairRate(index)` selects a zero-based
swap leg or loan rate segment and populates **one scalar `fair_rate` result**.
Swap target 0 solves the fixed coupon; target 1 solves the floating spread,
including all legs and principal in total PV. One evaluation accepts one fair-rate
target; use separate evaluations for several targets. Single-quote products accept
only target 0. Table outputs accept `transpose` and `headers`.
Sensitivity values are raw derivatives, not automatically DV01. Scale decimal
rate derivatives by `0.0001` for an approximate +1 bp move and retain the risk
factor's actual units. Futures contract size is currency per price point, not
per basis point. Equity indices use `Equity:SPX`; FX spot rates are quote
currency per base unit. Product-specific FX carry conventions follow the selected native Rust pricer.

Bond anchors are required trade inputs: exact `CleanPrice`, `DirtyPrice`, or
`ZSpread` plus a value. Clean/dirty quotes use bond quotation units, spreads
annual decimals, and `QS.RESULT.VALUE` returns monetary PV. Although registered,
`QS.BOND.OAS` currently cannot successfully evaluate a bond from the exposed
straight-bond constructors.

JSON import functions take native **JSON text**, not filenames. Snapshots
preserve construction configurations but exclude fixing vintages and runtime
curve objects. Previous/Mid are selector defaults; missing Bid/Ask does not
fall back; the selected market reference date remains the valuation date.
Archive export is subject to Excel's 32767 UTF-16-unit cell limit.

`QS.INSTRUMENT.LOAN` creates fixed or floating contracts with French, bullet,
or balloon principal. `rate` is the fixed coupon when `market_index` is blank,
otherwise the floating spread. Optional worksheet ranges use Setting/Value,
Periods/Rate or spread/Market index, and Date/CPR columns. Enter numeric, text,
and TRUE/FALSE cells directly; unknown or duplicate settings fail explicitly.
Create the position with `QS.TRADE.CREATE`, then choose `LoanPricer` and a
`QS.DISCOUNT.POLICY.FIXED.INCOME("policy","EUR","ESTR")` or the same call
with `"FUNDING"`. Multiple currencies use `.FROM.MAPPINGS(name,range,prefer)`.
The earlier `(name,mappings,prefer)` formula remains compatible.
Funding is an ordinary curve constructed through the same curve APIs as other
indices. It is selected by policy, never by a loan contract setting.

A loan's `FairRate(index)` solves earlier segments successively, repaying remaining
capital at each prefix end and locking its solved coupon/spread before extending.
The requested segment is returned in `fair_rate`; bounded coupons remain unsupported.
See the [loan guide](../../docs/excel/guide/loans.rst) and editable workbook for
credit, coupon schedules, prepayment and pricer settings. Credit configurations
use `QS.CREDIT.CURVE.CONFIGS.CREATE` and `.ADD` with CDS identifier ranges.

Supported workflows include curves/credit, volatility, model/simulation
configuration, vanilla rates, straight fixed income, FX/equity, CDS, options,
loans, scenarios, and interpolation. There is no worksheet callable/scripted
product constructor, custom-pricer registration, or netting/exposure/XVA
workflow. OIS quote bootstrapping does not expose a standalone OIS trade.

## Examples, development, and validation

Regenerate workbook formulas on any platform:

```sh
python -m pip install -r bindings/excel/examples/requirements.txt
python bindings/excel/examples/create_examples.py
```

`--example swap`, `historical`, `loans`, `bonds`, or `interpolation` selects a workbook; `--output-dir`
selects its destination. Optional Windows live validation uses `pywin32` and
`--xll-path bindings/excel/dist/quantsupport.xll`.

Implementations follow `src/functions/{time,market,instruments,pricing,risk}`,
with `math` and `system` utilities. `#[xll_bindgen]` attributes are the
registration source; `src/documentation/function_help.rs` supplies authoritative
argument/result/error prose. Add domain-specific help for a new argument,
result/error documentation for each function, and a navigation classification.
Registry/AD operations stay on the main calculation thread; scalar or spilled
pure functions may be thread-safe.

The registry owns native values under `src/runtime`. Add a `QsObject` variant
and native pricer dispatch when implementing another stored type. Keep Excel
callback pointers at the boundary, not in stored objects.

```sh
cargo test -p quantsupport-xll --lib --example function_catalog
python -m unittest discover -s bindings/excel/examples -p 'test_*.py'
```

For lifecycle checks in actual Windows Excel, install `pywin32`, set
`QS_XLL_PATH` to the built XLL, and run:

```powershell
python -m unittest discover -s bindings/excel/examples -p 'test_lifecycle_live.py' -v
```

These checks cover workbook isolation, closing the last workbook while Excel
stays open, failed edits, clearing/renaming formulas, and Manual versus explicit
calculation. They use their own Excel instance and close it afterward.

Portable tests cover the registry, historical markets, interpolation and
source-derived catalog. Windows tests additionally compare registered inventory
and exercise worksheet boundaries; portable tests do not prove native Excel
loading. The [Sphinx build guide](../../docs/README.md) requires neither Windows
nor Excel and preserves exact per-function reference URLs.

The build derives an expected function inventory from the worksheet sources.
Loading checks that inventory, registers every primary name and alias, and
requires a numeric Excel registration ID. IDs are opaque values; zero and
negative IDs are accepted. A rejection rolls back the
registrations and displays the failed function's name instead of loading a
partial add-in. Argument-name separators are compacted to fit Excel's 255-character
registration field; an oversized argument list fails the build.

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
- [Download quantsupport.xll](https://jmelo11.github.io/quantsupport/downloads/quantsupport.xll)

The guide explains the financial conventions alongside complete worksheet
calculations. The Book and Python studies provide further examples. Registration metadata supplies
function names, argument order, aliases, categories, and per-parameter help.

## Build and load

On Windows, install the stable 64-bit MSVC Rust toolchain and Visual Studio
Build Tools, then run from the repository root:

```powershell
rustup target add x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File bindings/excel/scripts/build.ps1
```

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

1. Build QuantSupport with the script above. Keep the generated
   `quantsupport.xll` and `quantsupport.intellisense.xml` together in the same folder.
   For release downloads, take both files from the same
   [release](https://github.com/jmelo11/quantsupport/releases).
2. Download `ExcelDna.IntelliSense64.xll` from the linked Excel-DNA release and
   load it through Excel's Add-ins dialog, alongside `quantsupport.xll`.
3. Restart Excel with both add-ins enabled, then type `=QS.EVALUATE` followed
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
Additional studies cover [loans](examples/loan_pricing_example.xlsx),
[bond analytics](examples/bond_pricing_example.xlsx), and
[interpolation and schedules](examples/interpolation_example.xlsx).

Table functions accept trailing `transpose` and `headers` flags, defaulting to
`FALSE,TRUE`. For example, `=QS.RESULT.CASHFLOWS(result,TRUE,FALSE)` exchanges
rows and columns and omits headers. `=QS.RESULT.SENSITIVITIES(result,FALSE,FALSE)`
keeps the usual layout without headers. Lists such as `QS.SCHEDULE` and
`QS.SIMULATION.PATH` accept only `transpose` because they have no headers.

## A visible market-to-price chain

Put ISO text `2026-02-24` in `B1`. Put native quote identifiers and annual decimal
rates in `A3:B6`, for example deposit
`FixedRateDeposit_USD_SOFR_1D` at `0.045`, followed by
`OIS_USD_SOFR_1Y`, `_2Y`, `_5Y` at `0.04837`, `0.04528`, `0.04077`.
Use full identifiers in each row, not literal shorthand `_2Y`.

```text
H2 =QS.QUOTES.CREATE("workbook.usd.quotes",$B$1)
C3 =QS.QUOTES.SET(H2,A3,B3,,)
C4 =QS.QUOTES.SET(C3,A4,B4,,)
C5 =QS.QUOTES.SET(C4,A5,B5,,)
C6 =QS.QUOTES.SET(C5,A6,B6,,)
H3 =QS.CURVE.CONFIGS.CREATE("workbook.usd.curves")
H4 =QS.CURVE.CONFIGS.ADD(H3,"SOFR","Actual360","LogLinear",FALSE,A3:A6)
H5 =QS.CONTEXT.CREATE("workbook.usd.context",C6,H4,,,,,,,,,,"USD","SOFR","Mid")
H6 =QS.INSTRUMENT.SWAP("workbook.usd.instrument",$B$1,"2031-02-24","USD","SOFR",10000000,0.04,0,"Actual360","Simple","Annual","Semiannual","Quarterly")
H7 =QS.TRADE.CREATE("workbook.usd.trade",H6,$B$1,"Receive")
H8 =QS.DISCOUNT.POLICY.CSA("workbook.usd.policy","USD","SOFR")
H9 =QS.PRICER.CREATE("workbook.usd.pricer","DiscountedCashflowPricer",H8)
K2 Value
K3 =QS.REQUEST.FAIR.RATE(1)
H10 =QS.EVALUATE("workbook.usd.results",H9,H7,H5,K2:K3)
H11 =QS.RESULT.VALUE(H10)
H12 =QS.RESULT.FAIR.RATE(H10)
```

`QS.CONTEXT.CREATE` takes base currency and base index in positions **13 and
14**, then optional quote level. Preserve the intervening empty component
positions. Dates are ISO text; 4% is numeric `0.04`; a quote-side blank remains
absent instead of copying Mid. See the guide for inspection spills and examples
that require no JSON.

## Calculation and financial conventions

Constructors return revisioned handles such as
`QSObject:QuoteStore:workbook.usd.quotes:3`. The objects remain in the XLL's
process/thread-local registry. Unchanged inputs reuse objects; changed inputs
advance revisions and recalculate dependents. Readers reject stale handles;
mutators accept older revisions of the same identity in dependency chains.
Use workbook-qualified object names when several workbooks may be open.

Deleting a mutation row does not remove its old observation automatically.
Use `QS.OBJECT.RESET(store, reset_key)` (alias `QS.STORE.RESET`) before the
update chain when the input set changes. Reset applies to supported mutable
stores/configuration collections, not immutable curves, contexts, or trades.
Workbooks must preserve formulas and inputs; handles do not serialize objects.

`QS.INSTRUMENT.*` creates native contracts for every exposed product family.
`QS.TRADE.CREATE` adds position terms; `QS.PRICER.CREATE` selects an actual Rust
pricer and an explicit native discount policy. `QS.EVALUATE` returns an
`EvaluationResults` handle, read by `QS.RESULT.*`. Bond pricing anchors belong
to trades; loan refinancing and fair-rate amortization settings belong to pricers.
Existing `QS.TRADE.<product>` and direct valuation shortcuts remain compatibility
helpers; new examples use the separated object workflow. Loans require the
explicit pricer path.

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
`QS.DISCOUNT.POLICY.FIXED.INCOME` mapping such as EUR/ESTR or EUR/FUNDING.
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

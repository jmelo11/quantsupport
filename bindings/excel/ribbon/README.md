# QuantSupport ribbon companion

This optional 64-bit Windows Excel add-in supplies the QuantSupport ribbon and
an object browser attached to each workbook window. It targets .NET Framework
4.6.2 and uses Excel-DNA 1.9.0. Build on Windows with the .NET 8 SDK:

```powershell
dotnet build bindings/excel/ribbon/QuantSupport.Ribbon.csproj -c Release
```

The packed output is `bin/Release/net462/publish/QuantSupport.Ribbon64.xll`.
The main Excel build script copies it beside `quantsupport.xll` and includes it
in the distribution ZIP. Both XLLs must be loaded from that folder.

The UI resolves the already-loaded native XLL by its full path. The versioned
C ABI v3 (`qs_browser_api_version`, `qs_browser_revision`, `qs_browser_snapshot`,
`qs_browser_query`, `qs_browser_reset`, `qs_browser_free`)
copies UTF-8 JSON on Excel's main thread. It never loads another copy of the
native DLL, calls worksheet constructors, or retains
native object pointers. Registry snapshots use no ownership/dependency reads.
`qs_browser_revision` returns a constant-time change token without serializing
or walking the registry. `qs_browser_reset` clears all workbook registries and
the AD tapes after checking that no worksheet call is active. Workbook session
IDs continue increasing so handles saved before a reset cannot become valid again.
Queries validate handles and their upstream revisions without recording worksheet
reads. They borrow the named workbook's existing registry without selecting it.
Pricing requests use the selected native pricer/context and return owned result
tables without registering EvaluationResults. Requests include PV, Cashflows,
Fair rate and Sensitivities. Sensitivities returns Risk factor/Sensitivity rows
with raw derivatives and localized numeric formatting; decimal-rate exposures
times `0.0001` estimate a +1 bp move. All requests run in queued macros,
with workbook/window checks and cancellation when the selection changes.
Sheet IDs and one-based source coordinates are resolved to Excel references
when navigating, with a fresh handle/source check in a queued macro.

Only numeric window identities and owned metadata persist in the UI; no
Workbook, Window or Range COM wrappers are cached. A Forms timer refreshes the
visible active pane when Excel is ready and the native change token changes,
and disposes panes for closed windows. The object grid uses virtual rows, and
only the visible explorer tab binds its context/pricer selectors.
Details is always available; Pricing appears for instruments and trades, and
Curves appears for objects with curves. **Price Selection** in the ribbon opens
Pricing for the active handle cell; **Price selected** is also available in the
panel. Instrument previews pass an explicit trade date and Receive/Pay side to
the native contract-to-trade conversion, retaining the instrument notional.
The date starts at the selected context's valuation date. Bonds require an
explicit CleanPrice, DirtyPrice or ZSpread anchor and value. Preview trades and
results are temporary; stored trade selections use their original position terms.
Selection and the chosen tab follow the same object when
recalculation replaces its handle revision, including after quote corrections.
Normal browsing does not request workbook recalculation. Trade pricing and
curve queries are explicit; choose a view and click its request/refresh button.
**Reset Memory & Rebuild** is a queued macro: it clears native state, cancels
pending browser requests and explicitly invokes Excel's `CalculateFullRebuild`
for all open workbooks. The calculation mode is not changed. Results use read-only grids;
all grids allow multiple selected rows, Ctrl+A/Ctrl+C, and a right-click copy menu.
Column headers wrap and grow to fit their text. Numeric values and copied table
values use Excel's decimal separator, including a custom separator; identifiers,
dates, formulas and handles retain their original text.
Cashflow tables include coupon dates, contractual/fixing indices, signed notional
and rate/spread conventions. Selecting a calibration row displays its parameters.
Closing a workbook retires its native objects independently of other open workbooks.
Reopening it creates a fresh session, even before the next maintenance timer tick.
Setup links open worksheet guides; creation
wizards are deferred.

The managed companion can also be compiled and packed on macOS/Linux with the
.NET SDK. To compile without generating XLL files:

```sh
dotnet build bindings/excel/ribbon/QuantSupport.Ribbon.csproj -c Release \
  -p:RunExcelDnaBuild=false -p:RunExcelDnaPack=false -p:RunExcelDnaClean=false
```

Run `dotnet run --project bindings/excel/ribbon/tests/ContractTests.csproj` for
the managed JSON contract. Rust verifies the same fixture against an actual
registry snapshot, including Unicode names, date semantics and formula quoting.
On Windows, after building Release, run
`powershell -NoProfile -ExecutionPolicy Bypass -File bindings/excel/ribbon/tests/smoke.ps1`
to check pane construction, trade selection and request controls without opening Excel.
Run `cargo test -p quantsupport-xll --lib --example function_catalog` for native
metadata, workbook isolation and lifetime checks. Building the native engine
and validating the live ribbon require Windows Excel; the repository's Windows
bindings job builds both as part of the complete distribution.

## Panel layout

The ribbon groups Market, Pricing, Workbook, Calculation and Help. Panel forms
use two-column labels/controls, and result grids use virtual rows with fixed
initial widths, resizable columns and wrapped headings. Worksheet diagnostics
include source cells and native messages. Details omit formulas and dependency
traces; navigation and copying remain explicit actions.

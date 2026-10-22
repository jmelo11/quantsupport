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
C ABI v2 (`qs_browser_api_version`, `qs_browser_snapshot`, `qs_browser_query`, `qs_browser_free`)
copies UTF-8 JSON on Excel's main thread. It never loads another copy of the
native DLL, calls worksheet constructors, selects a caller registry, or retains
native object pointers. Registry snapshots use no ownership/dependency reads.
Queries validate handles and their upstream revisions without recording worksheet
reads. They borrow the named workbook's existing registry without selecting it.
Trade requests use the selected native pricer/context and return owned result
tables without registering EvaluationResults. All requests run in queued macros,
with workbook/window checks and cancellation when the selection changes.
Sheet IDs and one-based source coordinates are resolved to Excel references
when navigating, with a fresh handle/source check in a queued macro.

Only numeric window identities and owned metadata persist in the UI; no
Workbook, Window or Range COM wrappers are cached. A Forms timer refreshes the
visible active pane when Excel is ready and disposes panes for closed windows.
The browser does not request workbook recalculation. Trade pricing is explicit;
curve selection reads nodes and calibration inputs. Results use read-only grids;
all grids allow multiple selected rows, Ctrl+A/Ctrl+C, and a right-click copy menu.
Cashflow tables include coupon dates, contractual/fixing indices, signed notional
and rate/spread conventions. Selecting a calibration row displays its parameters.
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

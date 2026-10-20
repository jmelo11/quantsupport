# Download and install

**[Download the Excel add-in and IntelliSense ZIP for 64-bit Windows
Excel](https://jmelo11.github.io/quantsupport/downloads/quantsupport-excel-windows-x64.zip)**

The ZIP includes `quantsupport.xll`, its matching XML metadata,
`ExcelDna.IntelliSense64.xll`, installation instructions and license notices.
The [standalone XLL](https://jmelo11.github.io/quantsupport/downloads/quantsupport.xll)
remains available in the same download directory.

[Release history](https://github.com/jmelo11/quantsupport/releases) provides
versioned downloads. Excel for macOS and Excel in the browser cannot load an XLL.

1. Extract all files from the ZIP into a stable local folder.
2. In Excel open **File > Options > Add-ins**.
3. Select **Excel Add-ins**, click **Go**, then **Browse**, and load both XLLs.
   Keep `quantsupport.intellisense.xml` beside `quantsupport.xll`, then restart Excel.
4. Enter `=QS.VERSION()` to check that it loaded.
5. Open the [swap example workbook](https://jmelo11.github.io/quantsupport/downloads/swap_pricing_example.xlsx)
   and press **Ctrl+Alt+F9** to rebuild its object handles.

The workbook includes pricing, curve tables, cashflows, and sensitivities.

## Discover functions

`=QS.FUNCTIONS()` lists worksheet names. `=QS.FUNCTIONS.DETAILS()` spills the
catalog. `=QS.HELP("QS.INSTRUMENT.SWAP")` displays parameter guidance in cells. The
same help appears in Excel's Function Arguments dialog and the optional
[worksheet
reference](https://jmelo11.github.io/quantsupport/api/excel/functions/index.html).

For parameter hints **while typing**, load the ZIP's included
`ExcelDna.IntelliSense64.xll` alongside QuantSupport and keep the matching
`quantsupport.intellisense.xml` beside `quantsupport.xll`. Restart Excel with both
add-ins enabled, then type `=QS.EVALUATE` followed by an opening parenthesis to see its signature and active-argument
help. The companion requires .NET Framework 4.6.2 or later.

Without the companion, type the function name and press **Ctrl+A** for the
Function Arguments dialog, or **Ctrl+Shift+A** to insert its argument names.

Function names and aliases are public. Registry objects, raw pointers, and Rust
helper functions are internal. Scalar date functions can run on calculation
threads; registry and pricing functions run on Excel's main thread.

## Build from source

On Windows, install the 64-bit MSVC Rust toolchain and Visual Studio Build Tools,
then run from the repository root:

```powershell
rustup target add x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File bindings/excel/scripts/build.ps1
```

The build packages `bindings/excel/dist/quantsupport-excel-windows-x64.zip`
and the individual XLL, XML and companion files. It downloads a pinned
[Excel-DNA IntelliSense](https://github.com/Excel-DNA/IntelliSense/releases)
release and verifies its SHA-256. Keep the files together if moving the add-in.

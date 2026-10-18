# Download and install

**[Download quantsupport.xll for 64-bit Windows
Excel](https://jmelo11.github.io/quantsupport/downloads/quantsupport.xll)**

[Release history](https://github.com/jmelo11/quantsupport/releases) provides
versioned downloads. Excel for macOS and Excel in the browser cannot load an XLL.

1. Download `quantsupport.xll` and keep it in a stable local folder.
2. In Excel open **File > Options > Add-ins**.
3. Select **Excel Add-ins**, click **Go**, then **Browse**, and choose the XLL.
4. Enter `=QS.VERSION()` to check that it loaded.
5. Open the [swap example workbook](https://jmelo11.github.io/quantsupport/downloads/swap_pricing_example.xlsx)
   and press **Ctrl+Alt+F9** to rebuild its object handles.

The workbook includes pricing, curve tables, cashflows, and sensitivities.

## Discover functions

`=QS.FUNCTIONS()` lists worksheet names. `=QS.FUNCTIONS.DETAILS()` spills the
catalog. `=QS.HELP("QS.TRADE.SWAP")` displays parameter guidance in cells. The
same help appears in Excel's Function Arguments dialog and the optional
[worksheet
reference](https://jmelo11.github.io/quantsupport/api/excel/functions/index.html).

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

The packaged add-in is `bindings/excel/dist/quantsupport.xll`.

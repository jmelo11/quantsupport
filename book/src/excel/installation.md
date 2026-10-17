# Download and Install

**[Download quantsupport.xll for 64-bit Windows Excel](https://github.com/jmelo11/quantsupport/releases/latest/download/quantsupport.xll)**

The download is attached to the latest published GitHub release.
[Release history](https://github.com/jmelo11/quantsupport/releases) provides
versioned downloads. Excel for macOS and Excel in the browser cannot load an XLL.

1. Download `quantsupport.xll` and keep it in a stable local folder.
2. In Excel open **File > Options > Add-ins**.
3. Select **Excel Add-ins**, click **Go**, then **Browse**, and choose the XLL.
4. Enter `=QS.VERSION()` to check that it loaded.
5. Open the [swap example workbook](https://github.com/jmelo11/quantsupport/raw/refs/heads/main/bindings/excel/examples/swap_pricing_example.xlsx)
   and press **Ctrl+Alt+F9** to rebuild its object handles.

The workbook includes pricing, curve tables, cashflows, and sensitivities.

## Discover Functions

`=QS.FUNCTIONS()` lists worksheet names. `=QS.FUNCTIONS.DETAILS()` spills
the catalog. `=QS.HELP("QS.TRADE.SWAP")` displays parameter guidance in cells.
The same help appears in Excel's Function Arguments dialog and the
[worksheet reference](reference.md).

Function names and aliases are public. Registry objects, raw pointers, and Rust
helper functions are internal. Scalar date functions can run on calculation
threads; registry and pricing functions run on Excel's main thread.

## Build from Source

On Windows, install the 64-bit MSVC Rust toolchain and Visual Studio Build Tools,
then run from the repository root:

```powershell
rustup target add x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File bindings/excel/scripts/build.ps1
```

The packaged add-in is `bindings/excel/dist/quantsupport.xll`.

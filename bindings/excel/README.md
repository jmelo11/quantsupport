# QuantSupport Excel XLL

This crate exposes QuantSupport through a native XLL for 64-bit Excel on
Windows. The Excel layer stores and composes the library's real types:
`QuoteStore`, `FixingStore`, `FxStore`, configuration vectors,
`DiscountCurveElement`, `ConstructedElementStore`, `PricingContext`, native
trades, and calls to the native pricers. It does not define a parallel market
model.

## Build and load

From a Windows PowerShell prompt with the 64-bit MSVC Rust toolchain and Visual
Studio Build Tools installed:

```powershell
rustup target add x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File bindings/excel/scripts/build.ps1
```

The packaged add-in is written to
`bindings/excel/dist/quantsupport.xll`. In Excel, open **File > Options >
Add-ins**, choose **Excel Add-ins**, click **Go**, and browse to that file.

Use `=QS.FUNCTIONS()` to spill the authoritative function list exposed by the
build. The current modules cover:

- dates, schedules, and day-count fractions;
- `QuoteStore`, `FixingStore`, and `FxStore` creation, JSON loading, updates,
  and lookup;
- curve, credit-curve, volatility-surface, volatility-cube, simulation, and
  scenario configurations loaded with the library's own serde formats;
- flat `DiscountCurveElement` construction and `ConstructedElementStore`
  composition;
- initialized `PricingContext` construction and inspection;
- flat and bootstrapped curve queries, volatility queries, and simulation
  metadata/paths;
- swaps, basis swaps, both cross-currency swap forms, fixed-rate bonds,
  deposits, floating-rate notes, FX forwards, FX/equity options, CDSs,
  caps/floors, caplets/floorlets, and rate futures;
- native price, fair-rate, cashflow, and sensitivity requests.

## Object handles and quote updates

An Excel cell cannot contain a Rust object, so constructors return a revisioned
token such as:

```text
QSObject:QuoteStore:usd_quotes:3
```

The object itself remains an owned QuantSupport value in the XLL registry. A
mutating function replaces the library value and returns the next revision.
That changed token is what makes Excel recalculate the `PricingContext` and all
dependent prices.

For example:

```text
A1 =QS.QUOTES.CREATE("usd_quotes","2026-01-02")
A2 =QS.QUOTES.SET(A1,"OIS_USD_SOFR_1Y",B2,,)
A3 =QS.CURVE.CONFIGS.FROM.JSON("usd_curves",C2)
A4 =QS.CONTEXT.CREATE("usd_ctx",A2,A3,,,,,,,,,"USD","SOFR")
A5 =QS.PRICE(D2,A4)
```

When `B2` changes, `QS.QUOTES.SET` replaces that quote and returns a new token;
`QS.CONTEXT.CREATE` reruns `PricingContext::initialize`, so curves,
volatilities, simulations, and the price are rebuilt consistently. Multiple
updates must form a chain (`A1 -> A2 -> A3 ...`). Reads reject stale handles,
which prevents a price from silently using an older context.

Recalculating a mutator may supply its original upstream revision after another
formula advanced the object. Mutators therefore apply to the current object
identity and emit a fresh revision. Constructors use named upsert semantics,
so recalculation replaces the named object instead of leaking a new object.

Objects are process-local and cleared when the XLL unloads. Workbooks must keep
the formulas or JSON inputs needed to recreate them. Use workbook-qualified
names when several workbooks may be open in one Excel process.

## Flat-curve pricing example

This example builds the same native components shown in the Rust quick start:

```text
A1 =QS.QUOTES.CREATE("quotes","2026-01-02")
A2 =QS.CURVE.FLAT("sofr","SOFR","2026-01-02",0.03,"Actual360","Continuous","Annual")
A3 =QS.ELEMENTS.CREATE("elements")
A4 =QS.ELEMENTS.ADD.DISCOUNT.CURVE(A3,A2)
A5 =QS.CONTEXT.CREATE("ctx",A1,,,,,,,,A4,,"USD","SOFR")
A6 =QS.TRADE.SWAP("swap","2026-01-02","2026-01-02","2031-01-02","USD","SOFR",10000000,0.03,0,"Receive","Actual360","Simple","Semiannual","Semiannual","Quarterly")
A7 =QS.PRICE(A6,A5)
A8 =QS.FAIR.RATE(A6,A5)
A9 =QS.SENSITIVITIES(A6,A5)
A10=QS.CASHFLOWS(A6,A5)
```

Complex indices use explicit prefixes in worksheet arguments:
`Equity:SPX`, `Credit:ACME`, `FX:EUR/USD`, and `Collateral:EUR/USD`.

## JSON-backed production context

Configuration functions deserialize the same types and JSON structures used by
the Rust and Python APIs:

```text
=QS.QUOTES.FROM.JSON("quotes", quotes_json)
=QS.FIXINGS.FROM.JSON("fixings", fixings_json)
=QS.FX.FROM.JSON("fx", fx_records_json)
=QS.CURVE.CONFIGS.FROM.JSON("curves", curve_configs_json)
=QS.CREDIT.CURVE.CONFIGS.FROM.JSON("credit", credit_configs_json)
=QS.VOL.SURFACE.CONFIGS.FROM.JSON("surfaces", surface_configs_json)
=QS.VOL.CUBE.CONFIGS.FROM.JSON("cubes", cube_configs_json)
=QS.SIMULATION.CONFIGS.FROM.JSON("simulations", simulation_configs_json)
=QS.SCENARIOS.FROM.JSON("scenario", scenario_json)
```

Pass those handles to `QS.CONTEXT.CREATE`. Optional arguments can be omitted
with empty argument positions or supplied through blank cells.

## Add a worksheet function

Functions are split by library area under `src/functions`. A function registers
itself through `#[xll_bindgen]`; there is no central hand-maintained registration
table:

```rust,ignore
#[xll_bindgen(
    name = "QS.MY.FUNCTION",
    category = "QuantSupport - My Area",
    help = "Explains the worksheet result"
)]
pub fn qs_my_function(value: f64) -> f64 {
    quantsupport_logic(value)
}
```

Add the module to `src/functions/mod.rs`. Pure scalar functions may be marked
`threadsafe`; registry and automatic-differentiation functions must remain on
Excel's main calculation thread.

## Add another stored library type or pricer

1. Add the actual QuantSupport type as a `QsObject` variant in
   `src/registry.rs` and give it a stable type tag.
2. Add its constructor in the relevant `src/functions` module.
3. If it is a trade, add its native pricer branch to `functions/pricing.rs`.
4. Test its ordinary Rust construction/pricing logic and cross-check the
   Windows target.

Keep raw `XLOPER12` values at the Excel boundary. The registry must own Rust
values because Excel callback arguments are only valid for the duration of a
call.

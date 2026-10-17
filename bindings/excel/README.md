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

After loading the XLL, open
[`examples/swap_pricing_example.xlsx`](examples/swap_pricing_example.xlsx).
Its **Pricing** sheet shows the complete curve-to-context-to-swap chain, its
**Curve Results** sheet spills bootstrapped discount factors, zero rates, and
forward rates, and its **Outputs** sheet spills priced cashflows and
sensitivities. Press
**Ctrl+Alt+F9** once if the workbook was opened before the XLL was loaded.

Use `=QS.FUNCTIONS()` for the worksheet-function names,
`=QS.FUNCTIONS.DETAILS()` for the full catalog, or—for example—
`=QS.HELP("QS.TRADE.SWAP")` for an in-sheet parameter guide. Excel's
**Function Arguments** dialog also displays a hint for every parameter. The
current modules cover:

- dates, schedules, and day-count fractions;
- `QuoteStore`, `FixingStore`, and `FxStore` creation, JSON loading, updates,
  and lookup;
- curve, credit-curve, volatility-surface, volatility-cube, simulation, and
  scenario configurations loaded with the library's own serde formats;
- flat `DiscountCurveElement` construction and `ConstructedElementStore`
  composition;
- initialized `PricingContext` construction and inspection;
- flat and bootstrapped curve handles, curve-table/discount/forward queries,
  volatility queries, and simulation metadata/paths;
- swaps, basis swaps, both cross-currency swap forms, fixed-rate bonds,
  deposits, floating-rate notes, FX forwards, FX/equity options, CDSs,
  caps/floors, caplets/floorlets, and rate futures;
- native price, fair-rate, cashflow, and sensitivity requests.

Set the clean-price, dirty-price, or Z-spread anchor when constructing a
trade with `QS.TRADE.FIXED.BOND`. The dedicated bond valuation functions
use that trade's anchor:

```text
=QS.BOND.PRICE(bond,context)
=QS.BOND.MODIFIED.DURATION(bond,context)
=QS.BOND.Z.SPREAD(bond,context)
=QS.BOND.SENSITIVITIES(bond,context)
```

The sensitivity spill contains curve risk and `dPV/dZSpread` in the same
table. Callable terms are not part of `QS.TRADE.FIXED.BOND`; the separate
`SteppedCouponBond` callable product is currently exposed by the Rust and
Python APIs. Instrument-level pricer selection and typed model assignments
are loaded with `QS.PRICER.CONFIGS.FROM.JSON` and passed to
`QS.CONTEXT.CREATE`.

## Object handles and quote updates

An Excel cell cannot contain a Rust object, so constructors return a revisioned
token such as:

```text
QSObject:QuoteStore:usd_quotes:3
```

The object itself remains an owned QuantSupport value in the XLL registry.
Constructors and mutators cache their defining worksheet inputs. Pressing F9
with unchanged cells returns the current handles and does not rebuild the
objects. When an input changes, the affected function returns the next
revision; that changed token makes Excel recalculate the `PricingContext` and
all dependent prices.

For example:

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

When a quote cell changes, `QS.QUOTES.SET` replaces that quote and returns a new token;
`QS.CONTEXT.CREATE` reruns `PricingContext::initialize`, so curves,
volatilities, simulations, and the price are rebuilt consistently. Multiple
updates must form a chain (`A1 -> A2 -> A3 ...`). Reads reject stale handles,
which prevents a price from silently using an older context.

The optional final `quote_level` argument of `QS.CONTEXT.CREATE`, after
`base_index`, selects `"Mid"` (the default), `"Bid"`, or `"Ask"` for market
construction and model calibration. Names are case-insensitive; blank uses
mid. For example, append `,"Bid"` after `"SOFR"` in the context formula above.
Changing the level rebuilds the context. Every consumed quote must contain
the requested level.

Recalculating a mutator may supply its original upstream revision after another
formula advanced the object. Mutators therefore apply changed inputs to the
current object identity, while repeated inputs return its current revision.
Constructors compare their full input set before creating the replacement, so
an unchanged F9 calculation does not repeat curve bootstrapping or trade
construction. `QS.OBJECT.COUNT()` remains stable across recalculation.

Objects are process-local and cleared when the XLL unloads. Workbooks must keep
the formulas and market inputs needed to recreate them. Use workbook-qualified
names when several workbooks may be open in one Excel process.

When rows are removed or identifiers are renamed, the old mutation can remain
in a store because no worksheet formula explicitly removes it. Put
`QS.OBJECT.RESET` (also available as `QS.STORE.RESET`) between the constructor
and its mutation chain, and drive it with a configuration version or signature:

```text
A1 =QS.QUOTES.CREATE("usd_quotes",$B$1)
A2 =QS.OBJECT.RESET(A1,$Z$1)
A3 =QS.QUOTES.SET(A2,"OIS_USD_SOFR_1Y",B3,,)
```

Changing `Z1` clears the store once, advances its handle revision, and causes
the unchanged `QS.QUOTES.SET` formulas downstream to repopulate it. The same
pattern works for fixing/FX stores, configuration collections, scenarios, and
constructed-element stores. It intentionally rejects immutable curves,
contexts, and trades; rebuild those from their reset inputs instead.

## Add a curve and price a swap

The example workbook builds a bootstrapped SOFR curve entirely from worksheet
cells. Put quote identifiers in column A, market levels in column B, and chain
the returned `QuoteStore` revisions in column C:

```text
B1  2026-02-24
A3  FixedRateDeposit_USD_SOFR_1D    B3  4.500%    C3 =QS.QUOTES.SET(QS.QUOTES.CREATE("quotes",$B$1),A3,B3,,)
A4  OIS_USD_SOFR_1Y                 B4  4.837%    C4 =QS.QUOTES.SET(C3,A4,B4,,)
...copy the C4 pattern down for the remaining OIS pillars...
```

Create a configuration collection, then add the curve by passing the visible
identifier range. `LogLinear` may be replaced with `Linear` or `CubicSpline`.

```text
D3 =QS.CURVE.CONFIGS.CREATE("curves")
D4 =QS.CURVE.CONFIGS.ADD(D3,"SOFR","Actual360","LogLinear",TRUE,A3:A10)
D5 =QS.CONTEXT.CREATE("ctx",C10,D4,,,,,,,,,"USD","SOFR")
D6 =QS.TRADE.SWAP("swap",$B$1,$B$1,"2031-02-24","USD","SOFR",10000000,0.0425,0,"Receive","Actual360","Simple","Semiannual","Semiannual","Quarterly")
D7 =QS.PRICE(D6,D5)
D8 =QS.FAIR.RATE(D6,D5)
```

To configure another curve, add its quote rows and call
`QS.CURVE.CONFIGS.ADD` again with the handle returned in `D4`. Every curve
setting and pillar selection remains visible and editable in the workbook.

Complex indices use explicit prefixes in worksheet arguments:
`Equity:SPX`, `Credit:CORPORATE-A`, `FX:EUR/USD`, and `Collateral:EUR/USD`.

## Inspect a bootstrapped curve

Curves now have one query workflow regardless of whether they were created as
flat curves or bootstrapped inside a context. Extract the context curve once,
then use the same discount, forward, and table functions everywhere:

```text
G2 =QS.CURVE.GET(D5,"SOFR")
G3 =QS.CURVE.DISCOUNT(G2,"2031-02-24")
G4 =QS.CURVE.FORWARD(G2,"2030-02-24","2031-02-24",,)
G5 =QS.CURVE.TABLE(G2,,)
```

Blank rate-convention arguments default to `Simple` and `Annual`.
`QS.CURVE.TABLE` spills pillar, date, discount-factor, zero-rate, and
prior-node-forward columns. The former context-specific discount/forward
functions were removed because they duplicated this API. Likewise, construct
a flat curve with `QS.CURVE.FLAT` and add it with `QS.CURVE.ADD`; the redundant
combined flat-add function is no longer registered.

The checked-in workbook can be regenerated after building the XLL:

```powershell
powershell -ExecutionPolicy Bypass -File bindings/excel/examples/create_swap_example.ps1
```

## Optional JSON imports

Worksheet-native quote and curve configuration is shown above. For systems that
already store QuantSupport JSON, the add-in also exposes import functions:

```text
=QS.QUOTES.FROM.JSON("quotes", quotes_json)
=QS.FIXINGS.FROM.JSON("fixings", fixings_json)
=QS.FX.FROM.JSON("fx", fx_records_json)
=QS.CURVE.CONFIGS.FROM.JSON("curves", curve_configs_json)
=QS.CREDIT.CURVE.CONFIGS.FROM.JSON("credit", credit_configs_json)
=QS.VOL.SURFACE.CONFIGS.FROM.JSON("surfaces", surface_configs_json)
=QS.VOL.CUBE.CONFIGS.FROM.JSON("cubes", cube_configs_json)
=QS.PRICER.CONFIGS.FROM.JSON("pricers", pricer_configs_json)
=QS.SIMULATION.CONFIGS.FROM.JSON("simulations", simulation_configs_json)
=QS.SCENARIOS.FROM.JSON("scenario", scenario_json)
```

A scenario JSON record contains its ordered shocks, for example
`{"shocks": [{"target": "SOFR", "value": 0.0001, "type": "Absolute"}]}`.
The worksheet context constructor materializes a new market snapshot when a
scenario handle is supplied. Rust and Python apply scenarios through borrowed
market views.

Pass those handles to `QS.CONTEXT.CREATE`. Optional arguments can be omitted
with empty argument positions or supplied through blank cells.

## Swaptions, equity forwards, and calibration reports

`QS.TRADE.EQUITY.FORWARD(name, trade_date, delivery_date, market_index,
currency, strike, notional, side, day_counter)` creates an equity-forward
position. `market_index` uses the canonical `EQ:SPX` form, and the context needs
a spot fixing at its evaluation date plus the index's forward curve. An absent
dividend curve means zero dividends. `QS.PRICE`, `QS.FAIR.RATE`, and
`QS.SENSITIVITIES` use the shared Rust forward pricer and the CSA discount policy.

`QS.TRADE.EUROPEAN.SWAPTION(name, trade_date, expiry_date, maturity_date,
currency, market_index, strike, notional, swaption_type, side, day_counter,
compounding, rate_frequency, fixed_leg_frequency, floating_leg_frequency,
calendar)` creates a payer or receiver swaption. Use `Payer` or `Receiver` for
exercise direction and `LongReceive` or `PayShort` for position side. A blank
calendar uses the Rust builder's default. The context must load an explicit
`EuropeanSwaption` / `HullWhiteClosedForm` pricer configuration with a
Hull–White model assigned to the trade's market index. The JSON schema is the
same as Python's `PricerConfiguration`: `parameter_source` accepts `Calibrated`
with the same surface/cube source and basket used by callable bonds and
simulations, or `Fixed` for an explicit sigma. Load the corresponding volatility
configuration into the context; cube baskets must select one tenor per expiry.
`QS.PRICE` uses the shared calibrated piecewise volatility schedule, and
`QS.SENSITIVITIES` includes curve and calibration volatility pillars.

`QS.SIMULATION.CALIBRATION(context, market_index)` spills the Hull–White
simulation's calibration records, including quote identifiers, market/model
prices, calibrated sigma, forward and strike, and signed `price_error`. A
simulation without those diagnostics returns an explanatory worksheet error.
The eleven columns match Python's `Simulation.calibration_diagnostics()`.

`QS.CALENDAR.IS.BUSINESS.DAY(date, calendar)` checks a market date;
`QS.CALENDAR.ADJUST(date, calendar, convention)` returns an adjusted ISO date.
`QS.SCHEDULE.ADJUSTED(start_date, end_date, frequency, calendar, convention)`
spills an adjusted schedule. `Following`, `ModifiedFollowing`, `Preceding`, and
other library business-day conventions are accepted.

All calendar inputs accept `UnitedKingdom(Settlement)`, `Japan`, and joint names
such as `JointCalendar(JoinHolidays,UnitedKingdom(Settlement),Japan)`.

The bindings workflow runs Windows worksheet/registry tests, builds the release
XLL with `scripts/build.ps1`, and uploads `excel-windows-x64` as a downloadable
Actions artifact alongside the Python wheels. Native Excel loading still
requires 64-bit Windows Excel.

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
Excel's main calculation thread. Parameter hints are inferred from argument
names in `src/function_help.rs`; add a specific description there when a new
argument name has domain-specific accepted values.

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

Bond anchors are trade inputs: `QS.TRADE.FIXED.BOND` requires trailing
`pricing_anchor` (`CleanPrice`, `DirtyPrice`, or `ZSpread`) and `anchor_value`
arguments. The `QS.BOND.*` valuation functions accept only the stored trade
and context handles. Side is stored on the trade and applied once in valuation.

# Volatility Surfaces

Option markets quote volatility across expiry, strike, and sometimes underlying tenor. A volatility surface or cube organizes those observations into a queryable market object that option pricers and model calibrators can share. This chapter explains how quote identifiers define the axes, how builders construct interpolated objects, how callers query them, and how dynamics models use them as calibration targets.

The volatility components live in `src/volatility/`. Their construction follows the same lifecycle as curves. A configuration identifies source quotes, a builder resolves and validates them, and the resulting object is stored in the `ConstructedElementStore`.

## Configurations

A surface represents expiry and smile coordinates. A cube adds an underlying-tenor coordinate, which is needed for markets such as swaptions. The configuration records the market identity, quoting convention, smile coordinate, and quote membership:

```rust,ignore
pub struct VolatilitySurfaceConfiguration {
    market_index: MarketIndex,           // required
    volatility_type: VolatilityType,     // default Black
    smile_type: SmileType,               // default Strike
    quotes: Vec<String>,                 // expiry × strike pillars
}
pub struct VolatilityCubeConfiguration { /* same fields; quotes are expiry × tenor × strike */ }

VolatilitySurfaceConfiguration::new(market_index, volatility_type, smile_type, quotes)
VolatilityCubeConfiguration::new(market_index, volatility_type, smile_type, quotes)
```

Configuration can also be loaded from JSON. The following example describes a SOFR caplet surface quoted in Black volatility against absolute strikes:

```json
{
  "market_index": "SOFR",
  "volatility_type": "Black",
  "smile_type": "Strike",
  "quotes": [
    "CapletFloorlet_USD_SOFR_3M_6M_Absolute_0.035_Straddle_Black",
    "CapletFloorlet_USD_SOFR_3M_6M_Absolute_0.045_Straddle_Black",
    "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black"
  ]
}
```

The quote identifiers provide the actual grid points. `volatility_type` and `smile_type` explain how their numbers should be interpreted. The central conventions are:

| Enum             | Variants                                | Meaning                                              |
| ---------------- | --------------------------------------- | ---------------------------------------------------- |
| `VolatilityType` | `Black`, `Normal`                       | lognormal (Black-76) or Bachelier quoting            |
| `SmileType`      | `Strike`, `Delta`, `LogMoneyness`       | what the second axis (`key`) means                   |
| `Strike`         | `Absolute(f64)`, `Atm`, `Relative(f64)` | `resolve(forward)` returns `K`, `F`, or `F + spread` |

Together, these conventions determine how a numerical key maps to an option strike and which pricing formula interprets the quoted volatility.

## Quote identifiers and axes

Builders recover coordinates from the structured quote identifiers in the store. This convention keeps a configuration compact and gives every output sensitivity a stable market label. The examples below show which parts of each identifier become grid axes:

| Identifier                                                    | Axes                                                                                                              |
| ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `CapletFloorlet_USD_SOFR_3M_6M_Absolute_0.045_Straddle_Black` | index tenor `3M`, **expiry `6M`**, **strike `0.045`**, strategy `Straddle`, vol type `Black`                      |
| `Swaption_CLP_ICP_1Y_2Y_Absolute_0.045_Black`                 | **expiry `1Y`**, **swap tenor `2Y`**, **strike `0.045`** (optional `PayFreq_RecvFreq` segments before the strike) |
| `FxCall_USDCLP_6M_Absolute_950`, `FxPut_...`                  | expiry, strike (FX surfaces)                                                                                      |
| `EquityCall_USD_AAPL_1Y_Absolute_150`                         | expiry, strike (equity surfaces)                                                                                  |

Caplet quotes populate an expiry-by-strike surface. Swaption quotes populate an expiry-by-tenor-by-strike cube. FX and equity option quotes also use surfaces because their contracts have no swap-tenor dimension.

## Builders

Builders resolve the configured identifiers at one market level, validate their coordinates, and assemble one object per market index. This is the direct construction API:

```rust,ignore
let surfaces: HashMap<MarketIndex, VolatilitySurfaceElement> =
    VolatilitySurfaceBuilder::new(surface_specs).build(&quote_store, Level::Mid)?;
let cubes: HashMap<MarketIndex, VolatilityCubeElement> =
    VolatilityCubeBuilder::new(cube_specs).build(&quote_store, Level::Mid)?;
```

`PricingContext::initialize()` invokes the same builders after constructing the curves. Applications add their configurations through `with_volatility_surface_configurations` and `with_volatility_cube_configurations`. Each resolved quote becomes a `DualFwd` leaf, so an option sensitivity is reported under the identifier of the market volatility that generated it.

## Querying

Once constructed, a surface can answer at quoted nodes and intermediate coordinates through one interface. `InterpolatedVolatilitySurface<T>` implements the `VolatilitySurface` trait with the following queries:

| Method                                                                    | Notes                                                       |
| ------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `volatility_from_period(expiry: Period, key: f64) -> Result<T>`           | bilinear in (expiry year fraction, key), flat extrapolation |
| `volatility_from_date(date: Date, key: f64) -> Result<T>`                 | converts the date to a period first                         |
| `volatility_type()`, `smile_type()`, `market_index()`, `reference_date()` |                                                             |

`InterpolatedVolatilityCube<T>` adds the tenor axis through `volatility_from_period(expiry, tenor, key)` and uses trilinear interpolation. The next example asks a SOFR surface for a nine-month volatility at a 3.25 percent absolute strike:

```rust,ignore
let elem = &surfaces[&MarketIndex::SOFR];
let vol = elem.surface().volatility_from_period(Period::from_str("9M")?, 0.0325)?;
println!("9M / 3.25% Black vol = {:.4}", vol.value());
```

The query returns a differentiable value, so subsequent pricing retains its link to neighboring quote pillars. The `examples/volatilitysurface` program builds the SOFR caplet surface from `examples/volatilitysurface/data` and prints interpolated values across an expiry-and-strike grid together with the surface conventions.

### FX orientation

An FX surface is stored for one pair direction because reciprocal pairs describe the same market under a coordinate transformation. `OrientedFxVolSurface::new(&element, inverted)` provides the requested trade orientation. For the inverse pair, it maps the strike according to \\(K \to 1/K\\). `FxOptionPricer` derives the orientation from the trade, which allows one stored surface to serve both `USDCLP` and `CLPUSD` options.

## Model calibration targets

Each `ModelConfiguration` variant obtains parameters through
`ParameterSource`. The `Fixed` variant contains a complete model parameter
set. The `Calibrated` variant identifies a constructed volatility market and
the option instruments used by the model calibrator.

The type definitions below show the common choice and the parameter type used by one-factor Gaussian rate models. `ModelCalibrationConfiguration` then describes the market and semantic basket used by calibration:

```rust,ignore
pub enum ParameterSource<P, C> {
    Fixed(P),
    Calibrated(C),
}
pub struct GaussianRateModelParameters {
    sigma: f64,
}
pub struct ModelCalibrationConfiguration {
    source: CalibrationSource, // Surface { market_index } | Cube { market_index }
    calibration_basket: CalibrationBasket, // optional expiries, tenors and strike
}
```

In JSON, `parameter_source` contains one of the two choices. A calibrated Hull-White configuration can select ATM instruments at specified expiries:

```json
"parameter_source": {
  "Calibrated": {
    "source": { "Surface": { "market_index": "SOFR" } },
    "calibration_basket": {
      "expiries": ["1Y", "2Y", "5Y"],
      "strike": "Atm"
    }
  }
}
```

For a model whose volatility is supplied directly, the same field contains its complete fixed parameter set:

```json
"parameter_source": { "Fixed": { "sigma": 0.01 } }
```

`VolatilitySurfaceConfiguration` and `VolatilityCubeConfiguration` define
market membership through their quote identifiers. The constructed volatility
object retains those instrument identifiers. `calibration_basket` then selects
expiries, tenors, and a strike rule from that market. An omitted expiry or
tenor axis includes every available value. Surface tenors identify the
floating-rate index tenor. Cube tenors identify the underlying swap tenor.

Every supplied expiry and tenor is required to occur in the source quote
identifiers. After applying both axes, every supplied value must participate
in at least one selected instrument. Resolution returns `NotFoundErr` before
calibration when a requested value is absent or has no instrument matching the
other axis. Basket selection therefore uses the configured quote coordinates
exactly. Missing expiries or tenors produce a configuration error.
For example, `tenors: ["6M"]` returns `NotFoundErr` when a caplet surface
contains only `3M` index-tenor quotes.

`calibration_instrument_ids()` provides the option contracts available to this
selection step. Their identifiers carry the expiry, tenor, and strike metadata
read by `calibration_basket`. `Pillars` provides the differentiable variables
registered on the AD tape, and its labels identify sensitivity results. The
two lists are equal for an interpolated quote grid. A parametrised surface can
keep the fitted option contracts as calibration instrument identifiers and
expose its fitted parameters as AD pillars.

Hull-White and LGM calibrators convert caplet or swaption prices into a
piecewise-constant short-rate sigma schedule. Brownian motion uses
`bootstrap_black_term_volatility` to strip a forward-volatility schedule that
satisfies
\\(\int_0^{T_i}\sigma(s)^2\\,ds = \sigma_i^2 T_i\\) at every selected expiry.
The bootstrap rejects negative forward variance because it indicates an
arbitrageable total-variance term structure.

`VolatilitySourceConfiguration` supports lower-level components that directly
sample a constant, surface, or cube. Model configurations obtain complete
parameter sets through `ParameterSource::{Fixed, Calibrated}`.

## What to remember

A volatility configuration owns the market quote membership and quoting conventions. Its builder converts those quotes into a surface or cube that supports interpolation, automatic differentiation, and calibration-instrument provenance. Model configurations then choose between a complete fixed parameter set and calibration to one of these constructed markets.

This division allows grid interpolation and future parametrized representations such as SABR to satisfy the same volatility query contract. It also gives Hull-White, LGM, HJM, and other dynamics models a common route to market data. Each dynamics model remains responsible for its own parameter type and calibration logic.

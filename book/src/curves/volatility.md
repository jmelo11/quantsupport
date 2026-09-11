# Volatility Surfaces

Volatility objects are built from quotes exactly like curves: a configuration lists quote identifiers, a builder resolves them and produces an interpolated object stored in the `ConstructedElementStore`. Source: `src/volatility/`.

## Configurations

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

| Enum             | Variants                                | Meaning                                              |
| ---------------- | --------------------------------------- | ---------------------------------------------------- |
| `VolatilityType` | `Black`, `Normal`                       | lognormal (Black-76) or Bachelier quoting            |
| `SmileType`      | `Strike`, `Delta`, `LogMoneyness`       | what the second axis (`key`) means                   |
| `Strike`         | `Absolute(f64)`, `Atm`, `Relative(f64)` | `resolve(forward)` returns `K`, `F`, or `F + spread` |

## Quote identifiers and axes

| Identifier                                                    | Axes                                                                                                              |
| ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `CapletFloorlet_USD_SOFR_3M_6M_Absolute_0.045_Straddle_Black` | index tenor `3M`, **expiry `6M`**, **strike `0.045`**, strategy `Straddle`, vol type `Black`                      |
| `Swaption_CLP_ICP_1Y_2Y_Absolute_0.045_Black`                 | **expiry `1Y`**, **swap tenor `2Y`**, **strike `0.045`** (optional `PayFreq_RecvFreq` segments before the strike) |
| `FxCall_USDCLP_6M_950`, `FxPut_...`                           | expiry, strike (FX surfaces)                                                                                      |
| `EquityCall_USD_AAPL_1Y_150`                                  | expiry, strike (equity surfaces)                                                                                  |

Caplet quotes populate a surface (expiry × strike); swaption quotes populate a cube (expiry × tenor × strike).

## Builders

```rust,ignore
let surfaces: HashMap<MarketIndex, VolatilitySurfaceElement> =
    VolatilitySurfaceBuilder::new(surface_specs).build(&quote_store, Level::Mid)?;
let cubes: HashMap<MarketIndex, VolatilityCubeElement> =
    VolatilityCubeBuilder::new(cube_specs).build(&quote_store, Level::Mid)?;
```

Inside `PricingContext::initialize()` the same builders run after the curves (`with_volatility_surface_configurations`, `with_volatility_cube_configurations`). Each quote value becomes a `DualFwd` leaf, so option sensitivities are reported per volatility quote identifier.

## Querying

`InterpolatedVolatilitySurface<T>` implements the `VolatilitySurface` trait:

| Method                                                                    | Notes                                                       |
| ------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `volatility_from_period(expiry: Period, key: f64) -> Result<T>`           | bilinear in (expiry year fraction, key), flat extrapolation |
| `volatility_from_date(date: Date, key: f64) -> Result<T>`                 | converts the date to a period first                         |
| `volatility_type()`, `smile_type()`, `market_index()`, `reference_date()` |                                                             |

`InterpolatedVolatilityCube<T>` adds the tenor axis: `volatility_from_period(expiry, tenor, key)` (trilinear).

```rust,ignore
let elem = &surfaces[&MarketIndex::SOFR];
let vol = elem.surface().volatility_from_period(Period::from_str("9M")?, 0.0325)?;
println!("9M / 3.25% Black vol = {:.4}", vol.value());
```

`examples/volatilitysurface` (`cargo run -p volatilitysurface`) builds the SOFR caplet surface from `examples/volatilitysurface/data` and prints interpolated vols for a grid of expiry/strike points plus the volatility and smile types.

### FX orientation

FX surfaces are stored for one pair direction. `OrientedFxVolSurface::new(&element, inverted: bool)` exposes `volatility_from_date(expiry, strike)` and, when `inverted`, maps the strike as \\(K \to 1/K\\) so the same surface serves both `USDCLP` and `CLPUSD` trades. `FxOptionPricer` chooses the orientation from the trade's pair.

## Volatility sources for models

Models and simulations do not take raw surfaces; they take a `VolatilitySourceConfiguration`:

```rust,ignore
pub enum VolatilitySourceConfiguration {
    Constant { value: f64 },
    Surface { market_index: MarketIndex, key: f64 },
    Cube { market_index: MarketIndex, tenor: Period, key: f64 },
    Calibrated(ModelCalibrationConfiguration),
}
pub struct ModelCalibrationConfiguration {
    source: CalibrationSource,        // Surface { market_index } | Cube { market_index }
    quote_ids: Vec<String>,           // instruments to fit
    strike: Option<Strike>,           // e.g. "Atm" overrides the quoted strike
    alpha: f64,                       // mean reversion used while fitting
}
```

```json
{ "Constant": { "value": 0.2 } }
{ "Surface": { "market_index": "SOFR", "key": 0.03 } }
{ "Cube": { "market_index": "ICP", "tenor": "1Y", "key": 0.045 } }
{ "Calibrated": { "source": { "Surface": { "market_index": "SOFR" } },
                  "quote_ids": ["CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black"],
                  "strike": "Atm", "alpha": 0.1 } }
```

`bootstrap_black_term_volatility(&config, &store, reference_date, day_counter) -> Result<PiecewiseConstantVolatility<f64>>` reads the implied vol \\(\sigma_i\\) at each calibration quote and strips a piecewise-constant forward volatility so that \\(\int_0^{T_i}\sigma(s)^2\\,ds = \sigma_i^2 T_i\\) at every pillar; negative forward variance is rejected as arbitrageable. `PiecewiseConstantVolatility::new(schedule)` requires a non-empty, strictly increasing `(year_fraction, sigma)` schedule and implements `TimeDependentVolatility::vol(t)`.

Hull-White and LGM use the `Calibrated` variant to fit their short-rate sigma schedule instead; see [Hull-White](../simulation/hull-white.md).

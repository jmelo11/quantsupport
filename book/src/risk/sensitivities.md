# Sensitivities

## Requesting

```rust,ignore
let results = pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &ctx)?;
let sens = results.sensitivities().ok_or(/* ... */)?;
for (key, dv) in sens.instrument_keys().iter().zip(sens.exposure()) {
    println!("{key:40} {dv:12.2}");
}
```

Sensitivities are only available when the context and trade use `DualFwd`. Values are \\(\partial \text{NPV} / \partial q\\) for each quote \\(q\\) in its own units (rate quotes in absolute rate: multiply by `1e-4` for a DV01 per basis point).

## `SensitivityMap`

```rust,ignore
pub struct SensitivityMap { instrument_key: Vec<String>, exposure: Vec<f64> }
impl SensitivityMap {
    pub fn instrument_keys(&self) -> &[String];
    pub fn exposure(&self) -> &[f64];
    pub fn with_instrument_keys(self, keys: &[String]) -> Self;
    pub fn with_exposure(self, exposure: &[f64]) -> Self;
    pub fn aggregate(self) -> Self;   // sums duplicate keys, keeps first-occurrence order
}
```

`aggregate()` is applied by the pricers: when a child curve (a basis or collateral curve) depends on a parent curve, the IFT produces contributions to the parent quotes from both curves; they are summed under one label.

## What appears in the table

| Market element             | Labels                                                                                                                                               |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Discount/projection curves | the quotes in the `CurveConfiguration` (`OIS_USD_SOFR_1Y`, `Swap_CLP_ICP_5Y`, `Deposit_USD_SOFR_1W`, `BasisSwap_*`, `FloatFloatCrossCurrencySwap_*`) |
| Credit curves              | `Cds_*` quotes                                                                                                                                       |
| Volatility surfaces/cubes  | `CapletFloorlet_*`, `Swaption_*`, `FxCall_*` quotes (vega ladder)                                                                                    |
| FX spot                    | only if the spot was added to the `FxStore` as `DualFwd::new`                                                                                        |
| Model parameters           | Hull-White/LGM sigma pillars when built with `HullWhiteTimeDependentVolatility::with_pillar_labels().with_ift_sensitivities()`                       |

Quotes that do not influence the price are omitted (zero adjoint).

## Portfolio aggregation

Sum maps across trades keyed by label:

```rust,ignore
let mut total: BTreeMap<String, f64> = BTreeMap::new();
for r in results {
    if let Some(s) = r.sensitivities() {
        for (k, v) in s.instrument_keys().iter().zip(s.exposure()) {
            *total.entry(k.clone()).or_default() += v;
        }
    }
}
```

Because all trades share the same quote leaves, the summed ladder is the exact portfolio sensitivity.

## Example

`cargo run -p sensitivity` prices SOFR, Term SOFR, ICP and USD/CLP cross-currency swaps and prints, per trade, the NPV followed by a table of quote identifier and exposure. The Term SOFR swap shows both `BasisSwap_USD_SOFR_TermSOFR3m_*` and `OIS_USD_SOFR_*` rows; the cross-currency swap adds `OIS_CLP_ICP_*` and `FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_*`.

## Verifying against bumps

For a check, shock a quote with a `Scenario` and reprice:

```rust,ignore
let base = ctx.evaluate(&trade, &[Request::Value])?.price();
let mut bumped = base_ctx.with_scenarios(vec![Scenario::new("OIS_USD_SOFR_5Y", 1e-4, ScenarioType::Absolute)]);
bumped.initialize()?;
let fd = (bumped.evaluate(&trade, &[Request::Value])?.price() - base) / 1e-4;
```

`fd` should match the `OIS_USD_SOFR_5Y` entry to first order.

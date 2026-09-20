# Sensitivities

A sensitivity measures how a valuation changes with one market input. QuantSupport reports these derivatives under quote identifiers, which makes the output suitable for hedging, limit aggregation, and comparison with finite-difference checks. This chapter explains how to request risk, interpret `SensitivityMap`, aggregate a portfolio, and verify selected results.

## Requesting

Sensitivity calculation is one of the outputs selected through `Request`. The following example asks for value and sensitivities, then prints each quote label with its derivative:

```rust,ignore
let results = pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &ctx)?;
let sens = results.sensitivities().ok_or(/* ... */)?;
for (key, dv) in sens.instrument_keys().iter().zip(sens.exposure()) {
    println!("{key:40} {dv:12.2}");
}
```

The context and trade must use `DualFwd` so the market graph carries derivative information. Each value is \\(\partial \text{NPV} / \partial q\\) for one quote \\(q\\) in that quote's own units. Rate quotes use absolute decimal rates, so multiplying their derivative by `1e-4` gives the value change for one basis point.

## `SensitivityMap`

`SensitivityMap` stores parallel vectors of market labels and exposures. Its builder-style methods support construction by pricers, and `aggregate` combines contributions that reach one label through several dependency paths:

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

Pricers apply `aggregate()` before returning results. When a basis or collateral curve depends on a parent curve, implicit differentiation can produce several contributions to one parent quote. Aggregation sums those values under one stable label and preserves first-occurrence ordering.

## What appears in the table

The rows are determined by the market objects reached during valuation and by the pillars those objects expose. The principal label families are:

| Market element             | Labels                                                                                                                                               |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Discount/projection curves | the quotes in the `CurveConfiguration` (`OIS_USD_SOFR_1Y`, `Swap_CLP_ICP_5Y`, `Deposit_USD_SOFR_1W`, `BasisSwap_*`, `FloatFloatCrossCurrencySwap_*`) |
| Credit curves              | `Cds_*` quotes                                                                                                                                       |
| Volatility surfaces/cubes  | `CapletFloorlet_*`, `Swaption_*`, `FxCall_*` quotes (vega ladder)                                                                                    |
| FX spot                    | only if the spot was added to the `FxStore` as `DualFwd::new`                                                                                        |
| Model parameters           | Hull-White/LGM sigma pillars when built with `HullWhiteTimeDependentVolatility::with_pillar_labels().with_ift_sensitivities()`                       |

Rows with a zero adjoint are omitted, keeping the report focused on active dependencies.

## Portfolio aggregation

Portfolio risk is the sum of trade-level derivatives under each label. A `BTreeMap` provides a compact deterministic aggregation:

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

All trades in the context share the same quote leaves. The summed ladder is therefore the derivative of total portfolio value under the same market construction.

## Example

The `sensitivity` example prices SOFR, Term SOFR, ICP, and USD/CLP cross-currency swaps. For each trade, it prints NPV followed by quote identifiers and exposures. The Term SOFR swap reaches both `BasisSwap_USD_SOFR_TermSOFR3m_*` and `OIS_USD_SOFR_*`. The cross-currency swap additionally reaches `OIS_CLP_ICP_*` and `FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_*`. These rows reveal the curve dependency graph in market terms.

## Verifying against bumps

A small quote scenario provides an independent numerical check for a selected row. This example moves the five-year SOFR OIS quote by one basis point, rebuilds the context, and calculates the finite-difference derivative:

```rust,ignore
let base = ctx.evaluate(&trade, &[Request::Value])?.price();
let mut bumped = base_ctx.with_scenarios(vec![Scenario::new("OIS_USD_SOFR_5Y", 1e-4, ScenarioType::Absolute)]);
bumped.initialize()?;
let fd = (bumped.evaluate(&trade, &[Request::Value])?.price() - base) / 1e-4;
```

For a sufficiently small shock, `fd` should match the `OIS_USD_SOFR_5Y` entry to first order. Larger shocks also include curvature and therefore serve as stress tests of the linear sensitivity approximation.

## What to remember

Sensitivity labels identify observable market inputs, and exposure values measure the price derivative in each input's native units. Calibration Jacobians carry risk through dependent curves and models, aggregation combines shared labels, and scenario repricing provides a practical validation tool.

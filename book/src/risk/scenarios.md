# Scenarios

Scenarios shock quotes before bootstrapping, so every curve, surface and simulation that depends on them is rebuilt consistently. Source: `src/quotes/scenario.rs`, `src/core/pricingcontext.rs`.

## `Scenario`

```rust,ignore
pub enum ScenarioType { Absolute, Relative }

pub struct Scenario { target: String, shock: f64, scenario_type: ScenarioType }

impl Scenario {
    pub fn new(target: impl Into<String>, shock: f64, scenario_type: ScenarioType) -> Self;
    pub fn matches(&self, identifier: &str) -> bool;
    pub fn shocked_value(&self, value: f64) -> f64;      // Absolute: v + shock; Relative: v · (1 + shock)
    pub fn apply(&self, store: &mut QuoteStore) -> Result<usize>;  // number of quotes shocked; error if none
}
```

**Matching** splits both strings on `_` and requires every segment of `target` to appear among the identifier's segments:

| Target                 | Matches                                                                                                                      |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `OIS_USD_SOFR_5Y`      | exactly that quote                                                                                                           |
| `SOFR`                 | every quote containing the `SOFR` segment (`OIS_USD_SOFR_*`, `BasisSwap_USD_SOFR_TermSOFR3m_*`, `CapletFloorlet_USD_SOFR_*`) |
| `USD_OIS`              | all USD OIS quotes regardless of tenor                                                                                       |
| `CapletFloorlet_Black` | all Black caplet vol quotes                                                                                                  |

Both bid and ask are shocked.

## Using with `PricingContext`

```rust,ignore
let mut ctx = PricingContext::new()
    .with_quote_store(quotes)
    .with_curve_configurations(curve_specs)
    .with_scenarios(vec![
        Scenario::new("OIS_USD_SOFR", 0.0001, ScenarioType::Absolute),      // +1bp parallel SOFR
        Scenario::new("CapletFloorlet_USD_SOFR", 0.10, ScenarioType::Relative), // vols ×1.10
    ]);
ctx.initialize()?;
```

On `initialize()` the context clones the base store, applies the scenarios in order, and stores the result as the _shocked_ store. `ctx.quote_store()` returns the shocked store when scenarios exist (otherwise the base), and `ctx.base_quote_store()` always returns the unshocked quotes. Curves, volatility surfaces and simulations are all bootstrapped from `quote_store()`.

## Patterns

- **Parallel shift**: one scenario with a partial target, e.g. `"OIS_USD_SOFR"`.
- **Key-rate ladder**: build one context per pillar target (`OIS_USD_SOFR_1Y`, `_2Y`, ...) and difference the NPVs — useful to validate the AD ladder from [Sensitivities](sensitivities.md).
- **Stress**: combine several scenarios (rates, vols, cross-currency basis) in one list; each is applied sequentially to the same store.
- **Relative FX moves**: target the FX spot quote (`FxSpot_USDCLP`-style identifiers) with `ScenarioType::Relative`.

Because scenarios act on quotes, all downstream consistency (multi-curve links, collateral curves, calibrated model vols) is preserved automatically, unlike bumping a curve node in isolation.

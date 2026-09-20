# Scenarios

Scenario analysis asks how a portfolio behaves under a specified market move. QuantSupport applies those moves to observable quotes before market construction, then rebuilds every dependent curve, surface, calibrated model, and simulation. This chapter explains target matching, context integration, and the main scenario patterns. The implementation spans `src/quotes/scenario.rs` and `src/core/pricingcontext.rs`.

## `Scenario`

A scenario contains a target expression, a shock size, and an absolute-or-relative interpretation. Its methods expose matching, single-value transformation, and application to a quote store:

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

Target matching splits the target and quote identifier on `_`. Every target segment must occur in the quote identifier. This creates a simple hierarchy from one exact instrument to a broad family:

| Target                 | Matches                                                                                                                      |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `OIS_USD_SOFR_5Y`      | exactly that quote                                                                                                           |
| `SOFR`                 | every quote containing the `SOFR` segment (`OIS_USD_SOFR_*`, `BasisSwap_USD_SOFR_TermSOFR3m_*`, `CapletFloorlet_USD_SOFR_*`) |
| `USD_OIS`              | all USD OIS quotes regardless of tenor                                                                                       |
| `CapletFloorlet_Black` | all Black caplet vol quotes                                                                                                  |

When a quote matches, the scenario updates all available levels, including bid, ask, and mid. `apply` returns the number of modified quotes and raises an error when the target matches no quote.

## Using with `PricingContext`

Scenarios become part of market initialization through `with_scenarios`. The example below applies a one-basis-point absolute SOFR shift and a ten-percent relative caplet-volatility move:

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

During `initialize()`, the context clones the base store, applies scenarios in list order, and stores the transformed result. `ctx.base_quote_store()` retains the original observations. `ctx.quote_store()` supplies the active observations used to build curves, volatility objects, and simulations. With an empty scenario list, both accessors describe the same market.

## Patterns

Target granularity and shock type combine into several useful workflows:

- **Parallel shift**: one scenario with a partial target, e.g. `"OIS_USD_SOFR"`.
- **Key-rate ladder**: build one context per pillar target (`OIS_USD_SOFR_1Y`, `_2Y`, ...) and difference the NPVs. This provides a finite-difference check of the AD ladder from [Sensitivities](sensitivities.md).
- **Stress**: combine several scenarios covering rates, volatilities, and cross-currency basis in one list. Each transformation is applied sequentially to the same store.
- **Relative FX moves**: target the FX spot quote (`FxSpot_USDCLP`-style identifiers) with `ScenarioType::Relative`.

## What to remember

A scenario describes a change in market language and lets normal initialization propagate that change through the full dependency graph. The rebuilt market preserves multi-curve links, collateral relationships, volatility construction, and calibrated-model parameters. Exact targets support focused validation, and partial targets support broad stresses with the same mechanism.

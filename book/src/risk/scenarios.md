# Scenarios

A `Scenario` combines ordered market shocks. Applying it returns a market view
that borrows a `PricingContext`; the context retains its original inputs and
constructed objects. Pricers and `Evaluator` accept this view through
`MarketDataProvider`.

## Applying a scenario in Rust

```rust,ignore
let scenario = Scenario::new(vec![
    Shock::new("OIS_USD_SOFR", 0.0001, ScenarioType::Absolute),
    Shock::new("CapletFloorlet_USD_SOFR", 0.10, ScenarioType::Relative),
]);
let stressed_market = scenario.apply(&context)?;
let stressed = pricer.evaluate(&trade, &[Request::Value], &stressed_market)?;
let base = pricer.evaluate(&trade, &[Request::Value], &context)?;
```

The first shock adds one basis point to SOFR OIS quotes. The second multiplies
matching caplet volatilities by 1.10. Each shock transforms every available quote
level; the context's selected `Level` determines which level construction uses.
Overlapping shocks compose in their supplied order.

## Applying a scenario in Python

```python
scenario = qs.Scenario(shocks=[
    qs.Shock(target="EURIBOR12m", value=0.01, shock_type=qs.ScenarioType.Absolute),
    qs.Shock(target="FUNDINGSPREAD", value=0.005, shock_type=qs.ScenarioType.Absolute),
])
with context, scenario:
    stressed = pricer.evaluate(trade=trade, context=context, requests=[qs.Request.Value])
with context:
    base = pricer.evaluate(trade=trade, context=context, requests=[qs.Request.Value])
```

The scenario applies to the innermost active context. Nested scenarios compose
in entry order. Leaving a scope releases its temporary market view, including
when pricing raises an exception. The same scenario can be reused with another
context. Scenario definitions belong outside the context constructor.

## Selecting market inputs

A target matches an exact identifier or a set of underscore-separated identifier
segments. Every target segment must occur in the identifier.

| Target | Inputs selected |
| --- | --- |
| `OIS_USD_SOFR_5Y` | One SOFR OIS quote |
| `OIS_USD_SOFR` | Every USD SOFR OIS quote |
| `CapletFloorlet_Black` | Every Black caplet quote |
| `FUNDINGSPREAD` | Every explicit zero-rate pillar of that spread curve |
| `FUNDINGSPREAD_2031-02-24` | One dated funding-spread zero-rate pillar |

Explicit zero-rate pillars have the same labels in scenario targets and
sensitivity reports. An absolute value of `0.0001` adds one basis point;
a relative value of `0.10` scales the input by 1.10. Empty targets, nonfinite
values and selectors matching no quote or explicit pillar produce an error.
Historical fixings remain unchanged.

## Construction and memory

Entering a scenario validates its shocks without constructing the entire market.
A pricing request constructs its required curves, volatility markets and
simulations, following their dependencies. Results remain in a cache for that
view. For a composite funding curve, a spread shock rebuilds the spread and
composite while an unchanged benchmark can reuse its calibrated values and
calibration metadata.

The view borrows quotes, historical fixings and FX inputs. Requested curve and
volatility objects have independent AD wrappers with detached tape pointers.
Unchanged simulation paths can be reused; an affected simulation is generated
when requested, using its configured seed. Unrelated simulations remain
unconstructed. Broad portfolio requests or shocks affecting many dependencies
still require the corresponding calibration and simulation work.

A pillar ladder uses separate scenarios against the same context. The resulting
finite differences can be compared with the labelled derivatives described in
[Sensitivities](sensitivities.md).

# Scenarios and full revaluation

A scenario measures the effect of a given market move. It changes the inputs,
rebuilds the affected market objects, and reprices the trade. Comparing the
repriced change with the sensitivity estimate shows how well a linear
approximation describes the move:

\\[
\Delta V=V(q+\Delta q)-V(q),\qquad
\Delta V_{\rm linear}=\sum_j
\frac{\partial V}{\partial q_j}\Delta q_j.
\\]

The two can differ because of curvature, a change in exercise or payoff, or
recalibration effects that partial model sensitivities leave out.

## Ordered input transformations

A `Scenario` is an ordered list of `Shock`s.

- An absolute shock adds its value to the quote.
- A relative shock multiplies the quote by one plus its value.

A shock target matches a quote identifier if either:

- the target equals the identifier exactly, or
- every underscore-separated part of the target appears in the identifier. The
  order and number of repeats of the parts do not matter.

Both rules always apply. A full quote identifier can therefore also match other
identifiers that contain the same parts. Repeated tenor or expiry parts can
make targets overlap, so one shock per full identifier can move some quotes
more than once.

- For a parallel stress of a quote family, use one family target and check the
  shocked value of every affected quote.
- For a bump of a single quote, check which quotes actually matched.

This fragment assumes an existing `context`, `trade`, and compatible `pricer`:

```rust,ignore
let scenario = Scenario::new(vec![
    Shock::new("OIS_USD_SOFR", 0.0001, ScenarioType::Absolute),
    Shock::new("CapletFloorlet_USD_SOFR", 0.10, ScenarioType::Relative),
]);
let stressed_market = scenario.apply(&context)?;
let stressed = pricer.evaluate(&trade, &[Request::Value], &stressed_market)?;
```

The first shock adds one basis point to the matching OIS quotes. The second
multiplies the matching optionlet volatilities by 1.10.

- Overlapping shocks are applied in the order given.
- All quote levels (bid, mid, ask) are shocked. The context then uses its
  selected level.

You can also shock direct zero-rate quotes. A funding spread curve may use
identifiers like `ZeroRate_EUR_OTHER:FUNDINGSPREAD_<tenor>`. The target
`OTHER:FUNDINGSPREAD` shocks all of them.

Scenarios shock quotes only. To change historical fixings or spot FX, change
those stores directly. Empty targets, non-finite shocks, and targets that match
no quote are errors.

## Independent views and construction

`scenario.apply(&context)` returns a `MarketDataProvider` that borrows the
context and has its own cache. The base quotes and built objects do not
change.

- Objects affected by the shock are rebuilt when a pricer asks for them.
- Unrelated simulations are not generated.
- Unaffected calibrated objects are reused.
- A rebuilt simulation uses its configured seed.

## Local validation and stress interpretation

For a single input, the central difference is

\\[
\frac{V(q+h)-V(q-h)}{2h}.
\\]

When you compare it with an AD sensitivity:

- use the same seeds, grids, policies, payment selection, and quote level;
- check that only the intended quote moved;
- try several values of \\(h\\) to separate curvature from solver or sampling
  noise. A single finite difference is not proof of the true derivative.

For a large stress, report the fully repriced change and the assumptions behind
it. A large parallel funding shock can move the whole composite discount curve,
while each sensitivity row is local to one pillar. Credit, prepayment, and
smoothed script payoffs can also make stress results differ a lot from a linear
sum of sensitivities.

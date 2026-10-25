# Scenarios and full revaluation

Scenarios measure the effect of a specified market move by changing inputs,
rebuilding the affected market objects and repricing the trade. Comparing the
revalued change with the sensitivity estimate shows how well a local linear
approximation describes that move:

\\[
\Delta V=V(q+\Delta q)-V(q),\qquad
\Delta V_{\rm linear}=\sum_j
\frac{\partial V}{\partial q_j}\Delta q_j.
\\]

The difference can reflect curvature, a change in exercise or payoff behavior,
and recalibration effects omitted from partial model sensitivities.

## Ordered input transformations

A `Scenario` contains ordered `Shock` definitions. An absolute shock adds its
value; a relative shock multiplies by one plus its value. The target matches
an exact identifier or requires every underscore-separated target segment to
occur in the input identifier. Segment order and multiplicity are ignored.
The exact-string branch does not prevent that same target from matching other
identifiers through the segment rule.

A fully qualified identifier is therefore not a universal exact-only selector.
Repeated tenor/expiry tokens can make targets overlap, so constructing one
shock per full quote identifier can move some quotes more than once. For a
parallel family stress, prefer one family selector and verify the transformed
values of every affected quote. An isolated finite-difference bump requires
checking the actual matched set rather than assuming the target is unique.

This fragment assumes an existing `context`, `trade` and compatible `pricer`:

```rust,ignore
let scenario = Scenario::new(vec![
    Shock::new("OIS_USD_SOFR", 0.0001, ScenarioType::Absolute),
    Shock::new("CapletFloorlet_USD_SOFR", 0.10, ScenarioType::Relative),
]);
let stressed_market = scenario.apply(&context)?;
let stressed = pricer.evaluate(&trade, &[Request::Value], &stressed_market)?;
```

The first change adds one basis point to matching OIS rate quotes; the second
scales matching optionlet volatilities by 1.10. Overlapping transformations
compose in supplied order. All available quote levels are transformed, then
the context's selected bid/mid/ask level is consumed.

Direct zero-rate quotes can also be targeted. A funding spread curve may
expose `ZeroRate_EUR_FUNDINGSPREAD_<tenor>` identifiers; targeting
`FUNDINGSPREAD` changes all matching spread quotes. Historical fixings and spot
FX are not implied targets of this quote mechanism. Empty targets, nonfinite shocks and
unmatched selectors are errors.

## Independent views and construction

`scenario.apply(&context)` returns a borrowed `MarketDataProvider` with an
independent cache. The base observations and constructed objects remain
unchanged. Requests rebuild affected dependencies; unrelated simulations are
not generated. Unchanged calibrated data can be reused with detached AD
wrappers. An affected simulation uses its configured seed when rebuilt.

In Python, the corresponding scope is `with context, scenario:`. The innermost
active context supplies the base market, nested scenarios compose in entry
order, and scope exit releases temporary state even on exceptions. Define
scenarios separately from the context constructor.

## Local validation and stress interpretation

For one input, a central comparison uses

\\[
\frac{V(q+h)-V(q-h)}{2h}.
\\]

Use identical seeds, grids, policies, payment selection and quote levels.
Check that only the intended target moved. Vary \\(h\\) to distinguish
curvature from solver/sampling noise; one finite difference is not a universal
truth about a derivative.

For a broad stress, report the full revalued change and the assumptions it
reconstructs. A large parallel funding shock can change the whole composite
discount curve, whereas a derivative row is local to one rate pillar. Credit,
prepayment and smoothed script behavior can also make stress results depart
substantially from a linear ladder.

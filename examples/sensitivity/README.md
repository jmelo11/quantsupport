# Sensitivity examples

Run these commands from the repository root.

## Risk reprojection and post-bootstrap zero-rate risk

```sh
cargo run -p sensitivity --bin risk-reprojection
```

The self-contained [risk-reprojection example](src/bin/risk-reprojection.rs)
bootstraps an ESTR curve from 1Y and 2Y deposit quotes, then prices a EUR 1 million
18-month receive-fixed OIS. It prints the same trade's NPV and sensitivities in
three bases:

- Original deposit quotes, using `PricingContext`.
- Implied 1Y and 2Y OIS par rates, using `context.reproject_risk(...)`.
- Continuously compounded Actual/365 zero rates at the bootstrapped curve nodes,
  using `context.with_zero_rate_risk(...)`.

The OIS reporting contracts need no stored market observations. Their implied
quotes come from the original curves. The example asserts that all three NPVs
are exactly equal and reports raw derivatives and first-order +1 bp P&L in EUR.
It requires no data files and writes no output files.

The target instrument basket must span the original curve risk. Reprojection
changes the local first-order risk basis while preserving the original pricing
curve and interpolation.

## Multi-curve portfolio risk

```sh
cargo run -p sensitivity
```

The original example loads `data/quotes.json` and `data/curve_specs.json`, prices
SOFR, Term SOFR, ICP and cross-currency swaps, and writes curve nodes, cashflows
and sensitivities to `data/qs_results.json`.

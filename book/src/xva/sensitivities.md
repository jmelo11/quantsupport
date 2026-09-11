# XVA Sensitivities

XVA depends on simulated market values, credit and funding inputs, calibrated models, and collateral assumptions. QuantSupport propagates AD values through the parallel exposure workflow to produce parameter and quote sensitivities.

XVA risk can include:

- domestic and foreign curve pillars;
- credit-curve pillars;
- funding spreads;
- calibrated model volatility parameters;
- FX and other simulated market factors.

Interpret the result according to the input's quoting unit. As with clean pricing, a derivative with respect to a decimal rate must be scaled by `0.0001` for a one-basis-point report.

Monte Carlo sensitivities contain sampling effects. Use identical seeds and factor ordering when comparing base and changed implementations. Increase paths to assess convergence and compare AD results with common-random-number finite differences on representative pillars.

The `cva` example calculates CVA/FVA and parallel AAD sensitivities. Run it in release mode because exposure valuation repeats the portfolio over many paths and dates.

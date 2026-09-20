# XVA Sensitivities

XVA sensitivities explain how simulated exposure and its valuation adjustments respond to market inputs and model assumptions. `XvaEngine::run` uses the tape-based differentiation framework shared with pricing. The market model begins from `DualFwd` leaves, simulated paths preserve those dependencies, aggregators remain differentiable, and a reverse sweep returns gradients under registered labels.

This chapter explains those labels, shows how to rank the results, and states the numerical conventions needed for interpretation and validation.

## Labels

`result.sensitivities` pairs each registered label with \\(\partial\text{XVA}/\partial\text{leaf}\\). Label families identify the economic source of the derivative:

| Label                                                                                                                                                                                        | Leaf                                                    |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| curve quote identifiers (`OIS_USD_SOFR_5Y`, `OIS_CLP_ICP_2Y`, `FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_5Y`)                                                                             | curve pillars mapped back to quotes through the IFT     |
| volatility pillar labels from `HullWhiteTimeDependentVolatility::with_pillar_labels()` (calibration quote identifiers such as `CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black`) | calibrated LGM sigma pillars                            |
| `FX.<pair>.spot`, `FX.<pair>.vol` (e.g. `FX.CLPUSD.spot`)                                                                                                                                    | FX spot and lognormal FX volatility per `FxModelConfig` |
| `<credit_index>.pillar_<i>`                                                                                                                                                                  | survival pillars of a bootstrapped credit curve         |
| `funding_spread.<date>` or `<funding_index>.<date>`                                                                                                                                          | funding spread term structure                           |

Curve labels refer to observable calibration quotes through the curve IFT. Volatility labels refer to the instruments used to calibrate LGM sigma schedules. FX, credit, and funding labels identify their direct model or term-structure inputs.

The returned values aggregate all netting sets included in the run. Running one set at a time produces a set-specific gradient when that reporting dimension is required.

## Example

Risk reports often rank values by absolute magnitude before grouping them by factor family. The following example sorts the engine result and prints the ten largest entries:

```rust,ignore
let result = engine.run(&mut netting_sets)?;
let mut sens = result.sensitivities.unwrap_or_default();
sens.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap_or(std::cmp::Ordering::Equal));
for (label, value) in sens.iter().take(10) {
    println!("{label:<55} {value:>12.4}");
}
```

For the `examples/cva` portfolio, large rows commonly include long-dated SOFR OIS quotes through exposure dynamics and calibration dependencies, cross-currency basis quotes through the collateralized CLP curve, and `FX.CLPUSD.vol` through simulated currency conversion.

## Notes

Four implementation conventions define the scope and statistical meaning of the reported gradient:

- The deterministic system-curve discounting sequence \\(P(0,t_k)\\) from the domestic curve is held fixed during differentiation. Curve sensitivities cover exposure and model paths.
- Value and gradient use the same paths and therefore share sampling noise. Their Monte Carlo error decreases as `n_paths` grows.
- Registered leaves such as `FX.<pair>.vol` appear in the report. The current engine treats `lambda` and `rho` as fixed configuration values.
- Validate by rerunning with a `Scenario` on the base quotes (see [Scenarios](../risk/scenarios.md)) and the same `seed`.

## What to remember

XVA risk follows one differentiable chain from quotes and model leaves through calibrated dynamics, simulated exposure, and aggregation. Labels preserve the economic origin of each derivative. Common random paths align values and gradients, and same-seed scenario runs provide a finite-difference validation for selected factors.

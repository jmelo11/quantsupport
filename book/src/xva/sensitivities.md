# XVA Sensitivities

`XvaEngine::run` computes sensitivities of every XVA value with the same tape-based AD used for pricing: the market model is built from `DualFwd` leaves, paths are simulated as `DualFwd`, aggregators are differentiated, and one reverse sweep returns the gradient with respect to every registered leaf. No bumping and re-simulation is required.

## Labels

`result.sensitivities: Option<Vec<(String, f64)>>` pairs a label with \\(\partial\text{XVA}/\partial\text{leaf}\\):

| Label                                                                                                                                                                                        | Leaf                                                    |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| curve quote identifiers (`OIS_USD_SOFR_5Y`, `OIS_CLP_ICP_2Y`, `FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_5Y`)                                                                             | curve pillars mapped back to quotes through the IFT     |
| volatility pillar labels from `HullWhiteTimeDependentVolatility::with_pillar_labels()` (calibration quote identifiers such as `CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black`) | calibrated LGM sigma pillars                            |
| `FX.<pair>.spot`, `FX.<pair>.vol` (e.g. `FX.CLPUSD.spot`)                                                                                                                                    | FX spot and lognormal FX volatility per `FxModelConfig` |
| `<credit_index>.pillar_<i>`                                                                                                                                                                  | survival pillars of a bootstrapped credit curve         |
| `funding_spread.<date>` or `<funding_index>.<date>`                                                                                                                                          | funding spread term structure                           |

Values are aggregated across all netting sets in the run. To obtain per-set sensitivities run the engine once per netting set.

## Example

```rust,ignore
let result = engine.run(&mut netting_sets)?;
let mut sens = result.sensitivities.unwrap_or_default();
sens.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap_or(std::cmp::Ordering::Equal));
for (label, value) in sens.iter().take(10) {
    println!("{label:<55} {value:>12.4}");
}
```

Typical top rows for the `examples/cva` portfolio are the long-dated SOFR OIS quotes (through both the exposure and the system discounting), the cross-currency basis quotes and `FX.CLPUSD.vol`.

## Notes

- The deterministic system-curve discounting step (\\(P(0,t_k)\\) from the domestic curve) is not differentiated, so curve sensitivities exclude that term.
- Path noise is common to value and gradient: since sensitivities come from the same paths, they are consistent with the reported XVA (no bump-noise), but they still carry Monte Carlo error that decreases with `n_paths`.
- Sensitivities to `lambda`, `rho` and constant `sigma`/`fx_vol` configuration values are available where those values are leaves (`FX.<pair>.vol`); `lambda` and `rho` are treated as constants.
- Validate by rerunning with a `Scenario` on the base quotes (see [Scenarios](../risk/scenarios.md)) and the same `seed`.

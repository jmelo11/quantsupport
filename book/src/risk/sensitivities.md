# Market Quote Sensitivities

Request sensitivities alongside value:

```rust,ignore
let result = pricer.evaluate(
    &trade,
    &[Request::Value, Request::Sensitivities],
    &context,
)?;

if let Some(risk) = result.sensitivities() {
    for (pillar, value) in risk.instrument_keys().iter().zip(risk.exposure()) {
        println!("{pillar}: {value:.6}");
    }
}
```

`SensitivityMap` aligns `instrument_keys()` with `exposure()`. Keys identify market calibration pillars, such as deposit, OIS, basis, or volatility quotes.

The raw derivative is generally \(\partial V/\partial q\). Apply reporting scales explicitly: multiply a rate derivative by `0.0001` for PV01/DV01 per basis point, or a volatility derivative by `0.01` for value per volatility point. Do not apply a universal scale to mixed quote types.

Curve dependencies matter. A forecast curve calibrated using another discount curve can produce risk on both sets of pillars. Group reports by curve or surface without merging equal tenors from different market objects.

Run `cargo run -p sensitivity` for a multi-curve quote-level risk report.

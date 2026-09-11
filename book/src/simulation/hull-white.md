# Hull-White

The one-factor Hull-White model represents the short rate with mean reversion and time-dependent volatility. It fits the initial term structure by construction and supports rates-option pricing and simulation.

`HullWhite` can be created directly or calibrated with `ModelCalibrationConfiguration`. Calibration consumes caplet or other configured option quotes and produces `HullWhiteCalibrationQuality` records.

```rust,ignore
let model = HullWhite::new(mean_reversion, &curve);
let calibrated = model.calibrate_with_configuration(
    &configuration,
    &constructed,
    &quotes,
    &curve,
    Level::Mid,
)?;
```

Always inspect calibration quality, not just solver success. Compare model and market prices or implied volatilities by expiry. Mean reversion may be fixed while a piecewise volatility term structure is calibrated.

Use the same calibrated model consistently for closed-form swaption/cap pricing and path generation. Run `cargo run -p hullwhite` for calibration diagnostics, option pricing, simulation, and optional plots.

# Volatility Surfaces

Volatility surfaces map expiry and strike coordinates to implied volatility. Cubes add an underlying tenor dimension, commonly required by swaptions.

`VolatilitySurfaceConfiguration` and `VolatilityCubeConfiguration` define quote selection, indexing, and volatility conventions. Builders consume these configurations and a `QuoteStore`:

```rust,ignore
let surfaces = VolatilitySurfaceBuilder::new(surface_configs)
    .build(&quotes, Level::Mid)?;
let cubes = VolatilityCubeBuilder::new(cube_configs)
    .build(&quotes, Level::Mid)?;
```

`VolatilityType` distinguishes Black and normal quoting. `SmileType` describes strike, moneyness, or log-moneyness coordinates. FX surfaces may require orientation through `OrientedFxVolSurface` so domestic/foreign conventions match the instrument.

Pricing models consume volatility through sources such as `ConstantVolatility`, `SurfaceTermVolatility`, `CubeTermVolatility`, and `PiecewiseConstantVolatility`. `ModelCalibrationConfiguration` can instead derive model volatility from market instruments.

Run `cargo run -p volatilitysurface` to build and query an interpolated SOFR caplet surface.

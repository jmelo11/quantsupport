# Configuration

Market construction is driven by Serde-compatible configuration types. The same JSON schemas are consumed by Rust and Python.

| Configuration                    | Purpose                                              |
| -------------------------------- | ---------------------------------------------------- |
| `CurveConfiguration`             | Discount/forecast curve instruments and dependencies |
| `CreditCurveConfiguration`       | Survival/default curve construction                  |
| `VolatilitySurfaceConfiguration` | Expiry/strike volatility surfaces                    |
| `VolatilityCubeConfiguration`    | Expiry/tenor/strike volatility cubes                 |
| `ModelConfiguration`             | Model family and calibration source                  |
| `SimulationConfiguration`        | Paths, seed, time grid, and model setup              |
| `XvaEngineConfig`                | Portfolio exposure model and simulation controls     |

A production initialization sequence is:

```rust,ignore
let mut context = PricingContext::new()
    .with_quote_store(quotes)
    .with_fixing_store(fixings)
    .with_fx_store(fx)
    .with_base_currency(Currency::USD)
    .with_base_index(MarketIndex::SOFR)
    .with_curve_configurations(curves)
    .with_credit_curve_configurations(credit_curves)
    .with_volatility_surface_configurations(surfaces)
    .with_volatility_cube_configurations(cubes)
    .with_simulation_configurations(simulations);
context.initialize()?;
```

Treat configuration and market data as a versioned pair. Validate identifiers, reference dates, currencies, indices, quote conventions, dependency cycles, and required fixings before valuation. The JSON files under `examples/*/data/` are the canonical working templates.

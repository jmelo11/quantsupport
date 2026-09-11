# LGM

The Linear Gaussian Model (LGM) supplies components for rates, FX, and equity simulation. QuantSupport exposes `LgmRateModel`, `LgmFxModel`, `LgmEquityModel`, and the coordinating `LgmMarketModel`.

```rust,ignore
let market_model = LgmMarketModel::new(
    Currency::USD,
    MarketIndex::SOFR,
    reference_date,
    DayCounter::Actual365,
)
.with_n_paths(10_000)
.with_seed(42);
```

The market model combines factors needed by the portfolio and generates coherent states across simulation dates. Rate models reference their curves; FX and equity models add spot and correlation dependencies.

LGM is the primary model family used by the exposure and XVA examples. Configuration types make the model graph serializable and reproducible. Calibrate model parameters to relevant market instruments, validate factor correlations, and ensure all currencies have appropriate discounting and FX conversion paths.

See `examples/pfe` for direct exposure simulation and `examples/cva` for the higher-level XVA engine.

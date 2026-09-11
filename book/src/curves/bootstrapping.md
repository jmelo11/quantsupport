# Curve Bootstrapping

Bootstrapping solves curve nodes so calibration instruments reproduce their selected market quotes. `CurveConfiguration` describes each curve; `MultiCurveBootstrapper` resolves dependencies between configurations.

```rust,ignore
let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
let curves = MultiCurveBootstrapper::new(configurations, policy)
    .bootstrap(&quote_store, Level::Mid)?;
```

Configurations are serializable, so the recommended production path is:

1. Load a `QuoteStore` with a coherent reference date.
2. Deserialize curve configurations from JSON.
3. Define the collateral discount policy.
4. Bootstrap at `Level::Mid`, `Bid`, or `Ask`.
5. Inspect pillars and calibration residuals before pricing.

Quote-pillar labels are retained in differentiable curve nodes. A subsequent sensitivity request therefore reports risk against calibration instruments rather than opaque zero-rate nodes.

Run `cargo run -p bootstrap` for a complete JSON-driven USD/CLP setup with dependent curves.

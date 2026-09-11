# Pricing Context

`PricingContext` owns the inputs and constructed market graph required during evaluation. It can be assembled from already-built elements or from serializable configuration.

```rust,ignore
let mut context = PricingContext::new()
    .with_quote_store(quotes)
    .with_fixing_store(fixings)
    .with_fx_store(fx)
    .with_base_currency(Currency::USD)
    .with_base_index(MarketIndex::SOFR)
    .with_curve_configurations(curve_configs)
    .with_volatility_surface_configurations(surface_configs)
    .with_volatility_cube_configurations(cube_configs)
    .with_simulation_configurations(simulation_configs);

context.initialize()?;
```

Initialization applies scenarios to quotes, then constructs discount and credit curves, volatility surfaces and cubes, and model-driven simulations. The resulting `ConstructedElementStore` resolves requests from pricers and simulation models.

The base currency and base index define default discounting. Product-specific collateral policies and cross-currency configurations can override how curves are selected.

Reuse an initialized context for a coherent batch of valuations. Reinitialize after changing observable data or scenarios so dependent objects are rebuilt. In Python, the context manager performs initialization and tape lifecycle management automatically.

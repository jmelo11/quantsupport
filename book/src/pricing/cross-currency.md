# Cross-Currency Swaps

Cross-currency swaps exchange cashflows in two currencies. QuantSupport supports fixed-floating and floating-floating structures through `MakeFixFloatCrossCurrencySwap` and `MakeFloatFloatCrossCurrencySwap`.

Valuation requires:

- forecast curves for each floating index;
- discount curves consistent with collateral treatment;
- an `FxStore` spot rate for currency conversion;
- historical fixings for already-reset coupons;
- explicit initial and final notional exchange conventions.

```rust,ignore
let xccy = MakeFloatFloatCrossCurrencySwap::<DualFwd>::default()
    // set identifiers, dates, currencies, indices, notionals, and schedules
    .build()?;
```

The NPV is reported in the pricing context's base currency. FX and rate dependencies remain differentiable, so quote sensitivities can include both domestic and foreign curve pillars.

Cross-currency bootstrapping is collateral-sensitive: changing the collateral currency changes implied discounting. Keep collateral assumptions in configuration and verify FX quote orientation. The `sensitivity`, `pfe`, and `cva` examples contain complete cross-currency trades.

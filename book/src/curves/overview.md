# Yield Curves

Yield curves provide discount factors and forward rates. QuantSupport represents them through term-structure traits, allowing pricers to work with flat curves, interpolated bootstrapped curves, and differentiable curves through the same interface.

```rust,ignore
let curve = FlatForwardTermStructure::new(
    today,
    DualFwd::from(0.04),
    RateDefinition::new(
        DayCounter::Actual360,
        Compounding::Continuous,
        Frequency::Annual,
    ),
);

let discount = curve.discount_factor(today + Period::from_str("5Y")?)?;
```

A `RateDefinition` controls day count and compounding. Do not compare rates without normalizing their definitions; equal numeric rates under simple and continuous compounding do not imply equal discount factors.

`DiscountCurveElement` associates a curve with a `MarketIndex`. The `ConstructedElementStore` uses that index to satisfy discount and forward requests. Bootstrapped curves carry pillar labels, which become sensitivity keys when their nodes use AD scalars.

Use flat curves for controlled tests. Use [Curve Bootstrapping](bootstrapping.md) for market valuation.

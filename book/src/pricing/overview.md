# Pricing Overview

Pricing is request-driven. A pricer receives a trade, requested measures, and a `PricingContext`, then returns `EvaluationResults`.

```rust,ignore
let requests = [
    Request::Value,
    Request::FairRate,
    Request::Cashflows,
    Request::Sensitivities,
];
let results = pricer.evaluate(&trade, &requests, &context)?;
```

Common requests include value, cashflows, fair rate, yield to maturity, modified duration, and sensitivities. Support depends on the pricer and instrument. Unrequested or unsupported outputs remain absent.

Pricer families include discounted cashflow valuation, Black closed forms, Hull-White closed forms, Monte Carlo equity valuation, FX pricing, rate futures, and CDS pricing. The `Evaluator` example demonstrates type-erased dispatch when a heterogeneous portfolio cannot use one concrete generic pricer type.

Always align the instrument scalar and market scalar. Use `DualFwd` when the result must retain market derivatives; use plain scalars for calculations that do not require risk.

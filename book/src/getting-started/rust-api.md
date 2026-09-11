# Rust API

Most applications begin with the prelude:

```rust
use quantsupport::prelude::*;
```

It re-exports the main dates, currencies, indices, instruments, market stores, configurations, pricers, models, simulations, and XVA types. More specialized items remain available through their module paths.

## The common evaluation shape

Rust pricing follows four steps:

1. Construct a `Trade` and its underlying `Instrument`.
2. Build or initialize a `PricingContext`.
3. Choose a pricer implementing `Pricer`.
4. Call `evaluate` with a slice of `Request` values.

```rust,ignore
let requests = [Request::Value, Request::Cashflows, Request::Sensitivities];
let result = pricer.evaluate(&trade, &requests, &context)?;

if let Some(value) = result.price() { println!("{value}"); }
if let Some(risk) = result.sensitivities() { println!("{risk:?}"); }
```

`EvaluationResults` is request-driven: fields that were not requested may be absent. Handle optional outputs instead of assuming every pricer supplies every measure.

## Numeric types

Many instruments are generic over their scalar. Use `f64` for plain calculations and `DualFwd` when quote-level automatic differentiation is required. Keep the scalar type consistent across instruments, curves, constructed elements, and pricers.

Library errors use `quantsupport::prelude::Result<T>`. Applications can convert `QSError` into their own error type with standard Rust error handling.

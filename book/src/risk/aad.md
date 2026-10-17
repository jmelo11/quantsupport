# Differentiation and market sensitivities

A useful risk report identifies both the derivative and the independent input.
QuantSupport names market pillars so that a trade's numerical dependencies
can be expressed as quote exposures rather than anonymous curve-node
derivatives.

For a decimal rate quote \\(q\\), the reported row is
\\(\partial V/\partial q\\). Its first-order +1 bp change is
\\(10^{-4}\partial V/\partial q\\). A volatility quote, FX spot or spread
anchor has different units. A number called “exposure” is not automatically a
DV01, vega per percentage point, or a historical stress loss.

## Forward and reverse representations

`Fwd<T>` stores a value and a first-order seed of the inner scalar type.
Nesting supplies higher orders: `Fwd1 = Fwd<f64>` and
`Fwd2 = Fwd<Fwd<f64>>`. `Dual<T>` records reverse operations;
`DualFwd = Dual<Fwd2>` is the standard pricing scalar.

```rust
# extern crate quantsupport;
use quantsupport::prelude::*;

let x = Fwd2::var(1.5);
let y = x * x;
assert!((y.value() - 2.25).abs() < 1e-12);
assert!((y.first_derivative() - 3.0).abs() < 1e-12);
assert!((y.second_derivative() - 2.0).abs() < 1e-12);
```

Reverse mode starts from a selected output and accumulates adjoints toward
independent leaves. A constant has no independent market meaning merely
because it is stored in an AD type.

```rust
# extern crate quantsupport;
use quantsupport::ad::expr::FloatExt;
use quantsupport::prelude::*;

Tape::start_recording_fwd();
let x = DualFwd::new(0.04);
let y: DualFwd = (x * DualFwd::scalar(2.0)).exp().into();
y.backward()?;
assert!((x.adjoint()?.value() - 2.0 * 0.08_f64.exp()).abs() < 1e-12);
Tape::stop_recording_fwd();
# Ok::<(), QSError>(())
```

The tape is thread-local. Marks separate persistent market dependencies from
temporary trade/path nodes. Pricers register required pillars, calculate
values, propagate adjoints and report them under labels; script/XVA services
use path-by-path rewind and worker-local markets. `PricingContext::initialize`
is a construction operation, not the owner of all these tape lifetimes.

## Calibration dependencies

For calibrated rate curves, the nonlinear solve is numerical `f64` work.
Finite-difference residual Jacobians and the implicit-function theorem
reconnect the converged discount factors to market quotes. This is a local
first-order construction, not exact differentiation of every Newton iteration
or a guarantee of exact calibration curvature.

The resulting ladder can include parent curves. A Term SOFR swap discounted
on SOFR may expose basis quotes and OIS quotes because both define the market
used by valuation. Model calibration has its own omissions: current Gaussian
sigma Jacobians estimate independent-fit volatility response by diagonal
forward differences, not joint schedule-calibration risk or curve/strike-induced
recalibration. Materialized simulation paths omit generation dependencies
entirely.

`SensitivityMap` stores parallel labels and values. Aggregation sums repeated
labels reached through several paths. Reporting is pricer-specific: do not
assume every implementation removes zero rows or includes every conceivable
model parameter. Bond Z-spread/OAS anchors, for example, appear as explicitly
independent spread factors rather than being re-solved under every curve move.

## What a derivative does not establish

AD differentiates the active implemented program. Branches use their selected
local behavior; hard digitals have zero pathwise derivative almost everywhere.
Scripting can smooth comparisons, introducing a deliberate approximation.
Numerical integration, finite-difference calibration and time/state grids
retain their errors.

Reverse work often avoids a separate pricing run for every market input, but
there is no universal fixed runtime multiplier. Memory, calibration graph,
trade count and path count matter. Validation should compare selected labeled
derivatives against same-input, same-seed
[full revaluation](scenarios.md), with a bump size small enough to test local
behavior and large enough to avoid numerical cancellation.

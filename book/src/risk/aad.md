# Differentiation and market sensitivities

Automatic differentiation computes a valuation's local response to its
inputs. QuantSupport registers those inputs under market pillar labels, so a
risk report can identify the deposit, swap, volatility quote or other quantity
whose change affects the trade.

For a calibrated curve, the derivative includes the connection between market
quotes and solved curve nodes. The label and units of each input determine
how to translate the derivative into a monetary change.

For a decimal rate quote \\(q\\), the reported row is \\(\partial V/\partial
q\\). Its first-order +1 bp change is \\(10^{-4}\partial V/\partial q\\). A
volatility quote, FX spot or spread anchor has different units. Apply the
scaling appropriate to each input when reporting DV01, vega or other market-move
estimates.

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
registered independent inputs. In the example below, `x` is a tracked
variable and the exponential's derivative is propagated back to it.

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
temporary trade/path nodes. Pricers register required pillars, calculate values
and propagate adjoints before reporting labelled derivatives. Script and XVA
services rewind path-local nodes and use worker-local markets. These valuation
services manage recording separately from context initialization.

## Calibration dependencies

For calibrated rate curves, the nonlinear solve is numerical `f64` work.
Finite-difference residual Jacobians and the implicit-function theorem reconnect
the converged discount factors to market quotes. The resulting connection
approximates the local first-order calibration response; higher-order
calibration effects require separate validation.

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

## Validation and numerical accuracy

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

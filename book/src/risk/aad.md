# Automatic Differentiation

All sensitivities in quantsupport are computed by algorithmic differentiation, not bumping. The implementation lives in `src/ad/`.

## Scalar types

| Type                    | Mode                                    | Use                                                                                              |
| ----------------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `f64`                   | none                                    | fastest pricing, no risk                                                                         |
| `Fwd1..Fwd4` (`Fwd<N>`) | forward, N-th order tangents            | second-order Greeks, tests                                                                       |
| `Dual<T>`               | reverse (tape) over an inner scalar `T` | full curve sensitivities                                                                         |
| `DualFwd = Dual<Fwd2>`  | reverse over forward                    | the default AD type: exact first derivatives to every quote and second-order information for IFT |
| `ADForward = Fwd2`      |                                         | alias used by curve code                                                                         |

Every pricer, curve and instrument is generic over `T: Scalar`; `DualFwd::scalar(x)`, `DualFwd::zero()`, `DualFwd::one()`, `DualFwd::from(x)` create constants.

## Tape

`Tape` is a thread-local recorder. Operations on `Dual` values push nodes only while recording:

```rust,ignore
Tape::start_recording_fwd();
let x = DualFwd::new(0.04);          // leaf (recorded)
let c = DualFwd::scalar(2.0);        // constant (not recorded)
let y = (x * c).exp();
y.backward();                        // reverse sweep from y
let dy_dx = x.adjoint()?;            // 2·exp(0.08)
Tape::stop_recording_fwd();
```

| API                                                                               | Purpose                                                                                 |
| --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `Tape::start_recording_fwd()` / `stop_recording_fwd()` / `is_active()`            | control recording (`start_recording` etc. for `Dual<f64>`)                              |
| `Tape::set_mark_fwd()` / `rewind_to_mark_fwd()`                                   | keep the market-data part of the tape and discard trade-level nodes between evaluations |
| `Tape::rewind_to_init_fwd()`, `propagate_mark_to_start_fwd()`, `reset_mark_fwd()` | full reset / propagate adjoints from mark to start                                      |
| `Dual::new(f64)`                                                                  | leaf variable; `constant(f64)` non-differentiable                                       |
| `value()`, `inner()`, `adjoint() -> Result<T>`                                    | read primal / inner forward value / gradient                                            |
| `backward()`, `backward_to_mark()`, `backward_mark_to_start()`                    | reverse sweeps over different tape ranges                                               |
| `put_on_tape()`, `ensure_on_tape()`, `is_on_tape()`                               | register a value created off-tape                                                       |

`PricingContext::initialize()` starts recording, bootstraps curves and surfaces (quotes become leaves), then sets a mark. Each `evaluate` call records the pricing nodes after the mark, runs `backward_to_mark()` and `propagate_mark_to_start_fwd()` to reach the quote leaves, reads their adjoints, and rewinds to the mark so the next trade starts from a clean tape. This is what makes portfolio-wide sensitivities cost roughly one extra pricing per trade.

## Curves and pillars

`curve.put_pillars_on_tape()` marks pillar discount factors as leaves; `curve.pillars() -> Option<Vec<(String, DualFwd)>>` returns them labelled with the quote identifier. The bootstrapper uses the implicit function theorem to convert pillar adjoints into quote adjoints (see [Curve Bootstrapping](../curves/bootstrapping.md)), so the labels in `SensitivityMap` are the original quotes (`OIS_USD_SOFR_5Y`), not internal pillars.

## Forward mode

`Fwd<N>` carries the value and up to N tangents:

```rust,ignore
let x = Fwd2::var(1.5);               // seed tangent 1
let y = x * x;
y.value();              // 2.25
y.first_derivative();   // 3.0
y.second_derivative();  // 2.0
```

`Fwd::constant(x)` has zero tangents. Inside `DualFwd`, the forward component propagates through the reverse sweep, which is how the bootstrapper obtains the Jacobian needed for the IFT without a second pass.

## Costs and caveats

- Recording allocates: keep `Tape::start_recording_fwd()` scoped and rewind between trades.
- Functions with branches (`max`, `if`) are differentiated along the taken branch; digital payoffs need smoothing (see the scripting `FuzzyEvaluator`).
- Sensitivities are exact derivatives of the implemented formulas, so bisection solvers in Hull-White pricers are differentiated via IFT at the converged root.

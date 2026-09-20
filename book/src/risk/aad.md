# Automatic Differentiation

QuantSupport computes market sensitivities through algorithmic differentiation. The calculation records how prices depend on market inputs and then propagates derivatives through that recorded graph. This chapter explains the scalar types, the reverse-mode tape, the treatment of calibrated curves, and the operational costs of the approach. The implementation lives in `src/ad/`.

## Scalar types

Different derivative questions benefit from different scalar representations. Ordinary `f64` values support pure valuation, forward types carry a small number of derivative directions, and reverse types report sensitivities to many market inputs in one sweep. The available aliases are:

| Type                    | Mode                                    | Use                                                                                              |
| ----------------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `f64`                   | none                                    | fastest pricing, no risk                                                                         |
| `Fwd1..Fwd4` (`Fwd<N>`) | forward, N-th order tangents            | second-order Greeks, tests                                                                       |
| `Dual<T>`               | reverse (tape) over an inner scalar `T` | full curve sensitivities                                                                         |
| `DualFwd = Dual<Fwd2>`  | reverse over forward                    | the default AD type: exact first derivatives to every quote and second-order information for IFT |
| `ADForward = Fwd2`      |                                         | alias used by curve code                                                                         |

Pricers, curves, and generic instruments operate on `T: Scalar`, so the financial formulas remain shared across these modes. `DualFwd::scalar(x)`, `DualFwd::zero()`, `DualFwd::one()`, and `DualFwd::from(x)` create constants. `DualFwd::new(x)` creates an independent variable when the tape is recording.

## Tape

`Tape` is a thread-local recorder that owns the reverse-mode computation graph. Operations on `Dual` values add nodes during an active recording scope. This compact example creates one market leaf, calculates a function, and reads its derivative:

```rust,ignore
Tape::start_recording_fwd();
let x = DualFwd::new(0.04);          // leaf (recorded)
let c = DualFwd::scalar(2.0);        // unrecorded constant
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
| `Dual::new(f64)`                                                                  | leaf variable. `constant(f64)` creates a fixed value                                     |
| `value()`, `inner()`, `adjoint() -> Result<T>`                                    | read primal / inner forward value / gradient                                            |
| `backward()`, `backward_to_mark()`, `backward_mark_to_start()`                    | reverse sweeps over different tape ranges                                               |
| `put_on_tape()`, `ensure_on_tape()`, `is_on_tape()`                               | register a value created off-tape                                                       |

The table separates graph lifecycle operations from scalar operations. Marks allow an application to retain the market-construction graph across many trades and discard each trade's temporary nodes after valuation.

`PricingContext::initialize()` starts recording, bootstraps curves and surfaces so their quotes become leaves, and sets a mark. Each `evaluate` call records pricing nodes after that mark. It runs `backward_to_mark()` and `propagate_mark_to_start_fwd()` to reach the quote leaves, reads their adjoints, and rewinds to the mark for the next trade. Portfolio-wide sensitivities therefore require roughly one additional reverse calculation per trade.

## Curves and pillars

Curves expose named risk factors through the `Pillars` interface. `curve.put_pillars_on_tape()` registers the differentiable values, and `curve.pillars()` returns them with labels. For a hand-built curve, those values can be its discount-factor nodes. For a bootstrapped curve, the implicit function theorem maps node derivatives to the original calibration quotes, as described in [Curve Bootstrapping](../curves/bootstrapping.md). `SensitivityMap` can therefore report a market label such as `OIS_USD_SOFR_5Y`.

## Forward mode

`Fwd<N>` carries a value and up to `N` derivative coefficients. It is useful when the number of directions is small or higher-order information is required. The example below recovers the first two derivatives of \\(x^2\\):

```rust,ignore
let x = Fwd2::var(1.5);               // seed tangent 1
let y = x * x;
y.value();              // 2.25
y.first_derivative();   // 3.0
y.second_derivative();  // 2.0
```

`Fwd::constant(x)` has zero tangents. Inside `DualFwd`, the forward component propagates through the reverse sweep. The bootstrapper uses this nested representation to obtain the Jacobian needed for implicit differentiation within the same derivative framework.

## Costs and caveats

Automatic differentiation gives exact derivatives of the implemented numerical program. Its resource use and treatment of nonsmooth functions follow directly from that program:

- Recording allocates: keep `Tape::start_recording_fwd()` scoped and rewind between trades.
- Functions with branches such as `max` and `if` are differentiated along the selected branch. Digital payoffs use smoothing through the scripting `FuzzyEvaluator`.
- Sensitivities are exact derivatives of the implemented formulas, so bisection solvers in Hull-White pricers are differentiated via IFT at the converged root.

## What to remember

The tape records one connected path from observable quotes through calibration and pricing to the requested result. Marks make that path reusable across a portfolio, and pillar labels preserve its market interpretation. Forward-over-reverse scalars also provide the higher-order information needed by calibration routines without changing the financial code.

# Differentiation and market sensitivities

Automatic differentiation (AD) measures how a value changes when its inputs
change. QuantSupport labels each market input with its quote identifier. Each
reported sensitivity therefore names the deposit, swap, volatility quote, or
other observation it refers to. For calibrated curves and models, the risk is
mapped from the calibrated parameters back to the market quotes.

For how recording, the reverse chain rule, Cranelift code generation, and code
reuse work, see
[Inside automatic differentiation and the JIT](../implementation/ad-jit.md).

A sensitivity is in the units of its input. If a rate quote \\(q\\) is an
annual decimal rate, the reported value is \\(\partial V/\partial q\\). The
first-order effect of a one-basis-point rise is then
\\(10^{-4}\partial V/\partial q\\). Volatility, FX, and spread sensitivities
are likewise in the units of their inputs.

## Requesting risk from a pricer

Use the same pricing method for values and sensitivities:

- `Request::Value` asks for the price.
- `Request::Sensitivities` asks for first derivatives with respect to the
  market inputs the pricer registers.

The pricer handles recording and running the derivative code. You do not
start a recording or handle intermediate derivatives.

The following fragment assumes that a trade, market provider, and pricer
already exist:

```rust,ignore
let results = pricer.evaluate(
    &trade,
    &[Request::Value, Request::Sensitivities],
    &market,
)?;
let price = results.price();
let sensitivities = results.sensitivities();
```

Most differentiable instruments and curves use `Recorded`, which is in
`quantsupport::prelude`. Outside a recording, a `Recorded` value is just a
number stored in a contract or market object. During valuation, the pricer
registers the labelled market pillars as inputs and records the calculations
that depend on them. The same pricing formulas then give both the value and
the code for its derivatives.

Cranelift compiles the first-derivative code to machine code. No second
derivatives are computed.

- A value-only request does not run the derivative code. Building a calibrated
  market may still need derivatives.
- Each pricer decides which inputs it reports. A model assumption is not
  reported as a market factor just because the arithmetic is differentiable.

## Recording, compiling and executing a calculation

The AD types are in `quantsupport::ad`:

- `Recorded` is the scalar type that records operations.
- `Graph` stores a recorded calculation.
- `CompiledKernel` holds the generated machine code.

Use them when you have your own calculation that you want to run many times
with different inputs. There are three steps:

1. **Record.** `Graph::trace` or `Graph::try_trace` runs a Rust closure with
   recorded inputs and stores its operations. The sample input values are used
   during recording. The graph keeps input slots, so you can change the values
   later.
2. **Compile.** `compile` generates code for the value and the first
   derivatives.
3. **Run.** `value_and_grad` runs the code on the current inputs and writes the
   derivatives into a buffer you provide. It does not call your closure again.

This example prices a discounted Black call with a fixed strike and expiry. Its
four inputs are spot, the continuously compounded rate, lognormal volatility,
and the continuously compounded dividend yield:

```rust
# extern crate quantsupport;
use quantsupport::prelude::*;
use quantsupport::models::utils::black_call_scalar;

let inputs = [100.0, 0.03, 0.20, 0.01];
let graph = Graph::try_trace(&inputs, |x| {
    let discount = (-x[1]).exp();
    let forward = x[0] * (-x[3]).exp() / discount;
    black_call_scalar(forward, 100.0, x[2], 1.0)
        .map(|undiscounted| undiscounted * discount)
})?;
let kernel = graph.compile()?;
let mut gradient = [0.0; 4];
let price = kernel.value_and_grad(&inputs, &mut gradient)?;
assert!((price - 8.827321225352).abs() < 1e-10);

let changed_market = [105.0, 0.04, 0.25, 0.02];
let changed_price = kernel.value_and_grad(&changed_market, &mut gradient)?;
assert!(changed_price.is_finite());
assert_eq!(kernel.derivative_order(), DerivativeOrder::First);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The gradient entries are in input order and in the units of each input.

- You can reuse the kernel with new inputs, as long as the formula and contract
  structure stay the same.
- A different schedule, strike convention, or embedded constant may need a new
  graph.
- `Graph::try_trace` passes on errors from a formula that can fail. Use
  `Graph::trace` when the closure cannot fail.

Recorded values belong to their recording. Use them only inside the closure,
and keep the resulting graph or kernel if you want to reuse it.

### Numerical choices and Rust control flow

A Rust comparison gives a `bool` at recording time, but a later input could
need the other branch.

- The simple `Graph` interface rejects comparisons that depend on inputs, and
  rejects reading the plain value of a recorded number.
- To choose between results at run time, use the scalar functions `max_val`,
  `min_val`, or `select_greater`.

`CompiledProgram` supports ordinary Rust comparisons with a guard. It records
the result of each comparison and checks it again on every run. If a result
changes, the run returns `CompileError::ComparisonChanged`, and you should
record the calculation again. These checks use the standard `PartialOrd` and
`PartialEq` traits.

Derivative conventions at non-smooth points:

- Ties in max and min, and `abs` at zero, have zero derivative.
- Selection conditions have zero derivative.
- The recorded normal CDF uses its numerical value with the exact Gaussian
  derivative, including at zero.

These matter when a payoff is evaluated exactly at a kink.

### Requesting second derivatives

A kernel compiled with `DerivativeOrder::First` (the default) gives the value
and the gradient. Use `DerivativeOrder::Second` if you also need
Hessian-vector products or a full Hessian. This compiles exact second-derivative
code; it does not bump inputs. First-derivative calls on the kernel still run
only the first-order code.

```rust
# extern crate quantsupport;
use quantsupport::prelude::*;
let inputs = [2.0, 3.0];
let graph = Graph::trace(&inputs, |x| x[0] * x[0] + x[0] * x[1])?;
let kernel = graph.compile_with_order(DerivativeOrder::Second)?;
let mut gradient = [0.0; 2];
let mut hessian = [0.0; 4];
kernel.value_grad_hessian(&inputs, &mut gradient, &mut hessian)?;
assert_eq!(hessian, [2.0, 1.0, 1.0, 0.0]);
let mut product = [0.0; 2];
kernel.hessian_vector_product(&inputs, &[0.0, 1.0], &mut product)?;
assert_eq!(product, [1.0, 0.0]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The full Hessian is stored row by row. It needs memory proportional to the
square of the number of inputs and one extra pass per input. For many market
factors, a Hessian-vector product is often cheaper.

Pricing and XVA sensitivity requests report first-order risk only. The kernel
interface above does not give second-order risk through calibration.

## Reusing calculations in simulations

The cost of AD depends on how you use it:

- **Pricers** record the current calculation on each call and then look for
  matching compiled code. This keeps results correct when inputs or branches
  change. For small calculations, recording and compiling can dominate the
  run time.
- **`XvaEngine::run`** compiles a program for all simulated paths, for
  supported workloads. Call `prepare` to pay this cost before `run`. Each run
  then takes the random draws and current model coefficients, computes fresh
  exposures and adjustments, and maps the gradients back to the market labels.
  Identical market requests share results, while each claim keeps its own
  payoff.

When market quotes change, call `update_market`. The next `prepare` refreshes
the model coefficients and their sensitivity to quotes, and reuses the compiled
code when it still matches. A new contract or collateral agreement may need a
new program. Always update the market after quotes change; reusing compiled
code does not refresh market values.

The parallel scripting API also prepares supported model and script layouts.
Other layouts record each path and run its compiled gradient, sharing a common
setup. Each worker thread has its own buffers and shares the compiled code.
`CompiledKernel::sum_paths` returns the sum of values and gradients over paths,
not the average. Divide by the number of paths, or apply whatever normalization
your estimator needs.

Stored simulation paths give less risk. The numbers on them were generated with
fixed inputs. The stored-path equity pricer therefore reports discounting and
FX risk only, while a scripted model valuation can differentiate the model
inputs used to simulate. Before comparing speed, check that both calculations
return the same outputs and the same risk. The
[benchmark runners](../../../tools/README.md#benchmarks) time recording,
compilation, market refresh, and repeated runs separately.

## Calibration dependencies

Calibration links solved parameters to market quotes. `CompiledFunction`
generates values, Jacobians, and optional output Hessians from one set of
residual equations. `ImplicitDerivatives` then uses the residual derivatives to
find how calibrated states and downstream values respond to quotes. Full
second-order risk would also need the curvature of the calibration itself; a
stored first-order map cannot provide it.

How each area computes its derivatives:

| Area | Derivative calculation |
| --- | --- |
| Credit calibration | Compiled partial derivatives of the CDS equations plus implicit differentiation, including tested curvature of observations. |
| Vector numerical equations | `CompiledFunction` generates native Jacobians and optional output Hessians from one residual definition. |
| Rate-bootstrap Newton steps | The residual Jacobian uses central differences. |
| Parent-curve bootstrap links | The link to parent pillars uses differences of rebuilt curves. |
| Hull–White quote maps | The sigma response to volatility is estimated with differences. |
| Curve-risk reprojection | The zero-rate basis uses compiled AD; the instrument basis uses differences of rebuilt curves. |
| Pricing and XVA risk | First derivatives only; no full second-order calibrated risk. |

Calibrated rate curves are solved with plain `f64` numbers. A first-order
calibration map then links the solved nodes back to market quotes. This link
can include parent curves. For example, a Term SOFR swap discounted on SOFR
can show risk to both basis and OIS quotes.

Gaussian model calibration has more limits. Its sigma Jacobians measure the
response of separate single-option fits. They do not capture a joint fit of the
schedule, or refitting caused by moves in curves or strikes.

A `SensitivityMap` stores labels and their derivatives. If a label is reached
through several dependencies, its contributions are added. Each pricer decides
what it reports. Check the pricer's documentation for zero rows, model
parameters, and separately reported factors such as a bond's Z-spread or OAS.

## Changing the curve risk basis

`PricingContext` has two methods that return a `RiskContext`, which implements
`MarketDataProvider`. Pass it to the same pricer and request
`Request::Sensitivities` as usual. Both methods keep the original curve values
and interpolation, so prices, including between nodes, do not change.

The complete example is
[`risk-reprojection.rs`](../../../examples/sensitivity/src/bin/risk-reprojection.rs).
Run `cargo run -p sensitivity --bin risk-reprojection` from the repository root
to compare deposit, OIS, and zero-rate sensitivities for the same swap. The
example prints the NPV in each basis, and its test checks that the price is
unchanged.

This fragment reports sensitivities to zero rates implied by the bootstrapped
discount factors:

```rust,ignore
let convention = RateDefinition::new(
    DayCounter::Actual365,
    Compounding::Continuous,
    Frequency::Annual,
);
let zero_context = context.with_zero_rate_risk(&[
    ZeroRateRisk::new(MarketIndex::SOFR, convention),
    ZeroRateRisk::new(MarketIndex::TermSOFR3m, convention),
])?;
let result = pricer.evaluate(
    &trade,
    &[Request::Value, Request::Sensitivities],
    &zero_context,
)?;
```

- By default, the grid is the curve's future nodes. Use
  `.with_dates(vec![date_1, date_2, ...])` for another grid. Curves without
  nodes, such as composites, need explicit dates.
- Labels include the curve, date, and rate convention.
- Exposures are derivatives per unit of annual decimal zero rate. Multiply by
  `1e-4` for a first-order +1 bp change.
- Quotes of parent curves of the selected curves are included in the
  transformation. Quotes of other curves keep their original labels.

This fragment reports curve risk against a different set of calibration
instruments:

```rust,ignore
let instruments: Vec<QuoteDetails> = [
    "OIS_USD_SOFR_1Y",
    "OIS_USD_SOFR_2Y",
    "Swap_USD_TermSOFR3m_1Y",
    "Swap_USD_TermSOFR3m_2Y",
].iter().map(|label| label.parse()).collect::<Result<_>>()?;
let projected_context = context.reproject_risk(&instruments)?;
let result = pricer.evaluate(&trade, &[Request::Sensitivities], &projected_context)?;
```

- The target instruments do not need quotes in the quote store. Their implied
  quotes are solved from the original curves.
- `target_labels()` and `target_values()` show the targets and their implied
  quotes.
- The targets must cover all discount and forward curve pillars in the context.
  The example assumes two pillars on each of its two curves.
- Instrument reprojection needs curves built from configuration. Curves you
  supplied directly support zero-rate risk only.

How it works: for target quotes `x` and original quotes `q`, reprojection
computes `J = dx/dq` and reports `dV/dx = dV/dq * J⁺`.

- Zero-rate Jacobians use AD. Instrument Jacobians use central differences of
  rebuilt curves.
- A basis that is too small or singular is an error.
- With more targets than source quotes, the pseudoinverse picks the
  exposures with the smallest Euclidean norm, in the units of the quotes.
- The transformation is local and first order. Create a new view when market
  inputs change.
- FX spots, volatilities, and other factors are held fixed in the
  transformation and keep their usual risk.
- It does not add risk that the original pricer leaves out, such as model
  recalibration or simulation paths.

## Validation and numerical accuracy

AD differentiates the code that actually runs.

- At a branch, the derivative is that of the branch taken. A hard digital has
  zero pathwise derivative almost everywhere.
- Scripting can smooth comparisons, which is an approximation.
- Numerical integration, finite-difference calibration, and time or state grids
  keep their own errors.

Reverse-mode AD often avoids a separate pricing run for each market input, but
the speed-up is not a fixed ratio. It depends on memory, the calibration graph,
and the number of trades and paths.

To validate, compare selected derivatives with
[full revaluation](scenarios.md) using the same inputs and the same random
seed. Choose a bump small enough to test local behavior and large enough to
avoid rounding error.

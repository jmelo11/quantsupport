# Differentiation and market sensitivities

Automatic differentiation measures how a valuation changes when its inputs
change. QuantSupport gives market inputs their quote labels, so each reported
sensitivity identifies the deposit, swap, volatility quote or other observation
that affects the price. When a curve or model is calibrated, the reported risk
also depends on the map from its market quotes to its calibrated parameters.

For the recording data structures, reverse chain rule, Cranelift code generation
and native-code reuse, read
[Inside automatic differentiation and the JIT](../implementation/ad-jit.md).

A sensitivity is expressed in the units of its input. For example, if a rate
quote \\(q\\) is an annual decimal rate, its reported sensitivity is
\\(\partial V/\partial q\\). A first-order estimate of the effect of a one-basis-point
increase is therefore \\(10^{-4}\partial V/\partial q\\). Use the corresponding
input units when interpreting volatility, FX and spread sensitivities.

## Requesting risk from a pricer

Applications use the same pricing method for values and sensitivities. A
`Request::Value` asks for the price, while `Request::Sensitivities` also asks
for its first derivatives with respect to the registered market inputs. The
pricer owns the capture and derivative execution; an application does not
start a recording or manage intermediate derivatives itself.

The following fragment assumes that a trade, market provider and pricer have
already been constructed:

```rust,ignore
let results = pricer.evaluate(
    &trade,
    &[Request::Value, Request::Sensitivities],
    &market,
)?;
let price = results.price();
let sensitivities = results.sensitivities();
```

Most differentiable Rust instruments and curves use `Recorded`, which is
available through `quantsupport::prelude`. Outside a capture, it can retain an
ordinary numerical value in a contract or market object. During valuation,
the pricer registers the labelled market pillars as independent inputs and
records the arithmetic that depends on them. This allows the same pricing
formulas to calculate both a value and the derivative program used for risk.

The first-order derivative program is compiled to machine code by Cranelift.
It contains no Hessian calculation. A value request does not ask the pricer to
run that gradient program, although constructing a calibrated market may itself
require derivatives. The set of inputs reported by each pricer remains part
of that pricer's contract: differentiable arithmetic alone does not make every
model assumption a reported market factor.

## Recording, compiling and executing a calculation

The numerical AD interfaces are defined directly in `quantsupport::ad`.
`Recorded` supplies the scalar operations, `Graph` stores a scalar calculation,
and `CompiledKernel` owns the generated machine code. These interfaces are
useful when an application has a numerical calculation that it wants to reuse
with several sets of inputs.

There are three stages. First, `Graph::trace` or `Graph::try_trace` runs a Rust
closure with recorded scalar inputs and captures its numerical instructions.
The sample values provide the values needed during that capture; the graph
stores input slots so that those values can change later. Second, `compile`
generates the value and first-gradient code. Finally, `value_and_grad` loads the
current inputs into that code and writes the derivatives to a caller-owned
buffer. It does not call the original Rust closure again.

For example, this calculation prices a discounted Black call with a fixed
strike and expiry. Its four runtime inputs are the spot, continuously
compounded rate, lognormal volatility and continuously compounded dividend
yield:

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

The gradient entries follow the input order and retain the inputs' numerical
units. Because the changed market uses the same formula and contract structure,
it can reuse the kernel. A different schedule, strike convention or embedded
constant may require a new graph. `Graph::try_trace` propagates errors returned
by a fallible formula; use `Graph::trace` when the closure is infallible.

A captured scalar belongs to its capture. Keep those expression values inside
the closure, and keep the resulting graph or kernel for reuse. Internally,
`RecordingScope` suspends an enclosing capture during isolated market work and
restores it when that work returns or unwinds. This prevents one valuation's
expression identities from leaking into another.

### Numerical choices and Rust control flow

A Rust comparison produces a Boolean value while the closure is being
captured. The simple `Graph` interface rejects input-dependent Rust comparisons
and extraction of recorded primal values, because a later input could select a
different calculation. Use scalar `max_val`, `min_val` or `select_greater` when a
payoff must select a numerical arm during execution.

`CompiledProgram` supports a guarded form of ordinary Rust comparisons. It
retains the observed comparison and checks its operands on every execution.
If that decision changes, execution returns `CompileError::ComparisonChanged`
so that the caller can capture the appropriate calculation again. It does not
silently reuse the previously selected branch. These checks use the ordinary
`PartialOrd` and `PartialEq` traits.

Max/min ties and the derivative of `abs` at zero follow the library's
zero-derivative convention. Selection predicates have zero derivatives. The
recorded normal CDF uses the numerical CDF value with analytic Gaussian
derivatives, including at zero. These local conventions matter when a payoff
is evaluated at a nondifferentiable point.

### Requesting second derivatives

A kernel compiled with `DerivativeOrder::First`, which is the default, provides
its value and first gradient. Select `DerivativeOrder::Second` when the
calculation also needs Hessian-vector products or a dense Hessian. This adds a
compiled derivative program; it does not estimate curvature by bumping inputs.
First-order calls on that kernel still invoke only its first-order function.

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

The dense Hessian is stored in row-major order. It requires quadratic result
storage and one directional evaluation per input, so a Hessian-vector product
is often preferable for a large market-factor set. Public pricing and XVA
sensitivity requests currently report first-order risk; the kernel interface
above does not imply complete second-order risk through every calibration map.

## Reusing calculations in simulations

Repeated pricing calls and prepared simulations have different cost boundaries.
Ordinary pricers capture the current calculation before looking for compatible
native derivative code. This keeps the current inputs and control flow valid,
but it can make compilation and capture significant for small calculations.

Supported `XvaEngine::run` workloads instead prepare a whole numerical path
program in the existing engine. `prepare` can pay that cost explicitly before
`run`. Each execution then supplies the indexed random draws and current model
coefficients, evaluates fresh exposures and adjustments, and maps the coefficient
gradients back to the published market labels. Identical market-request blocks
share response slots, while claims keep their separate identities and payoffs.

When market quotes change, use `update_market` to refresh the engine's market
snapshots and calibration. Its next preparation refreshes the coefficients and
their quote Jacobian; a compatible program can retain its machine code. A
changed contract or collateral agreement can require a different program.
Code reuse therefore does not justify using a stale numerical market binding.

The parallel scripting API also prepares supported model and script layouts.
Other control-flow layouts capture each path and execute its native gradient,
while retaining a shared setup prefix. Each worker uses its own mutable
buffers, and compiled code can be shared across workers. `CompiledKernel::sum_paths`
returns unnormalized value and first-gradient sums; the caller chooses the
normalization required by its estimator.

Materialized simulation paths have a narrower risk boundary. They contain
numbers whose generation inputs are held fixed. The stored-path equity pricer
therefore reports discounting and FX risk, whereas a scripted model valuation
can differentiate its simulated model inputs. Compare performance only after
checking that the requested outputs and risk boundaries are the same. The
[benchmark runners](../../../tools/README.md#benchmarks) record capture, compilation,
market refresh and repeated execution separately.

## Calibration dependencies

A curve or model calibration connects solved parameters to its market quotes.
`CompiledFunction` generates values, Jacobians and optional output Hessians from
shared residual equations. `ImplicitDerivatives` then uses those residual
partials to obtain the local response of the calibrated states and downstream
observations. Complete second-order risk also requires the calibration's own
curvature; a stored first-order map alone cannot provide it.

The current coverage is:

| Area | Derivative calculation |
| --- | --- |
| Credit calibration | The shared CDS equations use compiled partial derivatives and implicit differentiation, including tested observation curvature. |
| Vector numerical equations | `CompiledFunction` generates native Jacobians and optional output Hessians from one residual definition. |
| Rate-bootstrap Newton steps | The solver still forms its residual Jacobian with central differences. |
| Parent-curve bootstrap links | The connection to source pillars still uses reconstruction differences. |
| Hull–White quote maps | The calibration still estimates sigma/volatility response with differences. |
| Curve-risk reprojection | The zero-rate basis uses compiled AD; instrument reconstruction still uses differences. |
| Public pricing and XVA risk | The requests report first derivatives; they do not yet expose complete second-order calibrated risk. |

For calibrated rate curves, the nonlinear solve uses numerical `f64` values.
Its local first-order calibration map reconnects the solved nodes to market
quotes. That connection can include parent curves: for example, a Term SOFR
swap discounted on SOFR can expose both basis and OIS quotes. Gaussian model
calibration has further boundaries: its current sigma Jacobians estimate the
independent-fit volatility response, rather than joint schedule-calibration
risk or full curve/strike-induced recalibration.

A `SensitivityMap` stores matching labels and derivative values. Aggregation
adds repeated labels reached through several dependencies. Reporting remains
pricer-specific, so consult the selected pricer when checking zero rows, model
parameters and independently reported factors such as a bond's Z-spread or OAS.

## Changing the curve risk basis

`PricingContext` provides two transformations that return a `RiskContext`
implementing `MarketDataProvider`. Pass the resulting view to the same pricer
and request `Request::Sensitivities` as usual. Both transformations preserve the
original curve values and interpolation, including prices between nodes.

The complete runnable example is
[`risk-reprojection.rs`](../../../examples/sensitivity/src/bin/risk-reprojection.rs).
Run `cargo run -p sensitivity --bin risk-reprojection` from the repository root
to compare deposit, OIS and post-bootstrap zero-rate sensitivities for the same
swap. The example displays the NPV differences between the three bases; its
inline regression test checks that both transformations preserve the exact price.

This fragment reports sensitivities to zero rates implied from bootstrapped
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

Each grid defaults to that curve's future nodes. Add
`.with_dates(vec![date_1, date_2, ...])` to select another grid; curves without
nodes, such as composites, require explicit dates. Labels include the curve,
date and rate convention. Exposures are derivatives per unit of annual decimal
zero rate; multiply by `1e-4` for a first-order +1 bp change. Selected curves'
parent quote dependencies enter the transformation jointly. Other curve quotes
retain their original coordinates.

This fragment reports curve risk against another set of calibration instruments:

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

The target contracts need not have observations in the quote store. Their
implied quotes are solved against the original curves. The view exposes
`target_labels()` and `target_values()` for inspection. The target basket must
span all native discount/forward curve pillars in the context; the example
assumes two pillars on each of its two curves. Instrument reprojection requires
configuration-driven curves. Supplied runtime curves support zero-rate risk.

For target quotes `x` and original curve quotes `q`, reprojection computes
`J = dx/dq` and reports `dV/dx = dV/dq * J⁺`. Zero-rate Jacobians use AD;
instrument Jacobians use central differences of reconstructed source curves.
Insufficient or singular bases return an error. With more target factors than
source factors, the pseudoinverse chooses minimum Euclidean-norm exposures in
the supplied quote units. These transformations are local and first order;
construct a fresh view when market inputs change. FX spots, volatility and
other independent factors are held fixed in the curve basis calculation and
keep their native reported risk. This does not add model recalibration or
simulation-path dependencies omitted by the original pricer.

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

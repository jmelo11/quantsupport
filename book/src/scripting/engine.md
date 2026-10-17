# Script valuation and claim integration

`ScriptEngine` turns dated payoff code into an analyzed program and evaluates
it over model-backed paths. Its preparation separates semantic reasoning from
the repeated numerical work: parsing is done once, observations are declared
before simulation, and AD memory is bounded by rewinding temporary path nodes.

## Preparing the program

Construction receives an `EventStream`, reference date, local currency and
local discount index. Variable indexing assigns storage slots and payment
identities. Market calls become per-event `SimulationDataRequest` values and
flattened model requests with observation dates. Conditional transformation,
branch analysis and domain processing prepare the evaluator and remove
statically impossible branches.

The program's horizon includes requested observations and payments, not only
source-event dates. This is important for a fixing event that declares a much
later settlement. The model must be configured for the complete requested
timeline.

The following fragment assumes an existing event stream and differentiable
market model:

```rust,ignore
let engine = ScriptEngine::new(
    events, reference_date, Currency::USD, MarketIndex::SOFR,
)?;
let values = engine.evaluate(&mut model, Some("swap"))?;
let value = values.get("swap");
```

The selected variable identifies the result whose weighted adjoints are
propagated; returned averaged variables are not limited to that one name.
Without conditionals the engine uses exact expression evaluation. Conditional
programs use the smoothed evaluator described in the language chapter.

## Pathwise AD and reporting

Curve pillars and model inputs must be registered on the current tape before
evaluation. For each path the engine rewinds to its mark, generates requested
responses, evaluates the program and accumulates its contribution. It
propagates the selected weighted result back to pre-mark dependencies before
discarding path-local nodes. After the path loop, adjoints reach the registered
leaves.

This bounds temporary tape memory to one path rather than all paths at once.
Parallel evaluation constructs a model and tape per worker through
`ScriptModelSetup`, returning partial values, labeled sensitivities and
cashflows. The total path count determines normalization. Reproducible path
indices do not promise bitwise identical floating-point reductions across
arbitrary thread layouts.

`evaluate_with_cashflows` additionally collects executed payments. Each
`ExpectedCashflow` distinguishes mean undiscounted amount in payment currency
from mean discounted, numeraire-deflated present value in local currency.
These are raw sampled payments.

## Martingale controls and reconciliation

For a selected result, at least 16 paths and eligible discount/forward requests,
the engine estimates martingale control coefficients on a separate pilot
sample of up to 64 paths. Known initial-curve expectations anchor discounted
bond/forward controls. The main estimator is schematically

\\[
\widehat V_{\rm controlled}
=\frac1n\sum_p\left[Y_p-
\widehat\beta^\mathsf T(C_p-E[C])\right].
\\]

The fitted coefficients are held fixed during differentiation. This reduces
variance but does not eliminate all Monte Carlo error or turn a nonlinear
payoff into a deterministic one.

Only the selected result variable receives this adjustment. Expected cashflow
present values remain unadjusted sampled payments. Consequently their sum need
not equal the reported controlled result at finite path count. Equality also
requires that the selected variable actually represents the total payment
accumulator rather than another state quantity. Deterministic one-path
comparisons and particular regression assertions must not be generalized to
all stochastic script outputs.

## From payments to exposure claims

`ScriptedProduct::contingent_claims` creates one claim per payment identity.
Each uses unit notional and `LongReceive`, because scale and direction already
belong to the script expression. Claims share one compiled engine.

At an exposure date, a scripted claim replays the required program against its
response block and returns that payment's value. The live-claim schedule
controls which payments remain. Required discount, rate, FX and spot requests
must all be supported by the market model; the claim bridge does not create
missing dynamics.

These claims can share a netting set with native products. This gives bespoke
and standard payments a common exposure interface, but it is not a general
optimal-stopping engine for arbitrary scripted callability. A program that
describes an exercise rule is responsible for what that rule means.

The Rust examples compare native and scripted swaps under controlled matching
setups. Python's `evaluate_script` is a narrower averaged-variable service,
described in [Working in Python](../python/getting-started.md); it does not
expose every cashflow, path or claim-bridge facility available in Rust.

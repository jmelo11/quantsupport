# Scripting Overview

The `quantsupport::scripting` module describes a payoff as a dated sequence of small programs. This form is useful for bespoke products whose cashflows depend on path observations and evolving state. Scripts are parsed once, analyzed before simulation, and evaluated over paths produced by a `MarketModel<DualFwd>`. The current examples use the LGM market model.

Script evaluation shares the automatic-differentiation tape used throughout the library. One payoff definition can therefore produce NPV, quote-level sensitivities, expected cashflows, and contingent claims for XVA. This chapter introduces that end-to-end workflow and points to the detailed language and runtime chapters.

The scripting API is re-exported from the prelude. The following imports show its four main groups: dated source events, runtime evaluation, parallel execution, and XVA integration.

```rust
use quantsupport::prelude::{
    CodedEvent, Event, EventStream,      // dated scripts
    ScriptEngine, ScriptModelSetup,      // evaluation
    ScriptModelCallback, ParallelScriptEvaluation, ExpectedCashflow,
    ScriptedProduct,                     // XVA integration
    SimulationDataRequest, ScriptingError, ScriptValue,
};
```

`CodedEvent` and `EventStream` represent source and parsed events. `ScriptEngine` coordinates analysis and valuation. `ScriptedProduct` converts the same event stream into claims that the XVA engine can schedule.

## Pipeline

The pipeline turns source text into a validated runtime before it generates any paths. That early analysis discovers variables and market requests, which gives model setup and error reporting a complete view of the payoff. The stages are:

```text
Vec<CodedEvent>  ──TryFrom──▶  EventStream (parsed AST per event)
                                    │
                                    ▼
                          ScriptEngine::new(events, ref_date, ccy, discount_index)
                                    │  VarIndexer        → variable slots + SimulationDataRequest per event
                                    │  IfConditionTransform / IfProcessor / DomainProcessor
                                    ▼
      evaluate(&mut model, Some("swap"))            → HashMap<String, f64>
      evaluate_with_cashflows(&mut model, ..)       → (values, Vec<ExpectedCashflow>)
      evaluate_parallel(&setup, Some("swap"))       → ParallelScriptEvaluation
                                    │
                                    ▼
      ScriptedProduct::new(..).contingent_claims()  → Vec<ContingentClaim> for XvaEngine
```

The parsed event stream is the shared representation in the center of the flow. Direct valuation sends it to `ScriptEngine`, and XVA conversion sends it to `ScriptedProduct`. Both paths therefore use the same dates, expressions, and payment definitions.

The implementation under `src/scripting/` assigns each stage to a focused module:

| Path                                                                       | Responsibility                                                                                                                              |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `parsing/lexer.rs`, `parsing/parser.rs`                                    | Tokenizer and recursive-descent parser producing `Node` trees                                                                               |
| `nodes/node.rs`                                                            | The `Node` enum (arithmetic, comparison, `If`, `ForEach`, `Pays`, `Spot`, `Df`, `RateIndex`, …) and per-node metadata used by the analysers |
| `nodes/event.rs`                                                           | `CodedEvent` (date + source), `Event` (date + AST), `EventStream`                                                                           |
| `visitors/varindexer.rs`                                                   | Assigns variable slots, collects `SimulationDataRequest`s                                                                                   |
| `visitors/ifconditiontransform.rs`, `ifprocessor.rs`, `domainprocessor.rs` | Static passes preparing conditionals for smoothing and nested-if variable stores                                                            |
| `visitors/evaluator.rs`                                                    | `SingleScenarioEvaluator`: exact path evaluation                                                                                            |
| `visitors/fuzzyevaluator.rs`                                               | `FuzzyEvaluator`: smoothed conditionals for stable AAD on digital payoffs                                                                   |
| `request.rs`                                                               | `SimulationDataRequest` (discounts, forwards, FX, spots, numeraire flag)                                                                    |
| `runtime.rs`                                                               | `ScriptEngine`, `ScriptModelSetup`, `ParallelScriptEvaluation`, `ExpectedCashflow`                                                          |
| `product.rs`                                                               | `ScriptedProduct`, `ScriptedPayoff` and the `IntoContingentClaims` bridge to XVA                                                            |
| `utils/errors.rs`                                                          | `ScriptingError`                                                                                                                            |

The parser and visitors establish the meaning of the program, and `runtime.rs` executes that prepared representation. The numeric type inside scripts is `NumericType = DualFwd`, so every script variable remains differentiable with respect to curve pillars and recorded model parameters.

## A complete example

The `scripting-examples` package gives a controlled comparison using a one-year receive-fixed SOFR swap. It prices the contractual swap with `MakeSwap` and `DiscountedCashflowPricer`, then expresses the same four coupon periods as scripted events. The first event initializes `swap` and `fixed_rate`. Each event calculates its accrual fraction, requests the applicable forward rate, and records a payment:

```text
swap = 0; fixed_rate = 0.035;
accrual = cvg("2025-01-01", "2025-04-01", "Actual360");
floating_rate = RateIndex("SOFR", "2025-01-01", "2025-04-01");
swap pays 10000000 * (fixed_rate - floating_rate) * accrual on "2025-04-01";
```

The valuation driver in `examples/scripting/src/bin/valuation.rs` uses an LGM model with zero volatility. Under that deterministic setup, the native and scripted cashflows have an exact comparison:

```rust,ignore
Tape::start_recording_fwd();
curve.put_pillars_on_tape();
let rate_model = LgmRateModel::new(DualFwd::scalar(0.03), DualFwd::zero(), &curve);
let mut model = LgmMarketModel::new(Currency::USD, MarketIndex::SOFR, reference_date(), DayCounter::Actual360)
    .with_n_paths(1)
    .with_seed(42);
model.add_curve_model(MarketIndex::SOFR, rate_model);

let script = ScriptEngine::new(scripted_swap_events()?, reference_date(), Currency::USD, MarketIndex::SOFR)?;
let results = script.evaluate(&mut model, Some("swap"))?;
let npv = results["swap"];
// d(NPV)/d(pillar) is now available through pillar.adjoint() for every curve pillar.
Tape::stop_recording_fwd();
```

The package exposes separate commands for direct valuation and XVA:

```bash
cargo run -p scripting-examples --bin valuation   # NPV + pillar sensitivities vs native swap
cargo run -p scripting-examples --bin xva         # EPE profile + CVA/FVA sensitivities vs native swap
```

Both binaries assert agreement with the native implementation to `1e-8` for NPV and EPE and to `1e-6` for sensitivities. These checks demonstrate that scripting participates in the same market and risk graph as native products.

## When to use scripting

Scripting is most useful when payoff flexibility and rapid product iteration are the primary requirements:

- Structured coupons, digitals, range accruals, autocallables, and other bespoke payoffs.
- Products whose term sheet changes frequently: the script is data (a `Vec<CodedEvent>` is `Serialize`/`Deserialize`), so it can be stored and versioned alongside market data.
- Producing an XVA exposure profile from the payment events already present in a bespoke payoff.

Native instruments provide specialized closed forms for products such as Black caplets, Garman–Kohlhagen FX options, and Hull–White swaptions. They also provide product-specific outputs such as `Request::FairRate` and standardized cashflow tables. Scripting provides a general path-based route for products defined most naturally as dated payoff logic.

## What to remember

A scripted product begins as dated source text and becomes a parsed, analyzed event stream. The runtime derives its market requests, evaluates it over differentiable model paths, and records expected payments. The same event stream can then enter XVA through contingent claims, giving bespoke and native products a common exposure workflow.

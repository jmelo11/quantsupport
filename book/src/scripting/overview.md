# Scripting Overview

The `quantsupport::scripting` module lets you describe a payoff as a dated sequence of small scripts instead of implementing a new `Instrument` and pricer in Rust. Scripts are parsed once, statically analysed, and then evaluated over Monte Carlo paths produced by any `MarketModel<DualFwd>` (in practice the LGM market model). Because evaluation runs on the same AD tape as the rest of the library, a scripted payoff yields **NPV, per-pillar sensitivities, and expected cashflows** without any extra code, and it can enter the XVA engine as a set of ordinary contingent claims.

Everything you need is re-exported from the prelude:

```rust
use quantsupport::prelude::{
    CodedEvent, Event, EventStream,      // dated scripts
    ScriptEngine, ScriptModelSetup,      // evaluation
    ScriptModelCallback, ParallelScriptEvaluation, ExpectedCashflow,
    ScriptedProduct,                     // XVA integration
    SimulationDataRequest, ScriptingError, ScriptValue,
};
```

## Pipeline

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

Module layout (`src/scripting/`):

| Path | Responsibility |
| --- | --- |
| `parsing/lexer.rs`, `parsing/parser.rs` | Tokenizer and recursive-descent parser producing `Node` trees |
| `nodes/node.rs` | The `Node` enum (arithmetic, comparison, `If`, `ForEach`, `Pays`, `Spot`, `Df`, `RateIndex`, …) and per-node metadata used by the analysers |
| `nodes/event.rs` | `CodedEvent` (date + source), `Event` (date + AST), `EventStream` |
| `visitors/varindexer.rs` | Assigns variable slots, collects `SimulationDataRequest`s |
| `visitors/ifconditiontransform.rs`, `ifprocessor.rs`, `domainprocessor.rs` | Static passes preparing conditionals for smoothing and nested-if variable stores |
| `visitors/evaluator.rs` | `SingleScenarioEvaluator`: exact path evaluation |
| `visitors/fuzzyevaluator.rs` | `FuzzyEvaluator`: smoothed conditionals for stable AAD on digital payoffs |
| `request.rs` | `SimulationDataRequest` (discounts, forwards, FX, spots, numeraire flag) |
| `runtime.rs` | `ScriptEngine`, `ScriptModelSetup`, `ParallelScriptEvaluation`, `ExpectedCashflow` |
| `product.rs` | `ScriptedProduct`, `ScriptedPayoff` and the `IntoContingentClaims` bridge to XVA |
| `utils/errors.rs` | `ScriptingError` |

The numeric type used inside scripts is `NumericType = DualFwd`, so every script variable is differentiable with respect to curve pillars and model parameters that were put on the tape before evaluation.

## A complete example

The `scripting-examples` package prices a one-year receive-fixed SOFR swap twice: once with `MakeSwap` + `DiscountedCashflowPricer`, once as four scripted events. The script for each accrual period (`examples/scripting/src/lib.rs`) is:

```text
swap = 0; fixed_rate = 0.035;                                   # first event only
accrual = cvg("2025-01-01", "2025-04-01", "Actual360");
floating_rate = RateIndex("SOFR", "2025-01-01", "2025-04-01");
swap pays 10000000 * (fixed_rate - floating_rate) * accrual on "2025-04-01";
```

and the driver (`examples/scripting/src/bin/valuation.rs`) evaluates it against an LGM model with zero volatility so the comparison is exact:

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

Run it with:

```bash
cargo run -p scripting-examples --bin valuation   # NPV + pillar sensitivities vs native swap
cargo run -p scripting-examples --bin xva         # EPE profile + CVA/FVA sensitivities vs native swap
```

Both binaries assert agreement with the native implementation to `1e-8` (NPV, EPE) and `1e-6` (sensitivities).

## When to use scripting

- Structured coupons, digitals, range accruals, autocallables, and other payoffs that are not worth a dedicated Rust instrument.
- Products whose term sheet changes frequently: the script is data (a `Vec<CodedEvent>` is `Serialize`/`Deserialize`), so it can be stored and versioned alongside market data.
- Getting an XVA exposure profile for a bespoke product without writing a claim decomposition.

Prefer native instruments and pricers when a closed form exists (Black caplets, Garman–Kohlhagen FX options, Hull–White swaptions) or when you need `Request::FairRate`, `YieldToMaturity`, or cashflow tables in the `EvaluationResults` format.

# Script Engine

`ScriptEngine` (`src/scripting/runtime.rs`) turns an `EventStream` into an executable program, derives the market data it needs, and evaluates it over Monte Carlo paths while keeping the AD tape footprint bounded.

## Construction

```rust,ignore
pub fn new(
    events: EventStream,
    reference_date: Date,
    local_currency: Currency,
    local_discount_index: MarketIndex,
) -> Result<ScriptEngine, ScriptingError>
```

`new` performs the whole static analysis pipeline:

1. **Validation** – non-empty, no event before `reference_date`, events sorted by date.
2. **`VarIndexer`** – assigns a slot to each variable, numbers every `pays`, `Df`, `RateIndex` and `Spot` node, and produces one `SimulationDataRequest` per event holding the `DiscountRequest`s, `ForwardRateRequest`s, `FxRequest`s and `SpotRequest`s that event needs. Default discounting uses `local_discount_index`; default currency is `local_currency`.
3. **Request flattening** – the per-event requests are flattened into the `Vec<SimulationRequest>` format consumed by `MarketModel::set_requests`, along with the observation date of each request and a map back to the script nodes.
4. **`IfConditionTransform`** – rewrites every comparison into the canonical `(lhs - rhs) > 0` form used by the smoothing evaluator.
5. **`IfProcessor`** – computes the maximum `if` nesting depth (`max_nested_ifs`) and the set of variables written inside each branch.
6. **`DomainProcessor`** – propagates value domains through the tree so constants are folded and impossible branches are dropped.

### Accessors

| Method | Returns |
| --- | --- |
| `events() -> &EventStream` | The indexed and transformed stream |
| `requests() -> &[SimulationDataRequest]` | One request bundle per event (`dfs()`, `fwds()`, `fxs()`, `spots()`, `requires_numeraire()`) |
| `model_requests() -> &[SimulationRequest]` | Flattened requests for a `MarketModel` |
| `reference_date() -> Date` | |
| `maturity() -> Date` | Latest of all event dates and all requested observation/payment dates |
| `has_variable(&str) -> bool` | Whether the script defines a variable, useful to validate a `result_variable` before running |
| `local_currency() -> Currency` | |

## Evaluation on a single tape

```rust,ignore
pub fn evaluate(
    &self,
    model: &mut dyn MarketModel<DualFwd>,
    result_variable: Option<&str>,
) -> Result<HashMap<String, f64>>

pub fn evaluate_with_cashflows(
    &self,
    model: &mut dyn MarketModel<DualFwd>,
    result_variable: Option<&str>,
) -> Result<(HashMap<String, f64>, Vec<ExpectedCashflow>)>
```

Both methods:

- call `model.set_evaluation_dates(event_dates)` and `model.set_requests(model_requests)` so the model simulates exactly the dates and observables the script uses;
- read the numeraire at every event date and pre-compute control-variate expectations (see below);
- iterate over `model.n_paths()` paths. For each path: `Tape::rewind_to_mark_fwd()`, `model.generate_path(i)`, build a `Scenario` from the responses, run the evaluator, accumulate `value / n_paths` for every numeric variable;
- when `result_variable` is `Some`, back-propagate `result / n_paths` to the tape mark on each path, and after the loop propagate the accumulated adjoints from the mark to the start of the tape.

The result is that any `DualFwd` leaf recorded **before** `evaluate` (curve pillars via `curve.put_pillars_on_tape()`, model parameters created with `DualFwd::scalar`) exposes `d(mean result)/d(leaf)` through `.adjoint()`. Do not run your own `backward()` afterwards. Peak tape memory is one path, independent of `n_paths` (the same mark/rewind pattern as the XVA exposure evaluator).

`evaluate_with_cashflows` additionally captures every executed `pays` and returns path-averaged `ExpectedCashflow`s sorted by date:

```rust,ignore
pub struct ExpectedCashflow {
    pub date: Date,
    pub currency: Currency,   // payment currency (local currency when not named)
    pub amount: f64,          // path-averaged undiscounted amount in `currency`
    pub present_value: f64,   // path-averaged discounted, numeraire-deflated value in local currency
}
```

Summing `present_value` over the vector reproduces the script price, so the vector is a per-date decomposition of the NPV.

### Choice of evaluator

`ScriptEngine` picks the evaluator per scenario:

- `max_nested_ifs == 0` → `SingleScenarioEvaluator`: exact evaluation, no smoothing needed.
- otherwise → `FuzzyEvaluator::new(n_variables, max_nested_ifs)` with automatic comparison scaling, so digital payoffs get finite, stable pathwise sensitivities (see [Script Language](language.md#conditionals-and-smoothing)).

### Control variates

When `result_variable` is given, `n_paths >= 16`, and the script requests any discount factor or forward rate, the engine fits two martingale control coefficients on a pilot set of `min(n_paths, 64)` **extra** paths (indices `n_paths..n_paths+pilot`, so they are disjoint from the reported set): discounted zero-coupon bonds and discounted forward payoffs, whose expectations are known exactly from the curve. The main pass then subtracts `β·(control − E[control])` from the payoff. The betas are treated as constants, so the AAD pass is not differentiated through the regression. This anchors the linear-rate component of the payoff to the curve and is why the scripted swap in the examples matches the analytic swap to `1e-8` with a single path at zero volatility and with tight error at 1 000 paths in the XVA run.

## Parallel evaluation

```rust,ignore
pub trait ScriptModelSetup: Send + Sync {
    fn n_paths(&self) -> usize;
    fn with_model<R>(&self, callback: &mut ScriptModelCallback<'_, R>) -> Result<R>;
}
pub type ScriptModelCallback<'a, R> =
    dyn FnMut(&mut dyn MarketModel<DualFwd>, &[(String, DualFwd)]) -> Result<R> + 'a;

pub fn evaluate_parallel<S: ScriptModelSetup>(
    &self,
    setup: &S,
    result_variable: Option<&str>,
) -> Result<ParallelScriptEvaluation>

pub struct ParallelScriptEvaluation {
    pub values: HashMap<String, f64>,          // path-averaged script variables
    pub sensitivities: Vec<(String, f64)>,     // adjoints of the leaves supplied by `with_model`, sorted by label
    pub cashflows: Vec<ExpectedCashflow>,
}
```

A `DualFwd` holds a pointer into a **thread-local** tape, so a model built on the caller's thread cannot be shared with Rayon workers. `ScriptModelSetup::with_model` is your factory: on each worker it must rebuild the curves and model, put the pillars/parameters you want sensitivities for on that worker's tape, and pass them as labelled `leaves`. The engine then:

1. splits `0..n_paths` into `rayon::current_num_threads()` contiguous ranges;
2. on each worker resets and starts a fresh tape, calls `with_model`, configures the model, evaluates the range, and reads `leaf.adjoint()` for each supplied leaf;
3. sums values, adjoints and cashflows across workers. Normalisation always uses the total path count, so results are independent of the number of threads and deterministic for a fixed seed.

Sketch of a setup:

```rust,ignore
struct SofrSetup { ref_date: Date, dfs: Vec<(Date, f64, String)>, n_paths: usize }

impl ScriptModelSetup for SofrSetup {
    fn n_paths(&self) -> usize { self.n_paths }
    fn with_model<R>(&self, callback: &mut ScriptModelCallback<'_, R>) -> Result<R> {
        let mut curve = DiscountTermStructure::<DualFwd>::new(
            self.dfs.iter().map(|(d, _, _)| *d).collect(),
            self.dfs.iter().map(|(_, df, _)| DualFwd::from(*df)).collect(),
            DayCounter::Actual360, Interpolator::LogLinear, true,
        )?.with_pillar_labels(self.dfs.iter().map(|(_, _, l)| l.clone()).collect())?;
        curve.put_pillars_on_tape();
        let leaves: Vec<(String, DualFwd)> = curve.pillars().unwrap_or_default();

        let rate_model = LgmRateModel::new(DualFwd::scalar(0.05), DualFwd::scalar(0.01), &curve);
        let mut model = LgmMarketModel::new(Currency::USD, MarketIndex::SOFR, self.ref_date, DayCounter::Actual360)
            .with_n_paths(self.n_paths)
            .with_seed(42);
        model.add_curve_model(MarketIndex::SOFR, rate_model);
        callback(&mut model, &leaves)
    }
}

let result = engine.evaluate_parallel(&setup, Some("swap"))?;
println!("NPV = {}", result.values["swap"]);
for (pillar, dv) in &result.sensitivities { println!("{pillar}: {dv}"); }
```

## Errors

`evaluate*` return `ScriptingError::EvaluationError` when the result variable is not defined (`"result variable 'x' is not defined by the script"`), when the model exposes zero paths, when a path cannot be generated, or when a `SimulationResponse` lacks a value the script requested. Model failures are wrapped as `ScriptingError::QuantSupport(QSError)`.

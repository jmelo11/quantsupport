# Exposure Simulation

Exposure is computed by projecting trades into `ContingentClaim`s, evaluating them along `MarketModel` paths, and aggregating the resulting NPV cube. Source: `src/xva/`.

## Contingent claims

```rust,ignore
pub struct ContingentClaim {
    trade_id: String, leg_id: String, idx: usize,
    payment_date: Date, fixing_date: Option<Date>,
    accrual_start: Option<Date>, accrual_end: Option<Date>,
    currency: Currency, foreign_currency: Option<Currency>,
    notional: f64, side: Side,
    evaluation_strategy: ClaimEvaluationStrategy,
    index: Option<MarketIndex>,
    realized_fixing: Option<f64>, partial_fixing: Option<f64>,
}
```

| `ClaimEvaluationStrategy`                                             | Meaning                                                                        |
| --------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `Deterministic { amount }`                                            | fixed coupon / notional exchange                                               |
| `LinearRate { spread, day_counter }`                                  | floating coupon \\(N\\,(L+s)\\,\tau\\)                                         |
| `NonLinearRate { payoff_ops, strike, spread, day_counter }`           | caplet/floorlet-style payoff on the rate                                       |
| `SpotPayoff { payoff_ops, strike, observation_date }`                 | FX/equity option payoff on a spot                                              |
| `PathDependent { observation_dates, aggregator, payoff_ops, strike }` | Asian/lookback-style payoff                                                    |
| `ExerciseContingent { exercise_date, exercise_group, inner }`         | claim alive only if the group is exercised                                     |
| `Scripted { payoff }`                                                 | payoff from a `ScriptedProduct` ([Scripting](../scripting/events-products.md)) |

Trades implement `IntoContingentClaims` (swaps, cross-currency swaps, caps, FX forwards/options, scripted products); `MakeContingentClaim` builds claims by hand. Claims are the common language of the exposure engine, so any trade type reduces to the same evaluation loop.

## Preprocessing

```rust,ignore
let claims = PreprocessorExecutor::new()
    .with_preprocessor(Box::new(FixingPreprocessor::new(reference_date, DayCounter::Actual360, &fixing_store)))
    .with_compression()
    .visit(claims)?;
```

`FixingPreprocessor` fills `realized_fixing` for coupons whose fixing date is in the past; `with_compression()` merges deterministic claims paying on the same date and currency to shrink the cube.

## NPV cube

For each evaluation date \\(t_k\\) on the `frequency` grid and each path \\(p\\), the engine values every claim with payment date after \\(t_k\\) using the path's discount factors and numeraire, converts to the netting-set currency with the simulated FX and stores

\\[
\text{NPV}_{p,k} = \sum_{\text{claims}} \text{side}\cdot\text{payoff}_p\\,\frac{P_p(t_k,T)}{1}.
\\]

```rust,ignore
pub struct NpvCube { trade_id: String, dates: Vec<Date>, npvs: Matrix<f64> /* [path][date] */ }
impl NpvCube {
    pub fn epe(&self) -> Vec<f64>;  // mean(max(NPV,0)) per date
    pub fn ene(&self) -> Vec<f64>;  // mean(min(NPV,0)) per date
    pub fn ee(&self)  -> Vec<f64>;  // mean(NPV) per date
}
```

## Aggregators

| Type                                                                                        | Output                                                                                       |
| ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `PfeAggregator` / `PfeAggregatorFactory`                                                    | quantile of positive exposure per date (e.g. 97.5%)                                          |
| `CvaAggregator { lgd, hazard }`                                                             | \\(\sum*k \text{EPE}\_k\\,\text{LGD}\\,(S(t*{k-1})-S(t_k))\\) with \\(S(t)=e^{-\lambda t}\\) |
| `AggregatorBundle`                                                                          | runs several aggregators over one cube                                                       |
| `CvaFactory`, `DvaFactory`, `FvaFactory`, `CreditCurveCvaFactory`, `FundingCurveFvaFactory` | build aggregators from `CsaTerms` (flat spreads or bootstrapped credit/funding curves)       |

## Running

The high-level entry point is `XvaEngine` ([XVA Overview](../xva/overview.md)); the low-level flow used by `examples/pfe` is:

1. Build claims from trades and preprocess.
2. Build an `LgmMarketModel` (or any `MarketModel`) with `set_evaluation_dates` and the claims' `SimulationRequest`s.
3. Evaluate claims path by path into an `NpvCube`.
4. Apply aggregators.

`cargo run -p pfe` prints the trades' NPVs, then a table of date, EE, EPE and PFE quantile for the netted portfolio. For scripted payoffs the same machinery is reused by `ScriptEngine::evaluate_with_cashflows` and `ExpectedCashflow`, so exotic products can be included in the netting set ([Scripting and XVA](../scripting/xva.md)).

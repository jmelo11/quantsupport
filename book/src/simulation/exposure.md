# Exposure Simulation

Exposure simulation estimates how the value of a portfolio can evolve before its final cashflow. QuantSupport decomposes trades into atomic contingent claims, evaluates those claims along market-model paths, and aggregates the resulting date-by-path values. This chapter explains the claim representation, preprocessing, NPV cubes, exposure measures, and the execution flow implemented under `src/xva/`.

## Contingent claims

A `ContingentClaim` contains the dates, currencies, amount conventions, and evaluation strategy needed to value one future payment. It also carries stable trade and leg identifiers for later aggregation:

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

The `evaluation_strategy` determines how the amount is obtained from deterministic terms or simulated observations:

| `ClaimEvaluationStrategy`                                             | Meaning                                                                        |
| --------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `Deterministic { amount }`                                            | fixed coupon / notional exchange                                               |
| `LinearRate { spread, day_counter }`                                  | floating coupon \\(N\\,(L+s)\\,\tau\\)                                         |
| `NonLinearRate { payoff_ops, strike, spread, day_counter }`           | caplet/floorlet-style payoff on the rate                                       |
| `SpotPayoff { payoff_ops, strike, observation_date }`                 | FX/equity option payoff on a spot                                              |
| `PathDependent { observation_dates, aggregator, payoff_ops, strike }` | Asian/lookback-style payoff                                                    |
| `ExerciseContingent { exercise_date, exercise_group, inner }`         | claim alive only if the group is exercised                                     |
| `Scripted { payoff }`                                                 | payoff from a `ScriptedProduct` ([Scripting](../scripting/events-products.md)) |

Swaps, cross-currency swaps, caps, FX forwards, options, and scripted products implement `IntoContingentClaims`. `MakeContingentClaim` supports direct construction for a custom workflow. Once decomposed, every product enters the same scheduling and path-evaluation loop.

## Preprocessing

Preprocessing resolves information known at the reference date and can compress economically equivalent deterministic payments. A typical chain is:

```rust,ignore
let claims = PreprocessorExecutor::new()
    .with_preprocessor(Box::new(FixingPreprocessor::new(reference_date, DayCounter::Actual360, &fixing_store)))
    .with_compression()
    .visit(claims)?;
```

`FixingPreprocessor` fills `realized_fixing` for coupons whose fixing date has passed. `with_compression()` merges deterministic claims sharing payment date and currency, which reduces the number of entries evaluated and stored. The resulting claim list retains the information needed by simulation.

## NPV cube

For each evaluation date \\(t_k\\) on the configured frequency grid and each path \\(p\\), the engine values the live claims, converts their amounts to the netting-set currency with simulated FX, and applies pathwise discount factors and the numeraire. It stores the sum

\\[
\text{NPV}_{p,k} = \sum_{\text{claims}} \text{side}\cdot\text{payoff}_p\\,\frac{P_p(t_k,T)}{1}.
\\]

`NpvCube` organizes this result with paths as rows and evaluation dates as columns. Its convenience methods calculate three common profiles:

```rust,ignore
pub struct NpvCube { trade_id: String, dates: Vec<Date>, npvs: Matrix<f64> /* [path][date] */ }
impl NpvCube {
    pub fn epe(&self) -> Vec<f64>;  // mean(max(NPV,0)) per date
    pub fn ene(&self) -> Vec<f64>;  // mean(min(NPV,0)) per date
    pub fn ee(&self)  -> Vec<f64>;  // mean(NPV) per date
}
```

Expected positive exposure, or EPE, averages the positive part of NPV. Expected negative exposure, or ENE, averages the negative part. Expected exposure, or EE, averages the signed NPV. These measures feed counterparty, own-credit, funding, and limit calculations.

## Aggregators

Aggregators convert an NPV cube into risk or valuation-adjustment measures. QuantSupport provides profile quantiles and flat or curve-driven XVA factories:

| Type                                                                                        | Output                                                                                       |
| ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `PfeAggregator` / `PfeAggregatorFactory`                                                    | quantile of positive exposure per date (e.g. 97.5%)                                          |
| `CvaAggregator { lgd, hazard }`                                                             | \\(\sum_k \text{EPE}_k\\,\text{LGD}\\,(S(t_{k-1})-S(t_k))\\) with \\(S(t)=e^{-\lambda t}\\) |
| `AggregatorBundle`                                                                          | runs several aggregators over one cube                                                       |
| `CvaFactory`, `DvaFactory`, `FvaFactory`, `CreditCurveCvaFactory`, `FundingCurveFvaFactory` | build aggregators from `CsaTerms` (flat spreads or bootstrapped credit/funding curves)       |

`AggregatorBundle` lets several measures reuse one simulated cube. The factory forms read counterparty and collateral terms from the netting-set configuration, which keeps exposure generation separate from measure-specific integration.

## Running

The high-level entry point is `XvaEngine`, introduced in [XVA Overview](../xva/overview.md). The `pfe` example exposes the underlying flow in four stages:

1. Build claims from trades and preprocess.
2. Build an `LgmMarketModel` (or any `MarketModel`) with `set_evaluation_dates` and the claims' `SimulationRequest`s.
3. Evaluate claims path by path into an `NpvCube`.
4. Apply aggregators.

Running `cargo run -p pfe` prints trade NPVs followed by date, EE, EPE, and PFE for the netted portfolio. Scripted payoffs enter this flow through their contingent claims, as developed in [Scripting and XVA](../scripting/xva.md).

## What to remember

Contingent claims give diverse products one pathwise valuation language. Preprocessing resolves known state and reduces redundant work, `NpvCube` preserves values by path and date, and aggregators translate that cube into exposure profiles and valuation adjustments. Stable trade and leg identities keep the result explainable after netting.

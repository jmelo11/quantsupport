# CVA, DVA and FVA

Aggregators (`src/xva/aggregator.rs`) consume the netted `NpvCube` of a netting set and produce a single number plus AD adjoints. Each implements `name() -> &'static str` (`"CVA"`, `"DVA"`, `"FVA"`).

## Definitions

Let \\(V_k^p\\) be the netted NPV on path \\(p\\) at grid date \\(t_k\\), \\(n\\) the number of paths, \\(P(0,t_k)\\) the system discount factor, and

\\[
\text{EPE}_k=\frac1n\sum_p \max(V_k^p,0),\qquad
\text{ENE}_k=\frac1n\sum_p \min(V_k^p,0).
\\]

| Aggregator                   | Formula                                                                                               | Inputs                                                                  |
| ---------------------------- | ----------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `CvaAggregator { lgd, ... }` | \\(\text{CVA}=\text{LGD}\sum*k P(0,t_k)\\,\text{EPE}\_k\\,[S(t*{k-1})-S(t_k)]\\)                      | counterparty survival \\(S\\); LGD \\(=1-\text{recovery}\\)             |
| `DvaAggregator`              | \\(\text{DVA}=\text{LGD}_{own}\sum_k P(0,t_k)\\,(-\text{ENE}\_k)\\,[S_{own}(t*{k-1})-S*{own}(t_k)]\\) | own survival curve                                                      |
| `FvaAggregator`              | \\(\text{FVA}=\sum_k P(0,t_k)\\,\text{EPE}\_k\\,s_f(t_k)\\,\Delta t_k\\)                              | funding spread \\(s_f\\) (flat, term structure or from a funding curve) |

Survival with a flat spread is \\(S(t)=e^{-\lambda t}\\) with \\(\lambda=\\)`credit_spread`; with `credit_index` it is interpolated from the bootstrapped credit curve pillars (`CreditCurveCvaFactory`). Positive exposure is taken after netting, so the collateral policy and FX conversion applied in the exposure evaluator directly affect these values.

## Factories

`PfeAggregatorFactory` implementations create one aggregator per netting set:

| Factory                  | Fields                                                                                                  |
| ------------------------ | ------------------------------------------------------------------------------------------------------- |
| `CvaFactory`             | `credit_spread`, `recovery`, `n_paths`, `system_dfs`                                                    |
| `CreditCurveCvaFactory`  | `pillar_dates`, `pillar_survivals`, `pillar_labels`, `recovery`, `n_paths`, `day_counter`, `system_dfs` |
| `FvaFactory`             | flat `funding_spread`                                                                                   |
| `FundingCurveFvaFactory` | dated spreads (from `funding_spread_curve` or `funding_index` minus the system curve)                   |
| `DvaFactory`             | own-credit inputs; not wired by `XvaEngine::run`, use it directly with `AggregatorBundle`               |

## Reading results

```rust,ignore
let result = engine.run(&mut netting_sets)?;
for v in result.xva_values.iter().flatten() {
    println!("{:<10} {:<4} {:>14.2}", v.netting_set, v.measure, v.value);
}
```

`cargo run -p cva` output shape:

```text
netting_set  measure  value
CLIENT_A     CVA      12345.67
CLIENT_A     FVA       4567.89
```

## Credit curves for CVA

Bootstrapped from CDS quotes with a `CurveConfiguration` whose `market_index` is `{"Credit": "CLIENT_A"}` and quotes like `Cds_USD_CLIENT_A_1Y`; the bootstrapper solves piecewise-constant hazard rates by bisection (see [Curve Bootstrapping](../curves/bootstrapping.md)). Set `credit_index` in `CsaTerms` to use it, and the CVA sensitivities then include one entry per credit pillar.

## Exposure metrics

`NpvCube::epe()`, `ene()`, `ee()` return per-date vectors; `PfeAggregator` gives the quantile profile used for limit monitoring (`examples/pfe`). These are available in `result.cubes` regardless of CSA fields.

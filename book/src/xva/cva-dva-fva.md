# CVA, DVA and FVA

Exposure profiles become valuation adjustments after they are combined with default, recovery, funding, and discount information. QuantSupport implements this final integration through aggregators in `src/xva/aggregator.rs`. Each aggregator consumes the netted `NpvCube`, returns one measure, and preserves automatic-differentiation adjoints. This chapter defines the measures, explains their input factories, and shows how to read the results.

## Definitions

Let \\(V_k^p\\) be netted NPV on path \\(p\\) at grid date \\(t_k\\), let \\(n\\) be the path count, and let \\(P(0,t_k)\\) be the system discount factor. Expected positive and negative exposure are

\\[
\\text{EPE}_k=\\frac1n\\sum_p \\max(V_k^p,0),\\qquad
\\text{ENE}_k=\\frac1n\\sum_p \\min(V_k^p,0).
\\]

The aggregators apply these profiles to the following discrete-time formulas:

| Aggregator                   | Formula                                                                                                           | Economic inputs                                                        |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `CvaAggregator { lgd, ... }` | \\(\\text{CVA}=\\text{LGD}\\sum_k P(0,t_k)\\,\\text{EPE}_k\\,[S(t_{k-1})-S(t_k)]\\)                                  | counterparty survival \\(S\\) and loss given default                    |
| `DvaAggregator`              | \\(\\text{DVA}=\\text{LGD}_{own}\\sum_k P(0,t_k)\\,(-\\text{ENE}_k)\\,[S_{own}(t_{k-1})-S_{own}(t_k)]\\)             | own survival curve and own loss given default                          |
| `FvaAggregator`              | \\(\\text{FVA}=\\sum_k P(0,t_k)\\,\\text{EPE}_k\\,s_f(t_k)\\,\\Delta t_k\\)                                         | funding spread \\(s_f\\) as a flat value or term structure             |

CVA weights positive exposure by the counterparty default probability over each interval and by the loss incurred at default. DVA applies the corresponding own-credit logic to negative exposure. FVA integrates positive funding needs against the applicable spread.

For a flat credit spread, survival is \\(S(t)=e^{-\\lambda t}\\), where \\(\\lambda\\) is `credit_spread`. When `credit_index` is configured, `CreditCurveCvaFactory` obtains survival probabilities from the bootstrapped credit curve. Exposure is calculated after netting and currency conversion, so the CSA policy directly influences every measure.

## Factories

An aggregator factory creates the measure implementation appropriate for one netting set. Flat factories take scalar assumptions, and curve factories take dated pillars with labels for risk reporting:

| Factory                  | Inputs                                                                                                  |
| ------------------------ | ------------------------------------------------------------------------------------------------------- |
| `CvaFactory`             | `credit_spread`, `recovery`, `n_paths`, and `system_dfs`                                                |
| `CreditCurveCvaFactory`  | survival dates, values, labels, recovery, path count, day counter, and `system_dfs`                     |
| `FvaFactory`             | flat `funding_spread`                                                                                   |
| `FundingCurveFvaFactory` | dated spreads from `funding_spread_curve` or from a funding curve relative to the system curve         |
| `DvaFactory`             | own-credit inputs for direct use with `AggregatorBundle`                                                |

`XvaEngine::run` creates CVA and FVA factories from each set's `CsaTerms`. `DvaFactory` is available through the lower-level `AggregatorBundle` API for workflows that explicitly supply own-credit terms.

## Reading results

`ExposureResult::xva_values` contains the netting-set name, measure name, and scalar value. A caller can display every produced measure as follows:

```rust,ignore
let result = engine.run(&mut netting_sets)?;
for v in result.xva_values.iter().flatten() {
    println!("{:<10} {:<4} {:>14.2}", v.netting_set, v.measure, v.value);
}
```

The `cva` example prints a table with this shape:

```text
netting_set  measure  value
CLIENT_A     CVA      12345.67
CLIENT_A     FVA       4567.89
```

The measure name comes from each aggregator's `name()` implementation. Values use the reporting currency implied by the domestic model and netting-set conversion rules.

## Credit curves for CVA

A credit curve is bootstrapped from CDS quotes under a credit `MarketIndex`, as explained in [Curve Bootstrapping](../curves/bootstrapping.md). Setting `credit_index` in `CsaTerms` directs `CreditCurveCvaFactory` to read its survival pillars. CVA sensitivities then contain one entry per credit quote or pillar label, alongside the rate and model risks generated by exposure.

## Exposure metrics

`NpvCube::epe()`, `ene()`, and `ee()` return per-date profiles. `PfeAggregator` adds a positive-exposure quantile for limit monitoring, as demonstrated by the `pfe` example. These profiles remain available in `result.cubes` and provide the intermediate data used to interpret each scalar adjustment.

## What to remember

CVA, DVA, and FVA are integrations of simulated net exposure against distinct economic inputs. Factories translate CSA fields and constructed curves into those inputs, and aggregators reuse the same NPV cube for each measure. Because the calculations remain differentiable, their sensitivities identify the market quotes, credit assumptions, and funding terms that drive the final adjustment.

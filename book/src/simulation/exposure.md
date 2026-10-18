# Claims, exposure and the XVA engine

Exposure measures the value of a trade or portfolio at future dates. To
calculate it, QuantSupport converts supported positions into scheduled
`ContingentClaim` payments and simulates the market observations needed to
value them. At each exposure date, the remaining payments are valued under
the simulated market state.

The resulting path values can be inspected as exposure profiles or combined
under netting and collateral agreements to calculate valuation adjustments.

## Claim-level valuation

A claim retains payment/fixing/accrual dates, currencies, notional, side and
stable trade/leg identifiers. Its strategy describes deterministic amounts,
rate coupons, optional rates, spot/path-dependent payoffs, exercise-contingent
payments or a scripted payment.

For a native claim, the implemented contribution is

\\[
v_{j,p,k}
=a_{j,p,k}\\,P_{j,p}(t_k,T_j)\\,
X_{j,p,k}\\,s_jN_j.
\\]

The strategy determines the raw amount \\(a\\); discount, currency conversion,
sign and notional supply the remaining factors. A scripted claim obtains its
payment value from the compiled script response block instead. Claim
construction must use the strategy's amount/notional convention consistently
to avoid double scaling.

Before simulation, `FixingPreprocessor` installs realized or partially
realized observations. Compression can merge compatible deterministic claims
to reduce repeated evaluation. The claim converter checks supported contract
features, including overnight observation rules, before these steps.

## Cubes and pointwise profiles

`NpvCube` stores `npvs[path][date]` for a trade. For values \\(V_{p,k}\\),

\\[
EE_k=\frac1n\sum_pV_{p,k},\qquad
EPE_k=\frac1n\sum_p\max(V_{p,k},0),\qquad
ENE_k=\frac1n\sum_p\min(V_{p,k},0).
\\]

ENE is signed and nonpositive. These are **pointwise date profiles**; the
methods do not calculate a time-average EPE or a PFE quantile.
`XvaAggregator` reduces each path into an adjustment contribution, with
implementations for CVA, DVA and FVA. Contributions are normalized by the
total path count and summed across paths.

Returned cubes and these profiles remain raw, pre-margin values. Adjustment
aggregators can consume netted, post-margin exposure instead. A sum of
trade-level EPE is generally not the EPE of the netted sum:

\\[
\max\left(\sum_jV_j,0\right)
\neq\sum_j\max(V_j,0).
\\]

Calculate netted profiles from the summed path values before taking their
positive and negative parts.

## High-level orchestration

`XvaEngine` receives a market provider and `XvaEngineConfig`. Rate factors
identify mean reversion/parameter sources or reuse driver factors; FX
configuration adds foreign currencies, volatilities and supported correlations.
The configured frequency determines the simulation/exposure grid.

At a high level, `run` preprocesses claims, determines required curves and
factors, builds the date grid, prepares system discount factors and per-netting
set factories, then evaluates shared model paths. Foreign currencies and
policy-selected discount identities need corresponding supported models.
The engine reports an error if a required factor is missing.

The following fragment assumes prepared `context`, `configuration` and
`netting_sets`:

```rust,ignore
let engine = XvaEngine::new(&context, configuration)?;
let result = engine.run(&mut netting_sets)?;
for cube in &result.cubes {
    println!("{}: {:?}", cube.trade_id, cube.epe());
}
for adjustment in result.xva_values.iter().flatten() {
    println!("{} {} {}", adjustment.netting_set, adjustment.measure, adjustment.value);
}
```

One run returns trade-level cubes, optional adjustments by netting set and
optional aggregated labeled sensitivities. High-level construction supplies
CVA and FVA factories. DVA exists through lower-level aggregators with
explicit own-credit inputs, not an automatic third output of `run`.

## Numerical and modeling boundaries

Shared paths make netting internally coherent, but finite grids and samples
remain approximations. LGM path construction, calibration targets and
correlations determine the future-state distribution. Optional payoffs may
require numerical conditioning or smoothing. Comparisons with native pricers
need matching payoff, model and payment-selection assumptions, together with
tolerances for numerical and sampling error.

The engine's system discount sequence is stored numerically and held fixed
during AD. Credit/funding factories also introduce particular independent
leaves rather than replay every original calibration. The
[adjustment chapter](../xva/cva-dva-fva.md) explains the resulting derivative
meaning; [collateral](../xva/netting-csa.md) explains which exposures those
adjustments actually consume.

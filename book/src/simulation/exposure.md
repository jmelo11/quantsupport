# Claims, exposure and the XVA engine

Exposure is the value of a trade or portfolio at future dates. To compute it,
QuantSupport:

1. converts supported trades into scheduled `ContingentClaim` payments;
2. simulates the market data needed to value them;
3. at each exposure date, values the remaining payments on each simulated path.

You can read the results as exposure profiles, or combine them under netting
and collateral agreements to compute valuation adjustments.

## Claim-level valuation

A claim holds payment, fixing, and accrual dates, currencies, notional, side,
and stable trade and leg identifiers. Its strategy says how the amount is
computed. It can be:

- a fixed amount;
- a rate coupon;
- a coupon with optionality;
- a payoff on spot or a path;
- a payment that depends on exercise;
- a scripted payment.

For a native claim, the value is

\\[
v_{j,p,k}
=a_{j,p,k}\\,P_{j,p}(t_k,T_j)\\,
X_{j,p,k}\\,s_jN_j.
\\]

The strategy gives the raw amount \\(a\\). Discounting, currency conversion,
sign, and notional give the other factors. A scripted claim gets its value from
the compiled script instead. When you build claims, apply the notional only
once, following the strategy's convention.

Before simulation:

- the converter checks that the contract's features are supported, including
  overnight observation rules;
- `FixingPreprocessor` fills in fixings that are already known, fully or in
  part;
- compression merges compatible fixed claims so they are valued once.

## Cubes and pointwise profiles

`NpvCube` stores `npvs[path][date]` for a trade. For values \\(V_{p,k}\\),

\\[
EE_k=\frac1n\sum_pV_{p,k},\qquad
EPE_k=\frac1n\sum_p\max(V_{p,k},0),\qquad
ENE_k=\frac1n\sum_p\min(V_{p,k},0).
\\]

- ENE is signed and never positive.
- These are profiles at each date. There is no time-averaged EPE or PFE
  quantile.

`XvaAggregator` turns each path into an adjustment contribution. There are
implementations for CVA, DVA, and FVA. Contributions are divided by the total
number of paths and summed.

Cubes and profiles are raw values before margin. Adjustment aggregators can use
netted exposure after margin instead.

The EPE of a netted portfolio is generally not the sum of trade EPEs:

\\[
\max\left(\sum_jV_j,0\right)
\neq\sum_j\max(V_j,0).
\\]

To get netted profiles, sum the path values first and then take the positive
and negative parts.

## High-level orchestration

`XvaEngine` takes a market provider and an `XvaEngineConfig`.

- Each rate factor gives its mean reversion and parameter source, or reuses
  another factor as its driver.
- FX settings add foreign currencies, volatilities, and supported correlations.
- The configured frequency sets the simulation and exposure date grid.

`run` does the following:

1. prepares the claims;
2. finds the curves and factors they need;
3. builds the date grid;
4. computes the system discount factors and sets up each netting set;
5. values all claims on shared model paths.

Each foreign currency and each discount curve chosen by the policy needs a
supported model. If a required factor is missing, the engine returns an error.

The following fragment assumes prepared `context`, `configuration`, and
`netting_sets`:

```rust,ignore
let mut engine = XvaEngine::new(&context, configuration)?;
let result = engine.run(&mut netting_sets)?;
for cube in &result.cubes {
    println!("{}: {:?}", cube.trade_id, cube.epe());
}
for adjustment in result.xva_values.iter().flatten() {
    println!("{} {} {}", adjustment.netting_set, adjustment.measure, adjustment.value);
}
```

One run returns:

- a cube for each trade;
- optional adjustments for each netting set;
- optional aggregated sensitivities, by label.

The high-level engine computes CVA and FVA. DVA is not returned by `run`. To
compute it, use the lower-level aggregators with explicit own-credit inputs.

## Prepared execution and market refresh

An `XvaEngine` keeps a compiled path program for supported simulations. The
first `run` prepares it automatically, even for a small number of paths. You
can call `prepare` yourself to pay the recording and compilation cost before
asking for results. Both use the same engine and the same `run` method.

Preparation:

- computes the fixed curve and model terms;
- records the path, claim, collateral, and adjustment calculations;
- computes the map from model coefficients to the labelled market inputs used
  for risk.

Each later run uses fresh random draws and computes new exposures, adjustments,
and first derivatives. A changed contract or collateral agreement can require a
new preparation.

For a small, one-off valuation, preparation can take most of the time. Compare
the cost of the first call with the cost of repeated runs to decide whether
preparing is worthwhile. The benchmark report measures both.

The following fragment assumes `engine`, `netting_sets`, and a newly initialized
`new_context`:

```rust,ignore
engine.prepare(&mut netting_sets)?;
let first = engine.run(&mut netting_sets)?;
engine.update_market(&new_context)?;
engine.prepare(&mut netting_sets)?;
let updated = engine.run(&mut netting_sets)?;
```

`update_market` refreshes the engine's market data and calibration. The next
`prepare` refreshes the coefficients and their sensitivity to quotes. If the
calculation has the same structure, the compiled code is reused. Otherwise it
is compiled again.

Scripted and path-dependent claims run differently inside the same engine: each
path is recorded and its gradient is compiled and run. Calling `prepare`
explicitly returns an error for these strategies, while `run` picks the right
method automatically. Both report first-order risk. See
[calibration coverage](../risk/aad.md#calibration-dependencies) for which
derivatives use approximations and for the limits on second-order XVA risk.

The [XVA benchmark runners](https://github.com/jmelo11/quantsupport/blob/main/tools/README.md#benchmarks) measure first
preparation, reuse, market updates, overhead on small cases, and numerical
agreement.

## Numerical and modeling boundaries

All trades in a netting set use the same paths, so netting is consistent.
Finite grids and path counts are still approximations.

- LGM path generation, calibration targets, and correlations set the
  distribution of future states.
- Payoffs with optionality may need smoothing for stable results.
- To compare with a native pricer, use the same payoff, model, and payment
  selection, and allow for numerical and sampling error.

AD limits:

- The system discount factors are stored as numbers and held fixed.
- Credit and funding inputs enter as their own AD inputs. Their original
  calibrations are not replayed.

The [adjustment chapter](../xva/cva-dva-fva.md) explains what the derivatives
mean. The [collateral chapter](../xva/netting-csa.md) explains which exposures
the adjustments use.

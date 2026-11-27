# Netting and collateral

Trades under one netting agreement are valued together, so positive and
negative values can offset each other.

- `NettingSet` holds the claims of those trades and the discount policy.
- `CsaTerms`, attached to a netting set, gives the collateral, credit, and
  funding inputs used by the high-level XVA engine.

The collateral currency of the agreement sets the discounting. Variation-margin
settings set the simulated collateral balance and the remaining exposure used
for adjustments.

## Currency and discounting

The CSA currency and index give a `SingleCurveCSADiscountPolicy`:

- payments in the CSA currency use the collateral index;
- payments in other currencies use `Collateral(payment_ccy, collateral_ccy)`.

Each of these curves must be built and must have a supported rate model. FX
models link the currencies on each path.

- `NettingSet::new` takes an explicit policy, for lower-level exposure work.
- `with_csa_terms` adds the inputs used by the high-level adjustment
  calculations.

Claims from native instruments and from scripts can be in the same netting set,
as long as their direction, scale, and model requests are consistent.

## Variation margin

`variation_margin` is false by default. When it is enabled, the engine keeps one
signed collateral balance for each netting set and path. Thresholds and the
minimum transfer amount (MTA) must be finite and non-negative. They are in the
collateral currency, whatever the reporting currency.

For raw exposure \\(V\\), counterparty threshold \\(H_c\\), and own threshold
\\(H_o\\), the target collateral is

\\[
C^\ast=\max(V-H_c,0)-\max(-V-H_o,0).
\\]

- If \\(|C^\ast-C|\geq\mathrm{MTA}\\), the balance \\(C\\) is set to the
  target. Otherwise it stays the same.
- Between calls, collateral held earns interest at the simulated CSA
  remuneration curve rate.
- Collateral in a foreign currency is converted at the simulated FX rate.

Adjustment aggregators receive the exposure after collateral, \\(V-C\\). The
returned NPV cubes and their EPE, ENE, and EE methods show raw exposure before
margin. Keep this in mind when you reconcile profiles with a collateralized
adjustment.

Margin timing:

- A margin call happens on every simulation date, including the first, and
  settles at once.
- There is no separate call frequency, settlement lag, margin period of risk,
  or initial margin.

Refine the simulation grid to see how call timing affects the results.

## Credit and funding inputs

### Credit

- With `credit_index`, CVA uses that built survival curve.
- Otherwise, `credit_spread` is a flat credit spread. It is converted to a
  hazard rate using the loss given default:

\\[
\lambda=\frac{\text{credit_spread}}{1-\text{recovery}}.
\\]

`credit_spread` is a spread, not a hazard rate. The recovery rate also sets the
loss amount in CVA.

### Funding

Funding inputs are used in this order:

1. `funding_index`: forward spreads are derived from this curve relative to the
   system curve, plus an optional explicit spread.
2. `funding_spread_curve`: dated annual spreads.
3. `funding_spread`: a flat spread.

An explicit spread curve is a `ZeroRateTermStructure` or a tenor-based zero
curve. It uses its own reference date, day counter, interpolation, and
extrapolation. Risk to its zero-rate pillars can be reported. Spreads derived
from a funding index have a narrower risk scope, explained in the
[next chapter](cva-dva-fva.md).

### Several netting sets

All netting sets in a run use the same market paths. Each keeps its own
agreement and adjustment inputs. Netting changes the positive and negative
parts of exposure in a nonlinear way. Because FVA uses signed exposure, a
smaller positive exposure does not by itself mean a smaller FVA.

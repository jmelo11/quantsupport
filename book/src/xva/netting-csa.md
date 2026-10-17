# Netting and collateral

A netting set defines which positions offset under one agreement.
`NettingSet` owns their claims and a discount policy; `CsaTerms` additionally
supplies collateral, credit and funding assumptions for the high-level XVA
engine. A collateral discounting convention is not the same as simulating a
margin account.

## Currency and discounting

The CSA currency/index imply a `SingleCurveCSADiscountPolicy`. Payments in that
currency use the collateral index; foreign payments use
`Collateral(payment_ccy, collateral_ccy)`. Each selected identity needs a
constructed curve and supported rate model. FX models connect the relevant
currencies on paths.

`NettingSet::new` accepts an explicit policy for lower-level exposure work.
`with_csa_terms` attaches the additional economic inputs used by high-level
adjustment factories. Claims from native instruments and scripts can coexist,
provided their direction, scale and model requests are consistent.

## Variation margin

`variation_margin` defaults to false. When enabled, the engine maintains one
signed collateral balance per netting set and path. Thresholds and minimum
transfer amount are finite, nonnegative amounts in collateral currency,
independent of the reporting currency.

For raw exposure \\(V\\), counterparty threshold \\(H_c\\) and own threshold
\\(H_o\\), target collateral is

\\[
C^\ast=\max(V-H_c,0)-\max(-V-H_o,0).
\\]

The transfer rule replaces the current balance \\(C\\) when
\\(|C^\ast-C|\geq\mathrm{MTA}\\), including equality. Otherwise the previous
balance remains. Between calls, retained collateral accrues using the
simulated CSA remuneration-curve interval discount factor; foreign collateral
is converted at simulated FX.

Adjustment aggregators receive residual exposure \\(V-C\\) after each call.
Returned NPV cubes and their EPE/ENE/EE methods still show raw pre-margin
exposure. That reporting distinction is essential when reconciling profiles
to a collateralized adjustment.

Calls occur on every simulation date, including the initial date, and settle
immediately. Separate call frequency, settlement lag, margin period of risk
and initial margin are not implemented. Grid refinement can study this
particular timing approximation; it cannot create those absent contractual
features.

## Credit and funding inputs

With `credit_index`, CVA uses a constructed survival curve. Otherwise
`credit_spread` is interpreted as a flat credit spread and converted to hazard
using loss given default:

\\[
\lambda=\frac{\text{credit_spread}}{1-\text{recovery}}.
\\]

The field is not itself a hazard-rate input. Recovery also determines the
loss multiplier in CVA.

Funding precedence is separate. A configured `funding_index` derives forward
spreads relative to the system curve, with an optional explicit spread
overlay. Without that index, `funding_spread_curve` supplies dated annualized
spreads; otherwise the flat `funding_spread` is used.

Explicit spread curves reuse `ZeroRateTermStructure` or the supported
tenor-based zero representation. They respect their reference date, day
counter, interpolation and extrapolation conventions. The native AD
representation can retain explicit zero-rate pillar risk, but derived
funding-index factory leaves have a different scope, explained next.

Multiple netting sets share the run's market paths, while each retains its own
agreement and adjustment inputs. Netting changes positive and negative parts
nonlinearly. FVA's signed integration means that a smaller positive-exposure
profile alone does not establish a smaller funding adjustment.

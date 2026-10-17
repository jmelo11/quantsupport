# Credit, funding and XVA risk

Valuation adjustments integrate future exposure against credit or funding
inputs. QuantSupport separates path generation from measure-specific
aggregators. The distinction makes formulas inspectable, but also makes the
scope of each independent risk input explicit.

Let \\(U_{p,k}\\) be exposure after the netting/collateral rules applied for
the adjustment, \\(D_k\\) the numerical system discount factor, and
\\(\Delta t_k\\) the ACT/365 year fraction between consecutive grid dates.
The implemented integration uses right-endpoint exposure.

## CVA and DVA

For counterparty survival \\(Q_k\\) and recovery \\(R\\),

\\[
\mathrm{CVA}=(1-R)\frac1n
\sum_p\sum_{k=1}^{m}D_k\max(U_{p,k},0)(Q_{k-1}-Q_k).
\\]

Flat spread \\(s\\) is converted to hazard \\(\lambda=s/(1-R)\\), then
\\(Q(t)=e^{-\lambda t}\\). With a credit curve, survival comes from
log-linear survival interpolation and the implemented terminal hazard
continuation. A credit spread is not interchangeable with hazard, and recovery
can affect both the flat hazard and the loss multiplier.

DVA uses own-credit survival/recovery and \\(\max(-U,0)\\). It therefore
turns a negative exposure into a positive own-default benefit measure.
`DvaAggregator` and its factory are lower-level facilities. `XvaEngine::run`
automatically constructs CVA and FVA, not a fully specified bilateral
CVA/DVA model.

The aggregators produce named measures; they do not prescribe a universal
accounting sign for adding/subtracting those measures from a clean price.
State that convention at the application boundary.

## Signed funding integration

The implemented FVA is

\\[
\mathrm{FVA}=\frac1n\sum_p\sum_{k=1}^{m}
D_k\\,U_{p,k}\\,s_f(t_k)\\,\Delta t_k.
\\]

It uses signed exposure, not its positive part. Negative and positive funding
contributions can cancel. This is a symmetric signed funding convention,
not a separate borrowing/lending-spread FVA model. Do not replace \\(U\\)
with EPE while describing its results or claim monotonic reductions from a
change in netting.

Funding inputs can be flat, dated explicit spreads, or spreads derived from
a configured funding curve relative to the system curve. System discount
factors are optional at lower levels; high-level construction supplies them.
Without them, aggregation is not automatically present-valued by another
hidden discounting stage.

## What the returned sensitivities mean

The engine returns one aggregated list of labeled derivatives across the run.
Rate-path leaves can retain original curve calibration/IFT dependencies.
Fixed model lambda and configured correlations are not automatically
independent risk factors. Calibrated Gaussian sigma includes the supplied
volatility Jacobian but omits curve/strike-induced recalibration.

The numerical system discount sequence \\(D_k\\) is held fixed during AD.
A quote scenario that recalculates it can therefore disagree with the
reported partial derivative for a legitimate reason.

Credit-curve CVA has another important boundary. High-level construction
snapshots numerical survival pillars and their labels; `CreditCurveCvaFactory`
creates independent survival leaves. A label such as `CVA.<quote-like label>`
therefore describes a derivative with respect to a **survival pillar**, not a
replayed original CDS-spread calibration derivative. Recovery is separately
labeled. Quote-like text does not change a leaf's numerical units.

Similarly, funding-index construction snapshots derived forward spreads.
`FundingCurveFvaFactory` creates independent spread leaves, generally labeled
with `FVA.` prefixes, rather than differentiating their derivation from all
funding/system curve quotes. Explicit funding-spread curves can contribute
their own dated zero-rate leaves. Distinguish these factory risks from direct
deterministic credit/funding-curve sensitivities.

Value and gradient share simulated paths and sampling noise. Validate selected
dependencies with same-seed runs and carefully specified bumps, but compare
like with like: a full quote shock reconstructs more than a partial
survival/spread/model derivative may include. Reporting the leaf meaning,
units and held-fixed assumptions is as important as reporting the number.

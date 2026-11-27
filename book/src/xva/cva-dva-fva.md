# Credit, funding and XVA risk

Credit and funding adjustments combine future exposure with assumptions about
counterparty credit, own credit, or funding cost. QuantSupport:

1. generates market paths and values the portfolio on them;
2. applies netting and variation margin to get the exposure;
3. applies an aggregator for each measure to each path.

This chapter gives the formulas used and the inputs held fixed when
sensitivities are computed.

Notation:

- \\(U_{p,k}\\) is the exposure after the netting and collateral rules for the
  adjustment;
- \\(D_k\\) is the system discount factor;
- \\(\Delta t_k\\) is the ACT/365 year fraction between consecutive grid dates.

The sums use the exposure at the end of each grid interval.

## CVA and DVA

For counterparty survival probability \\(Q_k\\) and recovery \\(R\\),

\\[
\mathrm{CVA}=(1-R)\frac1n
\sum_p\sum_{k=1}^{m}D_k\max(U_{p,k},0)(Q_{k-1}-Q_k).
\\]

- A flat spread \\(s\\) is converted to a hazard rate \\(\lambda=s/(1-R)\\),
  and then \\(Q(t)=e^{-\lambda t}\\).
- With a credit curve, survival is interpolated log-linearly, and the last
  hazard rate is extended beyond the final pillar.
- A credit spread is not a hazard rate. The recovery rate affects both the
  flat hazard rate and the loss amount.

DVA uses own-credit survival and recovery, and \\(\max(-U,0)\\). A negative
exposure therefore gives a positive DVA.

`XvaEngine::run` computes CVA and FVA only. For DVA, use `DvaAggregator` and its
factory from the lower-level interface.

Aggregators return named measures. They do not decide whether a measure is
added to or subtracted from a clean price. Choose and document that sign in
your application.

## Signed funding integration

FVA is

\\[
\mathrm{FVA}=\frac1n\sum_p\sum_{k=1}^{m}
D_k\\,U_{p,k}\\,s_f(t_k)\\,\Delta t_k.
\\]

- FVA uses signed exposure, so positive and negative contributions can cancel.
- One spread is used for both borrowing and lending. Separate borrowing and
  lending spreads are not supported.
- A lower EPE alone therefore does not tell you how FVA changes.

The funding spread can be flat, a dated curve of spreads, or derived from a
funding curve relative to the system curve. At the lower level, system discount
factors are optional. If you leave them out, the result is not discounted. The
high-level engine always supplies them.

## What the returned sensitivities mean

The engine returns one list of labelled derivatives for the whole run.

- Rate-path inputs can carry risk back through curve calibration to the
  market quotes.
- Fixed model lambda and the configured correlations are not reported as risk
  factors.
- Calibrated Gaussian sigma includes the volatility Jacobian, but not
  refitting caused by curve or strike moves.
- The system discount factors \\(D_k\\) are held fixed. A quote scenario that
  recomputes them can therefore differ from the reported derivative.

### Credit-curve CVA

The high-level engine copies the survival probabilities at the credit pillars,
with their labels. `CreditCurveCvaFactory` uses these as independent AD inputs.
A label such as `CVA.<quote-like label>` is therefore a derivative with respect
to a **survival pillar**, not to the CDS spread quote. Recovery has its own
label. A label that looks like a quote does not change the units of the input.

### Funding

Spreads derived from a funding index are also copied as numbers.
`FundingCurveFvaFactory` uses them as independent AD inputs, usually with an
`FVA.` label prefix. Their sensitivity is not carried back to the funding or
system curve quotes. Explicit funding-spread curves report risk to their own
dated zero-rate pillars. Keep these apart from direct sensitivities to credit
or funding curves.

### Validation

The value and its gradient use the same paths and share the same sampling
noise. To check a sensitivity, run with the same seed and a carefully chosen
bump. Compare like with like: a full quote shock rebuilds more than a
derivative to a survival pillar, spread, or model input includes. When you
report a sensitivity, state what input it is for, its units, and what was held
fixed.

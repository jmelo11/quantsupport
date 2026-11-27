# Calibration and dependent curves

Curve calibration finds discount factors so that a set of instruments
reproduces their market quotes. A `CurveConfiguration` built from quotes sets:

- the calibration instruments;
- the interpolation rule;
- the market index of the resulting curve.

Some calibration instruments need other curves. For example, a term-rate
projection curve may be discounted on an overnight curve.
`MultiCurveBootstrapper` sorts out these dependencies. It then solves a
nonlinear system for all the pillars of each curve.

## Residuals and the nonlinear solve

Let \\(x=(P_1,\ldots,P_n)\\) be the unknown discount factors at the
calibration pillars, \\(q\\) the market quotes, and \\(z\\) the inputs of
parent curves that are already solved. Calibration solves

\\[
F(x,q,z)=0.
\\]

Each residual depends on the type of quoted contract:

- Deposits: implied rate minus quoted rate.
- Par swaps and basis swaps: NPV, which must be zero.
- Futures: implied rate minus quoted rate, with a convexity adjustment where
  supported.
- FX forwards: outright or forward points, depending on the quote.
- Bond anchors: model price minus a quoted price, or minus a price converted
  from a quoted yield, usually per 100. This is not OAS calibration.

Solver settings:

- Newton's method, with residual Jacobians computed by finite differences.
- Trial bumps and accepted steps keep discount factors positive.
- Each factor starts at `0.99`. The tolerance is `1e-12` and the iteration
  limit is 200.

Check the convergence status and the repricing residuals. If the solver runs
out of iterations, it can return a result without a `SolverErr`.

## Dependencies and collateral

A calibration instrument can project on one index and discount on another.

- A Term SOFR projection curve calibrated from basis swaps can use SOFR
  discounting as a parent curve.
- CLP payments collateralized in USD may need `Collateral(CLP, USD)` and an FX
  spot store.

The bootstrapper sorts the curves so that parents are solved first. You can
list configurations in any order. A missing parent, a duplicate identifier, a
curve that refers to itself, or a cycle is an error.

The discount policy sets which curve plays which role. Under USD collateral:

- USD payments use the chosen USD collateral index;
- CLP payments use the USD-collateralized CLP curve;
- a local ICP curve can project CLP coupons.

ICP is a parent of the collateral curve only if one of that curve's pillar
instruments actually uses it.

The same configuration list can include zero-rate curves built from quotes and
composite curves. A composite depends on both of its component curves, and it
can be the parent of other calibrated curves.

## Overnight-index calibration instruments

An OIS exchanges fixed coupons for compounded overnight interest. The quote
`OIS_CCY_Index_Tenor` builds an `OvernightIndexSwap` for calibration.

Default conventions:

- coupon-only legs with annual schedules;
- the index's calendar and day count;
- Modified Following for accrual dates and Following for payment dates;
- SOFR: two business days to settlement and two business days of payment lag;
- SONIA: zero settlement days; CORRA: one settlement day;
- other indices: two settlement days and no payment lag.

Optional positional fields in the identifier override these conventions. Empty
fields keep the default. For example, `OIS_USD_SOFR_1Y___0_3` keeps the
frequencies and sets settlement to zero days and payment lag to three days.
Calendars use their full names, such as `UnitedStates(Sofr)`, not just a
country name.

How OIS coupons are computed:

- A coupon that starts in the future is valued as a ratio of discount factors.
- A coupon that has already started compounds the past business-day fixings
  with their accrual weights, then projects the rest.
- A fixing on the valuation date is used if present, and projected otherwise.
- Payment lag affects discounting only, not accrual.
- Lookback, lockout, observation shift, and daily spread compounding change
  which fixings are used. They must match the real contract.

Deterministic pricing supports non-standard overnight settings that the legacy
XVA claim converter rejects.

### Regular swap quotes

`Swap_CCY_Index_Tenor[_FixedFreq[_FloatFreq[_SettlementDays[_FixedDC[_FloatDC]]]]]`
builds a swap whose floating coupons each use one fixing, even when the index is
an overnight index.

- Defaults come from the index's `swap_conventions(tenor)`. These are separate
  from the conventions used to quote the index rate itself.
- The tenor is counted from the settlement date on the index calendar.
- Swaption underlyings and swap builders use the same defaults.

Default conventions by index:

- Euribor: annual **Thirty360 (bond basis)** fixed coupons; floating coupons at
  the index tenor with ACT/360 accrual; TARGET calendar; two settlement days.
- TIBOR: semiannual ACT/365 fixed coupons.
- NIBOR: annual 30/360 fixed coupons.
- SOFR: annual ACT/360 on both legs.
- SONIA: annual ACT/365; zero settlement days.

There is no default fixed leg shared by all indices.

Each optional field overrides one convention. For example:

- `Swap_EUR_EURIBOR3m_1Y_Annual_Quarterly_1_Thirty360_Actual360` sets both
  frequencies, one settlement day, and both day counts.
- `Swap_EUR_EURIBOR3m_1Y___1_Actual365` keeps the index frequencies and
  floating day count, and changes the settlement days and fixed day count.

In builders and serialized quote details, set the same terms with the
individual setters and fields.

The settlement lag delays the start of accrual. Regular swap payments are made
at the end of each accrual period. For daily compounded coupons with their own
payment lag, use OIS quotes instead.

**Migration note:** to reproduce curves built with the earlier
semiannual/quarterly swap defaults, set those frequencies and day counts
explicitly.

### Reproducing a published curve

To match a curve published by someone else, match the calibration contracts,
interpolation, and valuation assumptions, not only the quoted rates. Also:

- The date a quote was taken need not be its effective date.
- `discount_factor(T)` is measured from the curve reference date. For a factor
  from a later start date `S`, use `discount_factor(T) / discount_factor(S)`.
- The curve's day counter measures interpolation time only. It does not change
  the fixed or floating day counts of the swap quotes.

The [Euribor 3M reconciliation](../../../examples/bootstrap/src/bin/euribor3m-reconciliation.rs)
example compares annual and semiannual fixed legs and checks the cashflows
independently.

## Differentiating calibration

At the solution, the implicit function theorem (IFT) gives

\\[
\frac{\partial x}{\partial q}
=-J^{-1}\frac{\partial F}{\partial q},\qquad
\frac{\partial x}{\partial z}
=-J^{-1}\frac{\partial F}{\partial z},
\quad J=\frac{\partial F}{\partial x}.
\\]

- \\(\partial F/\partial q\\) is diagonal, because each residual depends only
  on its own quote.
- Derivatives with respect to parent-curve inputs are computed by finite
  differences.

These matrices link the solved factors to the AD inputs with a first-order
relation:

\\[
x_i^{\rm AD}=x_i^\ast
+\sum_j S_{ij}(q_j^{\rm AD}-q_j^\ast)
+\sum_k C_{ik}(z_k^{\rm AD}-z_k^\ast).
\\]

This keeps the calibrated values and their first-order market sensitivities,
including risk that flows through parent curves.

Limitations:

- It does not repeat the Newton iterations.
- Second derivatives of the calibration are not exact.
- Finite-difference Jacobians and solver tolerances add numerical error.

## Credit calibration

`CreditCurveBootstrapper` builds piecewise-constant hazard rates from CDS par
spreads. A `CreditCurveElement` stores a curve whose values are survival
probabilities \\(Q(t)\\), not risk-free discount factors.

The configuration sets:

- the credit name;
- the payment currency;
- the recovery rate;
- the premium frequency;
- the discount index for the premium and protection legs.

For each maturity, the solver finds the hazard rate of the new interval while
keeping earlier intervals fixed. It uses bisection between \\(10^{-12}\\) and
20, with at most 200 iterations. Sensitivities to CDS quotes use a
finite-difference IFT step.

Empty quote sets and duplicate maturities are errors.

Discount and credit curves are built and differentiated in the same way, but
their values mean different things: discount factors versus survival
probabilities.

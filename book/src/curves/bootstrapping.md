# Calibration and dependent curves

Curve calibration chooses discount factors so that the prices or rates of
selected instruments reproduce market observations. A quote-based
`CurveConfiguration` specifies the instruments, interpolation rule and market
index of the resulting curve.

Some calibration instruments need other curves. For example, a term-rate
projection curve may use an overnight curve for discounting.
`MultiCurveBootstrapper` resolves these dependencies, then solves a nonlinear
system for all pillars of each curve.

## Residuals and the nonlinear solve

Let \\(x=(P_1,\ldots,P_n)\\) be unknown factors at the calibration pillars,
\\(q\\) the associated market quotes, and \\(z\\) already solved parent-curve
inputs. Calibration imposes

\\[
F(x,q,z)=0.
\\]

Each residual follows the quoted contract. Deposits compare implied and quoted
rates; par swaps and basis swaps contribute NPV conditions; futures compare
implied and quoted rates with supported convexity adjustment. FX forwards use
their outright/points strategy. Bond anchors compare model price with either
a quoted price or a yield-converted target, normally in units per 100.
These bond strategies do not constitute OAS calibration.

The bootstrapper uses a Newton solve with finite-difference residual
Jacobians. Trial bumps and accepted steps preserve positive discount factors.
The current setup starts each factor at `0.99`, with tolerance `1e-12` and
200 iterations. Inspect convergence status and repricing residuals: iteration
exhaustion can return a result without `SolverErr`.

## Dependencies and collateral

Calibration instruments may project one index and discount on another.
SOFR discounting can therefore be a parent of a Term SOFR projection curve
calibrated from basis swaps. Foreign-currency payments under USD collateral
may require `Collateral(CLP, USD)` and an FX spot store. A topological sort
orders the configured dependencies; absent parents and cycles are errors.

The discount policy makes these roles explicit. For USD collateral, USD
payments use the selected USD collateral index and CLP payments use their
USD-collateralized CLP curve. A local ICP curve can separately project CLP
coupons. Only the instruments actually chosen as collateral-curve pillars
determine whether ICP is a dependency of that calibration.

The bootstrapper determines solve order from dependencies, so configurations
can be supplied in any order. The same list can include explicit zero-rate
curves and composites. A composite depends on both component
curves and can itself become a parent of calibrated curves. Duplicate
identities, self-references and cycles are rejected.

## Overnight-index calibration instruments

An OIS exchanges fixed coupons against compounded overnight interest.
`OIS_CCY_Index_Tenor` identifies this contract and builds an
`OvernightIndexSwap` for calibration.
The preset uses coupon-only legs with annual schedules, index calendars and
day counts, Modified Following accrual adjustment and Following payment
adjustment. SOFR uses two business days to settlement and two business days of
payment lag; SONIA uses zero settlement days and CORRA one. Other index
presets use two settlement days and zero payment lag.

Optional positional fields in an OIS identifier override those contract
conventions. Empty intermediate fields retain defaults: for example,
`OIS_USD_SOFR_1Y___0_3` retains frequencies and sets settlement/payment lag to
zero/three. Calendars use canonical names such as `UnitedStates(Sofr)`, not an
assumed bare-country alias.

Standard future compounded coupons use discount-ratio telescoping.
Seasoned coupons compound historical business-day observations with their
accrual weights, then project the remainder. Same-day observations are used
when available and forecast otherwise. Payment lag affects discounting, not
accrual. Lookback, lockout, observation shift and daily spread compounding
alter observation semantics and must match the actual contract. Deterministic
valuation supports nonstandard overnight settings that the legacy XVA claim
converter rejects.

`Swap_CCY_Index_Tenor[_FixedFreq[_FloatFreq[_SettlementDays[_FixedDC[_FloatDC]]]]]`
uses single-fixing floating coupons, including when the index is overnight.
Its defaults come from the index's `swap_conventions(tenor)` metadata, which is
separate from the rate's deposit/forward quoting definition. The tenor runs
from settlement on the index calendar. Swaption underlyings and direct swap
builders use the same defaults.

Euribor defaults to annual **Thirty360 (bond basis)** fixed coupons, floating
coupons at the index tenor with ACT/360 accrual, TARGET and two settlement
days. TIBOR defaults to semiannual ACT/365 fixed coupons, while NIBOR uses
annual 30/360. SOFR uses annual ACT/360 on both legs; SONIA uses annual
ACT/365 and zero settlement days. There is no universal fixed-leg default.

Optional fields override each convention independently. For example,
`Swap_EUR_EURIBOR3m_1Y_Annual_Quarterly_1_Thirty360_Actual360` supplies both
frequencies, one settlement day and both accrual conventions.
`Swap_EUR_EURIBOR3m_1Y___1_Actual365` retains the index frequencies and
floating day count while changing settlement and fixed accrual. Empty fields
retain defaults. Rust builders and serialized quote details can supply the
same terms through their individual setters and fields.

Settlement lag delays the start of accrual; regular swap payments occur at
accrual end. Use OIS quotes for compounded daily observations and their
separate payment-lag conventions. To reproduce curves made with the former
semiannual/quarterly defaults, supply those frequencies and the former day
counts explicitly.

To reproduce a supplied market curve,
match the calibration contracts, interpolation and valuation assumptions as well
as the quoted rates.

In particular, a quote's extraction date need not be its effective date, and
`discount_factor(T)` is anchored at the curve reference date. Compare a factor
measured from a later start `S` using `discount_factor(T) / discount_factor(S)`.
The curve's day counter measures interpolation time; it does not override the
swap quote's fixed or floating coupon day counts. See the executable
[Euribor 3M reconciliation](../../../examples/bootstrap/euribor3m-reconciliation.md)
for an annual-versus-semiannual comparison and an independent cashflow check.

## Differentiating calibration

At a converged solution, implicit differentiation gives

\\[
\frac{\partial x}{\partial q}
=-J^{-1}\frac{\partial F}{\partial q},\qquad
\frac{\partial x}{\partial z}
=-J^{-1}\frac{\partial F}{\partial z},
\quad J=\frac{\partial F}{\partial x}.
\\]

The implemented own-quote derivative is diagonal by calibration-instrument
construction. Parent dependencies are finite-differenced. These matrices
reconnect solved factors to tape inputs through a local first-order relation:

\\[
x_i^{\rm AD}=x_i^\ast
+\sum_j S_{ij}(q_j^{\rm AD}-q_j^\ast)
+\sum_k C_{ik}(z_k^{\rm AD}-z_k^\ast).
\\]

The relation preserves the calibrated value and its local market derivatives,
including chained parent-curve risk. It does not replay the Newton iterations
or establish exact second-order differentiation of the calibration map.
Finite-difference Jacobians and solver tolerances remain numerical
approximations.

## Credit calibration

`CreditCurveBootstrapper` strips piecewise-constant hazards from CDS par
spreads. A `CreditCurveElement` uses a term-structure representation whose
curve value is survival probability \\(Q(t)\\), not a risk-free discount factor.
Configuration supplies credit identity, payment currency, recovery, premium
frequency and the discount index for the premium/protection legs.

For each maturity the new hazard interval is solved with earlier intervals
fixed. The implemented bisection search uses bounds \\(10^{-12}\\) to 20 and
200 iterations. A finite-difference IFT step connects survival to CDS quotes.
Empty quote sets and duplicate maturities are rejected. Discount and credit
curves therefore share construction and risk concepts, but their numerical
values have different economic meanings.

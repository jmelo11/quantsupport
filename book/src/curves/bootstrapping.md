# Calibration and dependent curves

Curve calibration chooses discount factors so that selected instruments
reproduce observed rates or prices. In QuantSupport, a quote-based
`CurveConfiguration` identifies those instruments, an interpolation rule and
a curve identity. `MultiCurveBootstrapper` resolves their dependencies and
solves each curve as a global nonlinear system, not as an assumption that every
instrument affects only one payment.

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

The solver starts from `0.99` at each unknown factor and uses
`VectorNewton::new(1e-12, 200)`. Its Jacobian comes from finite differences of
the full residual system. Bumps are scale-controlled and clipped to retain
positive trial factors. Newton step acceptance also guards positivity.
Numerical convergence status must be distinguished from the absence of a
returned error: iteration exhaustion is not universally reported as
`SolverErr`.

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

List order is not solve order. The same context list can also include explicit
zero-rate definitions and composites. A composite depends on both component
curves and can itself become a parent of calibrated curves. Duplicate
identities, self-references and cycles are rejected.

## OIS is a distinct calibration contract

`OIS_CCY_Index_Tenor` builds `OvernightIndexSwap`, not a generic regular swap.
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

`Swap_CCY_Index_Tenor[_FixedFreq[_FloatFreq]]` remains a different quote family.
It starts under regular swap conventions with single-fixing coupons and
semiannual/quarterly frequency defaults. Giving it an overnight index does not
make it an OIS. Matching a vendor curve requires matching all these instrument
choices, interpolation and valuation-time assumptions, not just the rates.

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

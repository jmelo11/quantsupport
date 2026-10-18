# Interest-rate options

An interest-rate option gives its holder a payment or exercise right tied
to future rates. Caps and floors protect individual coupon periods, while a
swaption gives the right to enter a swap. Their valuation uses the underlying
schedules and curves together with an option-pricing model.

QuantSupport supports optionlet valuation from Black or Normal volatility
markets and Gaussian short-rate models for caplets, caps and swaptions. The
choice determines the required volatility inputs and the interpretation of
model sensitivities.

## Optionlets and Black/Normal markets

A caplet fixes at \\(T\\), accrues over \\([T,S]\\), and pays
\\(N\tau(L-K)^+\\). A floorlet replaces the payoff by \\(N\tau(K-L)^+\\).
`MakeCapFloor` builds a strip of these periods. Under Black-76,

\\[
C=N\tau P_d(0,S)
\left[F\Phi(d_1)-K\Phi(d_2)\right],\qquad
d_1=\frac{\log(F/K)+\tfrac12\sigma^2T}{\sigma\sqrt T},
\quad d_2=d_1-\sigma\sqrt T.
\\]

Projection determines \\(F\\), the policy-selected curve determines
\\(P_d\\), and the index surface supplies volatility at the fixing/strike
coordinates. `Strike` can be absolute, ATM or relative to the forward.
The Black and Hull–White caplet/cap pricers interpret `Relative(p)` as
\\(F(1+p)\\), unlike the additive \\(F+p\\) returned by the generic
`Strike::resolve` helper. An absolute strike avoids this consumer-specific
ambiguity.
`VolatilityType::Normal` selects Bachelier rather than forcing a lognormal
formula on absolute-rate volatility:

\\[
C=N\tau P_d(0,S)
\left[(F-K)\Phi(d)+\sigma_N\sqrt T\\,\phi(d)\right],
\quad d=\frac{F-K}{\sigma_N\sqrt T}.
\\]

A cap/floor sums the corresponding optionlets. Market sensitivities can
therefore reach projection and discount quotes and the neighboring surface
pillars. The units of Black vega and Normal vega are not interchangeable.

## Gaussian bond-option representation

A one-curve caplet can be expressed as a put on the accrual-end bond:

\\[
C=N(1+\tau K)\\,
\operatorname{PutBond}\left(T,S,\frac{1}{1+\tau K}\right).
\\]

Hull–White supplies an affine bond price and its option volatility. For
constant sigma and nonzero mean reversion \\(\alpha\\),

\\[
B(t,S)=\frac{1-e^{-\alpha(S-t)}}{\alpha},\qquad
\sigma_P=\sigma B(T,S)
\sqrt{\frac{1-e^{-2\alpha T}}{2\alpha}}.
\\]

The fixed-parameter Hull–White caplet and cap pricers take alpha and sigma
directly. Their risk reports treat these supplied parameters as fixed, while
configured calibration is responsible for connecting model parameters to
market volatility quotes.

## Swaptions and Jamshidian decomposition

`MakeSwaption` creates a `EuropeanSwaption` with distinct expiry and underlying
swap maturity, payer/receiver direction, strike, rate convention and schedule.
A payer has the right to pay fixed. The specialized closed-form pricer uses
Jamshidian's bond-option decomposition.

At expiry \\(T_0\\), choose the critical rate \\(r^\ast\\) satisfying

\\[
\sum_i c_iP(T_0,T_i\mid r^\ast)=1,
\qquad X_i=P(T_0,T_i\mid r^\ast).
\\]

Here \\(c_i\\) includes the fixed coupons and final principal in the
decomposition. A payer becomes a weighted sum of bond puts with strikes
\\(X_i\\); a receiver uses calls. The converged critical-rate dependency is
reconnected by implicit differentiation rather than differentiation of branch
choices in the scalar root solver.

`ClosedFormHullWhiteSwaptionPricer::from_configuration` accepts a typed
`EuropeanSwaption`/`HullWhiteClosedForm` assignment and a Hull–White model for
the selected index. Fixed parameters give a constant sigma. A calibrated source
selects caplet-surface or swaption-cube contracts and resolves a piecewise
sigma schedule. A custom provider must supply source quote details as well as
constructed volatility objects and curves.

The schedule assembles independent constant-sigma fits; it is not a joint
piecewise-volatility repricing calibration. Its diagonal forward-difference
Jacobian includes sigma's response to volatility inputs, but not its response to
curve quotes or moving calibration strikes. Direct curve risk through valuation
still exists. Cube calibration permits at most one underlying tenor per expiry
and must remain in-domain. Sensitivity reports therefore include partial
calibration risk with these inputs held fixed.

The [model chapter](../simulation/monte-carlo.md) distinguishes these
closed forms from actual numerical path generation and its zero-parameter
limits.

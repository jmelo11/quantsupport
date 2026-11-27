# Interest-rate options

An interest-rate option pays out, or gives an exercise right, based on future
rates.

- A **cap** or **floor** protects each coupon period separately.
- A **swaption** gives the right to enter a swap.

Valuation uses the underlying schedules and curves together with an option
model. QuantSupport supports:

- caplets and floorlets from Black or Normal volatility markets;
- Gaussian short-rate models (Hull–White) for caplets, caps, and swaptions.

The choice of model decides which volatility inputs you need and what the model
sensitivities mean.

## Optionlets and Black/Normal markets

A caplet fixes at \\(T\\), accrues over \\([T,S]\\), and pays
\\(N\tau(L-K)^+\\). A floorlet pays \\(N\tau(K-L)^+\\). `MakeCapFloor` builds
a strip of these periods. Under Black-76,

\\[
C=N\tau P_d(0,S)
\left[F\Phi(d_1)-K\Phi(d_2)\right],\qquad
d_1=\frac{\log(F/K)+\tfrac12\sigma^2T}{\sigma\sqrt T},
\quad d_2=d_1-\sigma\sqrt T.
\\]

- The projection curve gives \\(F\\).
- The discount policy's curve gives \\(P_d\\).
- The index's volatility surface gives the volatility at the fixing date and
  strike.

`Strike` can be absolute, ATM, or relative to the forward. The Black and
Hull–White caplet and cap pricers read `Relative(p)` as \\(F(1+p)\\). The
generic `Strike::resolve` helper returns \\(F+p\\) instead. Use an absolute
strike to avoid any doubt.

`VolatilityType::Normal` uses the Bachelier formula, for volatilities quoted in
absolute rate units:

\\[
C=N\tau P_d(0,S)
\left[(F-K)\Phi(d)+\sigma_N\sqrt T\\,\phi(d)\right],
\quad d=\frac{F-K}{\sigma_N\sqrt T}.
\\]

A cap or floor is the sum of its optionlets. Sensitivities include the
projection and discount quotes and the nearby surface pillars. Black vega and
Normal vega have different units and cannot be compared directly.

## Gaussian bond-option representation

On a single curve, a caplet equals a put on the bond that matures at the end of
the accrual period:

\\[
C=N(1+\tau K)\\,
\operatorname{PutBond}\left(T,S,\frac{1}{1+\tau K}\right).
\\]

Hull–White gives an affine bond price and its option volatility. For constant
sigma and nonzero mean reversion \\(\alpha\\),

\\[
B(t,S)=\frac{1-e^{-\alpha(S-t)}}{\alpha},\qquad
\sigma_P=\sigma B(T,S)
\sqrt{\frac{1-e^{-2\alpha T}}{2\alpha}}.
\\]

The fixed-parameter Hull–White caplet and cap pricers take alpha and sigma
directly. Their sensitivities treat alpha and sigma as constants. To get risk
to market volatility quotes, calibrate the model from configuration.

## Swaptions and Jamshidian decomposition

`MakeSwaption` creates a `EuropeanSwaption`. You set the expiry, the maturity of
the underlying swap, payer or receiver, strike, rate convention, and schedule.
A payer swaption gives the right to pay fixed. The closed-form pricer uses
Jamshidian's decomposition into bond options.

At expiry \\(T_0\\), find the critical rate \\(r^\ast\\) such that

\\[
\sum_i c_iP(T_0,T_i\mid r^\ast)=1,
\qquad X_i=P(T_0,T_i\mid r^\ast).
\\]

Here \\(c_i\\) are the fixed coupons plus the final principal. A payer
swaption is then a weighted sum of bond puts with strikes \\(X_i\\). A receiver
uses calls. Sensitivity to the critical rate is computed with implicit
differentiation at the solved root.

`ClosedFormHullWhiteSwaptionPricer::from_configuration` takes an
`EuropeanSwaption`/`HullWhiteClosedForm` pricer assignment and a Hull–White
model for the index.

- With fixed parameters, sigma is constant.
- With a calibrated source, the pricer selects caplet-surface or swaption-cube
  contracts and builds a piecewise sigma schedule.
- A custom market data provider must supply the source quote details as well as
  the volatility objects and curves.

Limits of the calibration risk:

- The schedule is built from separate constant-sigma fits, not from a joint
  fit of the whole schedule.
- Its Jacobian (one-sided differences, one quote at a time) includes how sigma
  moves with the volatility quotes. It does not include how sigma moves with
  curve quotes or with calibration strikes that depend on the curve.
- Direct curve risk through pricing is still included.
- Cube calibration uses at most one underlying tenor per expiry, within the
  range of the cube.

Sensitivity reports therefore include only part of the calibration risk.

The [model chapter](../simulation/monte-carlo.md) explains how these closed
forms differ from numerical path generation.

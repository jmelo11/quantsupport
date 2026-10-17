# Volatility markets and model calibration

An implied-volatility market describes option prices across contract
coordinates. A dynamics model describes future states. Calibrating the second
to the first does not make them the same object: a surface retains the
observations that define the target, while the model retains parameters that
produce its own distribution.

## Coordinates and interpolation

`VolatilitySurfaceConfiguration` selects expiry/smile quotes.
`VolatilityCubeConfiguration` adds underlying tenor, as needed for swaptions.
The volatility type is Black or Normal; the smile coordinate can be strike,
delta or log-moneyness. The generic `Strike::resolve` maps `Absolute`, `Atm`
and `Relative` to \\(K\\), \\(F\\) and \\(F+\text{spread}\\). Consumers do
not all use that helper: the Black and Hull–White caplet/cap pricers interpret
`Relative(p)` multiplicatively as \\(F(1+p)\\). Prefer an absolute strike
when specifying an unambiguous contractual rate.

Configured identifiers supply the actual coordinates. Equivalent tenors such
as `1Y` and `12M` cannot define duplicate nodes. Builders reject empty sets,
nonpositive expiry/tenor, nonfinite keys and negative or nonfinite volatilities.
The selected bid/mid/ask level supplies the quoted values.

Interpolated surfaces first interpolate within each smile, then across expiry.
Cubes interpolate smile, tenor and finally expiry. Rectangular grids reproduce
bilinear/trilinear interpolation, but sparse grids can have different strikes
per slice and different tenor sets per expiry. Exact nodes reproduce quoted
values without needing a neighboring slice.

`ExtrapolationPolicy::Reject` is the default. A query must lie within each
selected slice; a singleton coordinate must match exactly. `Flat` clamps to
boundaries and treats singleton axes as constant. Both reject nonfinite
queries. Interpolation is in the implemented volatility coordinates, not an
automatic guarantee of arbitrage-free total variance.

An FX market can store one pair orientation. `OrientedFxVolSurface` resolves a
reciprocal request and maps inverse strike with \\(K\mapsto1/K\\). Smile-type
metadata elsewhere does not guarantee universal conversion between strike,
delta and moneyness. In particular, the current equity option pricer passes a
resolved strike directly; configure a compatible surface rather than assume a
delta inversion is being solved.

## Source instruments and risk pillars

Constructed volatility objects retain calibration-instrument identities.
These encode option expiry, tenor and strike and allow model calibration to
select a meaningful basket. Risk pillars identify differentiable inputs whose
labels appear in sensitivity results. An interpolated quote grid commonly uses
the same identifiers for both purposes, but the roles are conceptually
different.

`CalibrationBasket` selects expiries, tenors and a strike rule from the
configured source. Omitted axes include all available values. Every explicitly
requested value must exist and participate after the other filters are applied.
A caplet surface's tenor is the index observation tenor; a cube's tenor is
the underlying swap tenor. A three-month caplet source cannot satisfy a
six-month basket by silently reinterpreting those contracts.

## Parameter sources

`ParameterSource::Fixed` supplies a complete model parameter set.
`ParameterSource::Calibrated` names a surface or cube and a semantic basket.
Hull–White and LGM use Gaussian sigma parameters; lognormal Brownian motion
uses a volatility parameter. These are distinct units and parameterizations,
even when they are ultimately fitted to the same option premium.

Gaussian calibration converts each selected caplet or swaption's Black or
Normal volatility into an option price. It then fits a **constant** sigma to
each instrument independently and assembles those fitted values into a
piecewise schedule. This is not a forward-sigma bootstrap that preserves
earlier intervals while fitting the next instrument. The assembled schedule
is not guaranteed to jointly reproduce the calibration basket.

Hull–White quality reports retain instrument identity, target volatility/price,
fitted price, sigma, forward and strike. Their fitted prices describe the
individual constant-sigma solves, not repricing under the assembled schedule.

For lognormal Brownian motion, term-volatility stripping instead enforces

\\[
\int_0^{T_i}\sigma(u)^2\\,du=\sigma_{\rm term}(T_i)^2T_i.
\\]

Negative forward variance is rejected. This makes the meaning of model
volatility explicit: it is not generally the number read from one surface node.

The Gaussian calibration Jacobian uses diagonal forward differences with
absolute volatility bump \\(h=10^{-6}\\):

\\[
\frac{\partial\sigma_i}{\partial\nu_j}
\approx\delta_{ij}
\frac{\sigma_i(\nu_i+h)-\sigma_i(\nu_i)}{h}.
\\]

This numerical response holds curve inputs and calibration strikes fixed.
It is not a full joint schedule-calibration Jacobian. Direct pricing may still
have curve risk. Cube calibration selects at
most one underlying tenor per expiry and must respect the source domain.
Materialized numerical simulations introduce further derivative limits,
explained in [Stochastic models](../simulation/monte-carlo.md).

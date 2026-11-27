# Volatility markets and model calibration

Option markets quote implied volatility by expiry and strike. For swaptions
they also quote by the tenor of the underlying swap. QuantSupport builds
volatility surfaces and cubes from these quotes. Pricers can read them
directly, or use them as targets to calibrate a stochastic model.

Calibration turns the selected volatility quotes into option prices and then
fits the model parameters to those prices. The parameters are in the units of
that model.

## Coordinates and interpolation

- `VolatilitySurfaceConfiguration` selects quotes by expiry and smile point.
- `VolatilityCubeConfiguration` adds the underlying tenor, as needed for
  swaptions.

The volatility type is Black or Normal. The smile coordinate can be strike,
delta, or log-moneyness.

Strikes are given as `Absolute`, `Atm`, or `Relative`. The generic
`Strike::resolve` turns them into \\(K\\), \\(F\\), and
\\(F+\text{spread}\\). The Black and Hull–White caplet and cap pricers read
`Relative(p)` differently, as \\(F(1+p)\\). When you need an exact contract
rate, use an absolute strike.

The configured quote identifiers give the actual coordinates. Rules:

- Equivalent tenors such as `1Y` and `12M` cannot both appear as nodes.
- Builders reject empty quote sets, zero or negative expiries or tenors,
  non-finite coordinates, and negative or non-finite volatilities.
- The selected bid, mid, or ask level gives the quoted values.

Interpolation order:

- Surfaces interpolate along each smile first, then across expiries.
- Cubes interpolate along the smile, then across tenors, then across expiries.

On a full rectangular grid this is ordinary bilinear or trilinear
interpolation. A sparse grid can have different strikes in each slice and
different tenors for each expiry. A query exactly on a node returns the quoted
value.

Extrapolation:

- `ExtrapolationPolicy::Reject` is the default. A query must fall inside every
  slice it uses. On an axis with a single point, the query must match that
  point exactly.
- `Flat` clamps queries to the boundary and treats a single-point axis as
  constant.
- Both reject non-finite queries.

Interpolation works directly on volatilities. It does not guarantee an
arbitrage-free surface.

An FX market can store one direction of a currency pair. `OrientedFxVolSurface`
answers requests for the reverse pair and maps the strike with \\(K\mapsto1/K\\).

The smile type of a surface does not mean the library converts between strike,
delta, and moneyness for you. The equity option pricer passes a strike to the
surface. Configure a surface that uses strikes; the pricer does not solve for
delta.

## Source instruments and risk pillars

Each built volatility object keeps the identifiers of the options it was built
from. An identifier gives the option's expiry, tenor, and strike, so model
calibration can pick a sensible basket.

Risk pillars are the inputs whose labels appear in sensitivity results. On an
interpolated quote grid, the pillars usually have the same identifiers as the
source options, but they serve a different purpose.

`CalibrationBasket` selects expiries, tenors, and a strike rule from the
configured source:

- If you leave out an axis, all its values are included.
- Every value you list must exist and must remain after the other filters.
- For a caplet surface, the tenor is the index tenor. For a cube, it is the
  underlying swap tenor. A basket asking for six-month caplets cannot use a
  surface of three-month caplets.

## Parameter sources

- `ParameterSource::Fixed` gives a complete set of model parameters.
- `ParameterSource::Calibrated` names a surface or cube and a calibration
  basket.

Hull–White and LGM use Gaussian sigma parameters. Lognormal Brownian motion
uses a volatility parameter. These have different units and meanings, even if
both are fitted to the same option prices.

### Gaussian models (Hull–White, LGM)

Calibration converts the Black or Normal volatility of each selected caplet or
swaption into an option price. It then:

1. fits a constant sigma to each option on its own;
2. combines the fitted values into a piecewise sigma schedule.

Because each option is fitted separately, repricing with the combined schedule
can differ from the target prices.

Hull–White quality reports list, for each option: identifier, target
volatility and price, fitted price, sigma, forward, and strike. The fitted
price comes from the single-option fit, not from repricing with the combined
schedule.

### Lognormal Brownian motion

Term volatilities are stripped so that

\\[
\int_0^{T_i}\sigma(u)^2\\,du=\sigma_{\rm term}(T_i)^2T_i.
\\]

Negative forward variance is an error. The stripped period volatilities
reproduce the total variance implied by the selected quotes.

### Calibration sensitivities

The Gaussian calibration Jacobian uses forward differences with an absolute
volatility bump \\(h=10^{-6}\\), one quote at a time:

\\[
\frac{\partial\sigma_i}{\partial\nu_j}
\approx\delta_{ij}
\frac{\sigma_i(\nu_i+h)-\sigma_i(\nu_i)}{h}.
\\]

- Curve inputs and calibration strikes are held fixed in this calculation.
- It is not the Jacobian of a joint fit of the whole schedule.
- Direct pricing may still have curve risk.

Cube calibration uses at most one underlying tenor per expiry, within the
range of the source cube. Stored Monte Carlo simulations add further limits on
derivatives; see [Stochastic models](../simulation/monte-carlo.md).

# Stochastic models and simulation

Simulation provides the future rates, spot prices and discount factors needed
for stochastic payoffs and exposure calculations. A model specifies their
dynamics, while the date grid, path count and random seed control the numerical
experiment.

QuantSupport supports stored path sets generated from a
`SimulationConfiguration` and model-backed observations supplied by a
`MarketModel` to scripting and exposure services. Stored paths are useful for
inspection and numerical payoff valuation. The model-backed services also
retain the path-construction dependencies used for automatic differentiation.

## Materialized numerical paths

`SimulationBuilder` constructs a date grid strictly after the market reference
date through the configured horizon. Model inputs include index, path count,
seed, frequency and day counter. It converts required curves to `f64` and
generates Hull–White, LGM or lognormal Brownian paths using standard-normal
draws from seeded `StdRng`. This route does not inherit the Sobol/antithetic
construction of `LgmMarketModel`.

`GeneratedMonteCarloSimulation` stores the resulting values in `DualFwd`
containers with generation inputs held fixed. It exposes no simulation
pillars, so subsequent differentiation covers only the operations performed
on the stored values. The
[MC equity pricer](../pricing/options.md#materialized-monte-carlo-valuation)
can differentiate its subsequent discounting without providing path-generation
delta or vega.

For lognormal Brownian motion, the interval step has the exponential form

\\[
S_{t+\Delta}=S_t\exp\left[
(r-q-\tfrac12\sigma^2)\Delta+\sigma\sqrt{\Delta}Z\right].
\\]

Here \\(\sigma\\) is sampled at the interval endpoint. The step does not
integrate time-dependent variance across sigma boundaries; aligning or refining
the grid matters when the volatility schedule changes.
Calibrated term-volatility stripping matches accumulated variance at its
selected expiries, but does not change that path-stepping rule.
Spot comes from the reference-date fixing. The builder's
rate is obtained from the terminal discount factor; a general term structure
does not thereby become a fully time-dependent equity carry model.

## Hull–White representation

The one-factor Gaussian short-rate model is conventionally written

\\[
dr_t=(\theta(t)-a r_t)\\,dt+\sigma(t)\\,dW_t.
\\]

Its affine bond representation uses
\\(P(t,T\mid r)=A(t,T)e^{-B(t,T)r}\\), with
\\(B(t,T)=(1-e^{-a(T-t)})/a\\). Model calibration converts caplet or swaption
prices into a piecewise-constant sigma schedule and retains instrument-level
quality records. Selected-source domain and calibration-Jacobian limitations
are described in [Volatility markets](../curves/volatility.md).

The low-level materialized Hull–White path generator is **Euler-discretized**:

\\[
x_{i+1}=x_i-a x_i\Delta_i+
\sigma(t_{i+1})\sqrt{\Delta_i}Z_i.
\\]

The generator updates a variance estimate and adds a curve-derived shift to
the state. Its Euler discretization differs from the exact OU transitions
used in callable-bond valuation. Refine the grid to assess discretization
bias and increase path count to assess sampling precision.

Several low-level Hull–White formulas divide by mean reversion and do not
implement a zero-alpha limit. Do not infer zero-alpha support from the formal
continuous mathematical limit. LGM, described below, explicitly handles zero
lambda.

## LGM rates and numeraire

`LgmRateModel` expresses a rate factor through loading \\(H\\), volatility
loading \\(\alpha\\), and accumulated variance \\(\zeta\\):

\\[
H(t)=\frac{1-e^{-\lambda t}}{\lambda},\quad
H(t)=t\ \text{when }\lambda=0,\qquad
\alpha(t)=\sigma(t)e^{\lambda t},\quad
\zeta(t)=\int_0^t\alpha(u)^2\\,du.
\\]

The zero-lambda limit applies to direct LGM dynamics. Its calibrated
construction delegates to the low-level Hull–White calibrator, whose
zero-alpha singularities remain; zero-lambda calibration is not implied.

For its Gaussian state \\(z\\), the implemented discount factor and reference
numeraire are

\\[
P(t,T;z)=\frac{P(0,T)}{P(0,t)}
\exp\left[-(H(T)-H(t))z
-\tfrac12(H(T)^2-H(t)^2)\zeta(t)\right],
\\]
\\[
N(t,z)=\frac{\exp(H(t)z+\tfrac12H(t)^2\zeta(t))}{P(0,t)}.
\\]

The reference numeraire defines the measure under which the domestic factor
is driftless. Foreign rate factors receive measure-change drift
terms involving domestic-rate and FX correlations.

`LgmMarketModel` combines rate factors with supported FX/equity components.
FX component spot is domestic currency per foreign unit. Curve-driver mappings
let another curve reuse a factor without claiming that the two initial curves
are identical. The state vector orders rates, FX log spots and equity log
spots; correlation input must match that ordering.

Path construction uses Owen-scrambled Sobol draws, antithetic pairing and
Cholesky correlation. Consecutive paths share opposite-sign normal draws.
Even counts preserve complete pairs; odd counts, including one path, are
also supported. Reproducing a run requires matching the seed, grid, model and
indices. Path count controls sampling, while volatility and model inputs
determine whether the dynamics are stochastic.

## Model-backed observations

`MarketModel<T>` configures evaluation dates and typed simulation requests,
then returns fallible path scenarios. Responses contain requested discounts,
forwards, FX, spots, path-dependent observations and numeraires. Its
`generate_path` and `resolve_request` return `Result`, so failures retain their
market/model categories instead of becoming silently absent paths.

Scripting and XVA operate on these model responses in `DualFwd`. They can
propagate through registered model/curve inputs while rewinding temporary
path nodes. The resulting sensitivities remain subject to fixed correlations, partial
calibration Jacobians and payoff smoothing. Stored numerical simulations
retain the more limited risk dependencies described above.

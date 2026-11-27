# Stochastic models and simulation

Simulation generates the future rates, spot prices, and discount factors needed
to value stochastic payoffs and compute exposure. The model sets how these
quantities evolve. The date grid, number of paths, and random seed control the
numerical run.

QuantSupport offers two ways to simulate:

- **Stored path sets**, generated from a `SimulationConfiguration`. Use them to
  inspect paths and to value payoffs numerically.
- **Model-backed observations**, supplied by a `MarketModel` to the scripting
  and exposure engines. These keep the link between the paths and their inputs,
  so AD sensitivities include the model inputs.

## Stored path sets

`SimulationBuilder` builds a date grid from just after the market reference
date up to the configured horizon. Its inputs include the index, number of
paths, seed, frequency, and day counter.

It converts the curves it needs to `f64` and generates Hull–White, LGM, or
lognormal Brownian paths. Random numbers are standard normal draws from a
seeded `StdRng`. This builder does not use the Sobol and antithetic draws of
`LgmMarketModel`.

`GeneratedMonteCarloSimulation` stores the path values as `Recorded` numbers
that are not linked to their inputs. It has no simulation pillars, so
derivatives cover only what is computed from the stored values. For example,
the [MC equity pricer](../pricing/options.md#monte-carlo-with-stored-paths)
gives risk to discounting but no delta or vega from the paths.

For lognormal Brownian motion, each step is

\\[
S_{t+\Delta}=S_t\exp\left[
(r-q-\tfrac12\sigma^2)\Delta+\sigma\sqrt{\Delta}Z\right].
\\]

- \\(\sigma\\) is read at the end of each step. Time-dependent variance is not
  integrated across changes in sigma, so align or refine the grid where the
  volatility schedule changes.
- Calibrated term-volatility stripping matches total variance at the chosen
  expiries, but does not change this stepping rule.
- Spot is the fixing on the reference date.
- The rate \\(r\\) comes from the discount factor at the final date. The equity
  carry is therefore constant over time, even if the curve is not flat.

## Hull–White representation

The one-factor Gaussian short-rate model is usually written

\\[
dr_t=(\theta(t)-a r_t)\\,dt+\sigma(t)\\,dW_t.
\\]

Bond prices have the affine form
\\(P(t,T\mid r)=A(t,T)e^{-B(t,T)r}\\), with
\\(B(t,T)=(1-e^{-a(T-t)})/a\\). Calibration turns caplet or swaption prices
into a piecewise-constant sigma schedule and keeps a quality record for each
instrument. Limits on the source range and the calibration Jacobian are
described in [Volatility markets](../curves/volatility.md).

The stored-path Hull–White generator uses an **Euler** step:

\\[
x_{i+1}=x_i-a x_i\Delta_i+
\sigma(t_{i+1})\sqrt{\Delta_i}Z_i.
\\]

It updates a variance estimate and adds a shift derived from the curve. This
differs from the exact transitions used to value callable bonds. Refine the
grid to measure discretization bias, and add paths to measure sampling error.

Several low-level Hull–White formulas divide by the mean reversion. They do
not support zero mean reversion. LGM, described below, does support zero
lambda.

## LGM rates and numeraire

`LgmRateModel` describes a rate factor with a loading \\(H\\), a volatility
loading \\(\alpha\\), and accumulated variance \\(\zeta\\):

\\[
H(t)=\frac{1-e^{-\lambda t}}{\lambda},\quad
H(t)=t\ \text{when }\lambda=0,\qquad
\alpha(t)=\sigma(t)e^{\lambda t},\quad
\zeta(t)=\int_0^t\alpha(u)^2\\,du.
\\]

Zero lambda works in the LGM dynamics. Calibration, however, uses the
low-level Hull–White calibrator, which does not support zero mean reversion.
So you cannot calibrate LGM with zero lambda.

For the Gaussian state \\(z\\), the discount factor and numeraire are

\\[
P(t,T;z)=\frac{P(0,T)}{P(0,t)}
\exp\left[-(H(T)-H(t))z
-\tfrac12(H(T)^2-H(t)^2)\zeta(t)\right],
\\]
\\[
N(t,z)=\frac{\exp(H(t)z+\tfrac12H(t)^2\zeta(t))}{P(0,t)}.
\\]

Under the measure of this numeraire, the domestic factor has no drift. Foreign
rate factors get extra drift terms from their correlations with the domestic
rate and FX.

`LgmMarketModel` combines rate factors with supported FX and equity factors.

- FX spot is domestic currency per unit of foreign currency.
- A curve-driver mapping lets another curve reuse a factor. The two curves keep
  their own initial values.
- The state vector is ordered as rates, then FX log spots, then equity log
  spots. The correlation matrix must use the same order.

Paths use Owen-scrambled Sobol numbers, antithetic pairs, and Cholesky
correlation.

- Consecutive paths use normal draws of opposite sign.
- An even path count gives complete pairs. Odd counts, including one path, are
  also allowed.
- To reproduce a run, use the same seed, grid, model, and indices.

The path count controls sampling error. The volatility and model inputs decide
how random the dynamics are.

## Model-backed observations

You give a `MarketModel<T>` the evaluation dates and simulation requests. It
returns path scenarios, and each response can contain discount factors,
forwards, FX rates, spots, path-dependent observations, and numeraires.
`generate_path` and `resolve_request` return a `Result`. Errors keep their
market or model category.

Scripting and XVA use these responses as `Recorded` values. Their
sensitivities flow through the registered model and curve inputs. They are
still limited by fixed correlations, partial calibration Jacobians, and payoff
smoothing. Stored path sets have the narrower risk described above.

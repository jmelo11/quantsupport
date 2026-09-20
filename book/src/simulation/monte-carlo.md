# Monte Carlo Framework

Monte Carlo simulation represents future market states as a collection of reproducible paths. QuantSupport uses those paths for option pricing, scripted payoffs, exposure profiles, and XVA. This chapter explains the two simulation interfaces, their configuration, path storage, random-number generation, and the market-model contract.

Two layers serve different scopes:

1. **Single-index path sets** (`SimulationConfiguration` → `SimulationBuilder` → `GeneratedMonteCarloSimulation`) stored in the `PricingContext` and consumed by pricers such as `BlackMCEuropeanOptionPricer`.
2. **Multi-asset market models** (`LgmMarketModel`, the `MarketModel<T>` trait) that drive exposure and XVA engines and the scripting engine ([Scripting](../scripting/overview.md)).

The single-index layer is convenient for a pricer that needs one simulated factor. The market-model layer coordinates several curves, FX rates, and assets under a common path and numeraire.

## `SimulationConfiguration`

A simulation configuration identifies the factor, its dynamics model, and the numerical path grid. The Rust structure records the complete set of choices and their defaults:

```rust,ignore
pub struct SimulationConfiguration {
    market_index: MarketIndex,
    model: ModelConfiguration,
    n_paths: usize,          // default 1000
    seed: u64,               // default 42
    horizon: Period,
    frequency: Frequency,    // default Monthly
    day_counter: DayCounter, // default Actual365
}
SimulationConfiguration::new(market_index, model, n_paths, seed, horizon, frequency)
```

The same configuration can be loaded from JSON. This example simulates monthly SOFR short rates for five years under Hull-White with fixed parameters:

```json
{
  "market_index": "SOFR",
  "model": {
    "HullWhite": {
      "alpha": 0.1,
      "parameter_source": { "Fixed": { "sigma": 0.01 } }
    }
  },
  "n_paths": 2000,
  "seed": 7,
  "horizon": "5Y",
  "frequency": "Monthly"
}
```

The market index links the generated paths to downstream requests. The seed makes the sample reproducible, and the horizon and frequency determine the dates stored in each path.

### `ModelConfiguration`

`ModelConfiguration` selects the stochastic dynamics and its model-specific inputs. The current variants are:

| Variant                                        | Fields                                          | Dynamics                                        |
| ---------------------------------------------- | ----------------------------------------------- | ----------------------------------------------- |
| `HullWhite { alpha, parameter_source }`              | fixed or calibrated Gaussian parameters         | \\(dr = (\theta(t)-\alpha r)dt + \sigma(t)dW\\) |
| `BrownianMotion { parameter_source, dividend_rate }` | fixed or calibrated lognormal parameters        | \\(dS = (r-q)S\\,dt + \sigma(t)S\\,dW\\)        |
| `Lgm { lambda, parameter_source }`                   | fixed or calibrated Gaussian parameters         | see [LGM](lgm.md)                               |

Every model uses `ParameterSource::{Fixed, Calibrated}`. Hull-White and LGM use
`GaussianRateModelParameters { sigma }`. Brownian motion uses
`LognormalModelParameters { volatility }`. A calibrated configuration names a
surface or cube and selects its instruments through `calibration_basket`.

This common parameter-source shape lets a caller supply an explicit model or ask construction to infer its parameters from a volatility market. Each model retains its own parameter type and calibration procedure.

## Building

`SimulationBuilder` converts configurations into generated path sets using the curves, volatilities, quotes, and fixings already available in the market stores:

```rust,ignore
let sims: HashMap<MarketIndex, MonteCarloSimulationElement> =
    SimulationBuilder::new(specs).build(&constructed_store, &quote_store, &fixing_store, Level::Mid)?;
```

`PricingContext::with_simulation_configurations(specs)` schedules this builder during `initialize()`. Curves and volatility objects are constructed first, which makes them available to calibrated models. The path grid advances from the reference date by the configured frequency through the horizon.

### `GeneratedMonteCarloSimulation`

The generated element stores a rectangular path matrix together with its date and model identity metadata. Its principal constructor and accessors are:

```rust,ignore
pub fn new(market_index: MarketIndex, dates: Vec<Date>, paths: Vec<Vec<f64>>, dt: f64) -> Self;
fn path(&self) -> &Vec<Vec<DualFwd>>;   // paths[path][date]
fn n_paths(&self) -> i64;
fn dates(&self) -> &[Date];
fn dt(&self) -> f64;                    // average step in years
fn market_index(&self) -> MarketIndex;
```

The outer path index identifies a scenario and the inner date index identifies a point on its timeline. Values are stored as `DualFwd`, allowing an averaged payoff to retain sensitivities to spot, curve, and volatility leaves.

## `BrownianMotion`

`BrownianMotion` provides lognormal dynamics for equity-like factors. Construction supplies spot, rate, a possibly time-dependent volatility function, and an optional dividend rate:

```rust,ignore
BrownianMotion::new(spot, rate, Box<dyn TimeDependentVolatility<T>>, dividend_rate: Option<T>)
```

The process uses exact lognormal stepping over each interval,
\\(S_{t+\Delta} = S_t\exp\bigl((r-q-\tfrac12\sigma^2)\Delta + \sigma\sqrt\Delta Z\bigr)\\).
Static helpers for price, delta, vega, rho, and theta provide analytic references for European payoff tests.

## Random numbers

Random-number policy determines reproducibility and convergence behavior. Single-index simulations use `rand` with the configured seed. `LgmMarketModel` uses Owen-scrambled Sobol sequences from `sobol_burley`, antithetic pairing, and a Cholesky factor of the configured correlation matrix. Antithetic pairing requires an even path count. Reusing a seed reproduces the same sample.

## `MarketModel<T>` trait

Exposure and scripting components request simulated observables through `MarketModel<T>`. The trait configures dates and requests before generating a path, then resolves typed responses at each evaluation date:

```rust,ignore
pub trait MarketModel<T: Scalar> {
    fn n_paths(&self) -> usize;
    fn set_evaluation_dates(&mut self, dates: Vec<Date>);
    fn set_requests(&mut self, requests: Vec<SimulationRequest>);
    fn generate_path(&self, index: usize) -> Option<PathScenario<T>>;
    fn resolve_request(&self, eval_date: Date, request: &SimulationRequest) -> SimulationResponse<T>;
}
```

`SimulationResponse` carries discount factors, forward rates, FX rates, spots, path-dependent observations, and the numeraire at each evaluation date. Exposure components ask `resolve_request` for the quantities declared by each claim. This request boundary gives `ScriptEngine` and `XvaEngine` access to any model that implements the trait.

## What to remember

Simulation configuration defines a model and a reproducible path grid. Generated path sets serve focused pricing tasks, and `MarketModel<T>` serves multi-factor exposure workflows through typed requests and responses. Keeping paths in `DualFwd` connects Monte Carlo outputs to the same quote and parameter sensitivities used by deterministic pricing.

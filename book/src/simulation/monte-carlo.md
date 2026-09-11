# Monte Carlo Framework

Two simulation layers exist:

1. **Single-index path sets** (`SimulationConfiguration` → `SimulationBuilder` → `GeneratedMonteCarloSimulation`) stored in the `PricingContext` and consumed by pricers such as `BlackMCEuropeanOptionPricer`.
2. **Multi-asset market models** (`LgmMarketModel`, the `MarketModel<T>` trait) that drive exposure and XVA engines and the scripting engine ([Scripting](../scripting/overview.md)).

## `SimulationConfiguration`

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

```json
{
  "market_index": "SOFR",
  "model": {
    "HullWhite": {
      "alpha": 0.1,
      "volatility": { "Constant": { "value": 0.01 } }
    }
  },
  "n_paths": 2000,
  "seed": 7,
  "horizon": "5Y",
  "frequency": "Monthly"
}
```

### `ModelConfiguration`

| Variant                                        | Fields                                          | Dynamics                                        |
| ---------------------------------------------- | ----------------------------------------------- | ----------------------------------------------- |
| `HullWhite { alpha, volatility }`              | mean reversion, `VolatilitySourceConfiguration` | \\(dr = (\theta(t)-\alpha r)dt + \sigma(t)dW\\) |
| `BrownianMotion { volatility, dividend_rate }` | vol source, optional yield                      | \\(dS = (r-q)S\\,dt + \sigma(t)S\\,dW\\)        |
| `Lgm { lambda, volatility }`                   | mean reversion (0 = none), vol source           | see [LGM](lgm.md)                               |

The volatility source may be `Constant`, a point on a `Surface`/`Cube`, or `Calibrated` (fits the sigma schedule to caplets/swaptions, see [Hull-White](hull-white.md)).

## Building

```rust,ignore
let sims: HashMap<MarketIndex, MonteCarloSimulationElement> =
    SimulationBuilder::new(specs).build(&constructed_store, &quote_store, &fixing_store, Level::Mid)?;
```

`PricingContext::with_simulation_configurations(specs)` runs this in `initialize()` after curves and surfaces so calibrated models can see them. The dates grid is `reference_date + k·frequency` up to `horizon`.

### `GeneratedMonteCarloSimulation`

```rust,ignore
pub fn new(market_index: MarketIndex, dates: Vec<Date>, paths: Vec<Vec<f64>>, dt: f64) -> Self;
fn path(&self) -> &Vec<Vec<DualFwd>>;   // paths[path][date]
fn n_paths(&self) -> i64;
fn dates(&self) -> &[Date];
fn dt(&self) -> f64;                    // average step in years
fn market_index(&self) -> MarketIndex;
```

Paths are stored as `DualFwd`, so a pricer averaging payoffs over paths still yields AD sensitivities to spot, curve and volatility leaves.

## `BrownianMotion`

```rust,ignore
BrownianMotion::new(spot, rate, Box<dyn TimeDependentVolatility<T>>, dividend_rate: Option<T>)
```

Exact log-Euler stepping \\(S\_{t+\Delta} = S_t\exp\bigl((r-q-\tfrac12\sigma^2)\Delta + \sigma\sqrt\Delta Z\bigr)\\). Static helpers `closed_form_price`, `delta`, `vega`, `rho`, `theta` (`(fwd, strike, vol, tau, is_call)`) provide analytic references.

## Random numbers

Single-index simulations use `rand` with the configured `seed`. `LgmMarketModel` uses Owen-scrambled Sobol sequences (`sobol_burley`) with antithetic pairing (`n_paths` must be even) and a Cholesky factor of the user correlation matrix; the same `seed` reproduces the same paths.

## `MarketModel<T>` trait

```rust,ignore
pub trait MarketModel<T: Scalar> {
    fn n_paths(&self) -> usize;
    fn set_evaluation_dates(&mut self, dates: Vec<Date>);
    fn set_requests(&mut self, requests: Vec<SimulationRequest>);
    fn generate_path(&self, index: usize) -> Option<PathScenario<T>>;
    fn resolve_request(&self, eval_date: Date, request: &SimulationRequest) -> SimulationResponse<T>;
}
```

`SimulationResponse` carries `discounts`, `forward_rates`, `fx_rates`, `spots`, `path_dependent_observations` and the `numeraire` at each evaluation date; exposure engines call `resolve_request` per claim rather than reading raw states. The `ScriptEngine` and `XvaEngine` accept any implementor.

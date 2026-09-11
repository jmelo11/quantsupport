# Linear Gaussian Markov (LGM)

The multi-currency simulation engine (`src/models/lgm/`) is built from LGM rate components plus lognormal FX and equity components, all driven under the domestic risk-neutral measure.

## `LgmRateModel`

```rust,ignore
pub struct LgmRateModel<'a, T: Scalar> {
    lambda: T,                    // mean reversion (1/years); 0 = none
    sigma_schedule: Vec<(f64, T)>,// piecewise-constant σ(t)
    discount_curve: &'a dyn InterestRatesTermStructure<T>,
}
LgmRateModel::new(lambda, sigma, &curve)
LgmRateModel::new_piecewise(lambda, schedule, &curve)?        // schedule non-empty, increasing
LgmRateModel::calibrated(lambda, &curve, &calibration_config, &store, &quotes, Level::Mid)?
```

`calibrated` runs the Hull-White caplet/swaption bootstrap ([Hull-White](hull-white.md)) and transfers the sigma schedule.

State variable \\(z_t\\) with \\(z_0 = 0\\):

| Method                                                          | Formula                                                                                       |
| --------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `H(t)`                                                          | \\(\frac{1-e^{-\lambda t}}{\lambda}\\) (\\(= t\\) when \\(\lambda\approx 0\\))                |
| `H_dot(t)`                                                      | \\(e^{-\lambda t}\\)                                                                          |
| `alpha(t)`                                                      | \\(\sigma(t)e^{\lambda t}\\)                                                                  |
| `zeta(t)`                                                       | \\(\int_0^t\alpha(s)^2ds\\)                                                                   |
| `P_discount(t, T, z)`                                           | \\(\frac{P(0,T)}{P(0,t)}\exp\\!\bigl(-(H(T)-H(t))z - \tfrac12(H(T)^2-H(t)^2)\zeta(t)\bigr)\\) |
| `numeraire(t, z)`                                               | \\(\exp\\!\bigl(H(t)z + \tfrac12H(t)^2\zeta(t)\bigr)/P(0,t)\\)                                |
| `instantaneous_forward_rate(t, T, z)`                           | \\(f(0,T) + H'(T)H(T)\zeta(t) + H'(T)z\\)                                                     |
| `short_rate(t, z)`                                              | \\(f(t,t\mid z)\\)                                                                            |
| `self_drift(t)`                                                 | 0 (domestic factor is driftless)                                                              |
| `gamma_under_domestic_measure(t, &dom, fx_vol, rho_zx, rho_zz)` | \\(\rho*{zz}\alpha_i\alpha_d H_d - \alpha_i^2 H_i - \rho*{zx}\sigma_X\alpha_i\\)              |
| `evolve_domestic_factor_euler(t, z, dt, dw)`                    | \\(z + \alpha(t)\\,dW\\)                                                                      |
| `evolve_foreign_factor_under_domestic_measure_euler(..)`        | \\(z + \gamma\\,dt + \alpha(t)\\,dW\\)                                                        |

## FX and equity components

```rust,ignore
LgmFxModel::new(&domestic_rates, &foreign_rates, fx_vol, spot_0, rho_zx_dom_fx)  // spot = domestic per foreign
LgmEquityModel::new(&domestic_rates, equity_vol, spot_0, dividend_yield: Option<f64>, rho_zs_dom)
```

## `LgmMarketModel`

```rust,ignore
let mut model = LgmMarketModel::new(Currency::USD, MarketIndex::SOFR, reference_date, DayCounter::Actual365)
    .with_n_paths(2000)                  // must be even (antithetic)
    .with_seed(42)
    .with_correlation_matrix(corr);      // ordered as the state vector below
model.add_curve_model(MarketIndex::SOFR, sofr_lgm);
model.add_curve_model(MarketIndex::ICP, icp_lgm);
model.add_fx_model(Currency::CLP, clp_fx);
model.add_equity_model("AAPL".into(), aapl);
model.set_curve_driver(MarketIndex::TermSOFR3m, MarketIndex::SOFR);  // index simulated off another factor
model.set_evaluation_dates(dates);
model.set_requests(requests);
```

State vector \\(Y(t) = [z_d, z_{f_1},\dots,z_{f_F}, \log X_1,\dots,\log X_F, \log S_1,\dots,\log S_E]\\) with dynamics under the domestic measure

\\[
\begin{aligned}
dz_d &= \alpha_d\\,dW_d, & dz_{f_i} &= \gamma_i\\,dt + \alpha_i\\,dW_{f_i},\\
d\log X_i &= (r_d - r_i + \rho_{d,X_i}\alpha_d H_d\sigma_{X_i} - \tfrac12\sigma_{X_i}^2)dt + \sigma_{X_i}dW_{X_i},\\
d\log S_j &= (r_d - q_j + \rho_{d,S_j}\alpha_d H_d\sigma_{S_j} - \tfrac12\sigma_{S_j}^2)dt + \sigma_{S_j}dW_{S_j}.
\end{aligned}
\\]

Path generation: Owen-scrambled Sobol draws → antithetic pairs → Cholesky-correlated increments → Euler steps between consecutive evaluation dates → `resolve_request` answers each `SimulationRequest` (discount factor, forward rate, FX, spot, numeraire) from the state. Discount factors within a path are `P_discount`, forward rates come from `instantaneous_forward_rate`/`P_discount` ratios, and the numeraire is used to deflate cashflows in exposure and pricing engines.

## JSON configuration

`ModelConfiguration::Lgm { lambda, volatility }` in a `SimulationConfiguration`, or `LgmModelConfig` inside `XvaEngineConfig` ([XVA Overview](../xva/overview.md)):

```json
{
  "market_index": "SOFR",
  "lambda": 0.05,
  "volatility": {
    "Calibrated": {
      "source": { "Surface": { "market_index": "SOFR" } },
      "quote_ids": [
        "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black"
      ],
      "strike": "Atm",
      "alpha": 0.05
    }
  }
}
```

Provide either `sigma` (constant) or `volatility`; `driver` lets an index reuse another index's factor.

## Example

`cargo run -p pfe` builds a USD SOFR swap and an EUR/USD FX forward, bootstraps SOFR and ESTR, loads an LGM configuration, simulates, and prints per-trade NPV, the exposure profile through `PfeAggregator` (quantiles by date) — see [Exposure Simulation](exposure.md).

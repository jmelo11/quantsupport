# Linear Gaussian Markov (LGM)

The Linear Gaussian Markov framework combines analytically tractable rate factors with correlated FX and equity dynamics. QuantSupport uses it as the multi-asset market model for scripting, exposure, and XVA. This chapter explains the individual rate component, the composition of a domestic-measure market model, parameter configuration, and the complete PFE example. The implementation lives in `src/models/lgm/`.

## `LgmRateModel`

`LgmRateModel` describes one currency or rate index with mean reversion, a piecewise-constant volatility schedule, and an initial discount curve. It can be built from a constant sigma, an explicit schedule, or a model calibration configuration:

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

The calibrated constructor runs the caplet or swaption procedure described in [Hull-White](hull-white.md) and transfers the resulting sigma schedule into the LGM representation.

The model state is \\(z_t\\) with \\(z_0 = 0\\). From that state, the following methods calculate loadings, variance, discount factors, the numeraire, and drifts:

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
| `gamma_under_domestic_measure(t, &dom, fx_vol, rho_zx, rho_zz)` | \\(\rho_{zz}\alpha_i\alpha_d H_d - \alpha_i^2 H_i - \rho_{zx}\sigma_X\alpha_i\\)              |
| `evolve_domestic_factor_euler(t, z, dt, dw)`                    | \\(z + \alpha(t)\\,dW\\)                                                                      |
| `evolve_foreign_factor_under_domestic_measure_euler(..)`        | \\(z + \gamma\\,dt + \alpha(t)\\,dW\\)                                                        |

The functions \\(H\\) and \\(\zeta\\) connect the Gaussian state to bond prices and numeraires. Domestic factors have zero drift under the domestic measure. Foreign factors receive `gamma_under_domestic_measure`, which incorporates rate and FX correlations.

## FX and equity components

FX and equity components add lognormal state variables driven under the domestic measure. Each constructor receives the domestic rate model and the correlations needed for its drift adjustment:

```rust,ignore
LgmFxModel::new(&domestic_rates, &foreign_rates, fx_vol, spot_0, rho_zx_dom_fx)  // spot = domestic per foreign
LgmEquityModel::new(&domestic_rates, equity_vol, spot_0, dividend_yield: Option<f64>, rho_zs_dom)
```

The FX spot convention is domestic currency per unit of foreign currency. The equity model uses an optional dividend yield. Both components share correlated increments with the rate factors through the market model's correlation matrix.

## `LgmMarketModel`

`LgmMarketModel` assembles the rate, FX, and equity components into one ordered state vector. The following example creates a USD-domestic model, adds SOFR and ICP rate factors, CLP FX, and AAPL equity, then configures requests and dates:

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

Path generation begins with Owen-scrambled Sobol draws. Antithetic pairing expands the sample, a Cholesky factor applies the configured correlations, and Euler steps advance the state across consecutive evaluation dates. `resolve_request` then converts each state into the discount factors, forward rates, FX rates, spots, and numeraires requested by claims or scripts.

Pathwise discount factors use `P_discount`. Forward rates follow from instantaneous forwards or discount-factor ratios. The domestic numeraire deflates cashflows consistently for exposure and pricing.

## JSON configuration

LGM appears as `ModelConfiguration::Lgm { lambda, parameter_source }` inside a `SimulationConfiguration` and as `LgmModelConfig` inside `XvaEngineConfig`. [XVA Overview](../xva/overview.md) develops the latter. This fragment configures a SOFR factor calibrated to selected ATM surface expiries:

```json
{
  "market_index": "SOFR",
  "lambda": 0.05,
  "parameter_source": {
    "Calibrated": {
      "source": { "Surface": { "market_index": "SOFR" } },
      "calibration_basket": {
        "expiries": ["1Y", "2Y", "5Y"],
        "strike": "Atm"
      }
    }
  }
}
```

`parameter_source` accepts a complete fixed sigma or a calibration source and basket. A `driver` lets one index reuse the factor of another index. A driver-based configuration obtains mean reversion and volatility from that source factor, so its own `lambda` and `parameter_source` fields are omitted.

## Example

The `pfe` example builds a USD SOFR swap and a EUR/USD FX forward, bootstraps SOFR and ESTR, loads the LGM configuration, and simulates their shared market. It prints trade NPVs followed by exposure quantiles from `PfeAggregator`. Run it with `cargo run -p pfe` and use [Exposure Simulation](exposure.md) to interpret the resulting profile.

## What to remember

Each LGM rate component converts one initial curve and volatility schedule into a Gaussian factor. `LgmMarketModel` joins those factors with FX and equity states under one domestic measure and correlation matrix. Typed simulation requests then translate the shared state into the observables required by pricing, scripting, and XVA.
